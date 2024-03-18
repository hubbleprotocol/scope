use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::decimal::Decimal;
use perpetuals::Custody;
use solana_program::program_pack::Pack;

use crate::utils::account_deserialize;
use crate::utils::math::ten_pow;
use crate::{DatedPrice, Price, Result, ScopeError};

pub use jup_perp_itf as perpetuals;
pub use perpetuals::utils::{check_mint_pk, get_mint_pk};
pub const POOL_VALUE_SCALE_DECIMALS: u8 = 6;

/// Gives the price of 1 JLP token in USD
///
/// Uses the AUM of the pool and the supply of the JLP token to compute the price
pub fn get_price_no_recompute<'a, 'b>(
    jup_pool_acc: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    let jup_pool_pk = jup_pool_acc.key;
    let jup_pool: perpetuals::Pool = account_deserialize(jup_pool_acc)?;

    let mint_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    check_mint_pk(jup_pool_pk, mint_acc.key, jup_pool.lp_token_bump)
        .map_err(|_| ScopeError::UnexpectedAccount)?;

    let mint = {
        let mint_borrow = mint_acc.data.borrow();
        Mint::unpack(&mint_borrow)
    }?;

    let lp_value = jup_pool.aum_usd;
    let lp_token_supply = mint.supply;

    // This is a sanity check to make sure the mint is configured as expected
    // This allows to just divide the two values to get the price
    require_eq!(mint.decimals, POOL_VALUE_SCALE_DECIMALS);

    let price_dec = Decimal::from(lp_value) / lp_token_supply;
    let dated_price = DatedPrice {
        price: price_dec.into(),
        // TODO: find a way to get the last update time
        last_updated_slot: clock.slot,
        unix_timestamp: u64::try_from(clock.unix_timestamp).unwrap(),
        ..Default::default()
    };

    Ok(dated_price)
}

pub fn validate_jlp_pool(account: &AccountInfo) -> Result<()> {
    let _jlp_pool: perpetuals::Pool = account_deserialize(account)?;
    Ok(())
}

/// Get the price of 1 JLP token in USD
///
/// This function recompute the AUM of the pool from the custodies and the oracles
/// Required extra accounts:
/// - Mint of the JLP token
/// - All custodies of the pool
/// - All oracles of the pool (from the custodies)
pub fn get_price_recomputed<'a, 'b>(
    jup_pool_acc: &AccountInfo<'a>,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // 1. Get accounts
    let jup_pool_pk = jup_pool_acc.key;
    let jup_pool: perpetuals::Pool = account_deserialize(jup_pool_acc)?;

    let mint_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    // Get custodies and oracles
    let num_custodies = jup_pool.custodies.len();

    // Note: we take all the needed accounts before any check to leave the iterator in a consistent state
    // (otherwise, we could break the next price computation)
    let custodies_accs = extra_accounts.take(num_custodies).collect::<Vec<_>>();
    require!(
        custodies_accs.len() == num_custodies,
        ScopeError::AccountsAndTokenMismatch
    );

    let oracles_accs = extra_accounts.take(num_custodies).collect::<Vec<_>>();
    require!(
        oracles_accs.len() == num_custodies,
        ScopeError::AccountsAndTokenMismatch
    );

    // 2. Check accounts
    check_accounts(jup_pool_pk, &jup_pool, mint_acc, &custodies_accs)?;
    // Check of oracles will be done in the next step while deserializing custodies
    // (avoid double iteration or keeping custodies in memory)

    // 3. Get mint supply

    let lp_token_supply = get_lp_token_supply(mint_acc)?;

    // 4. Compute AUM and prices

    let custodies_and_prices_iter = custodies_accs.into_iter().zip(oracles_accs);
    let aum_and_age_getter = |(custody_acc, oracle_acc): (&AccountInfo, &AccountInfo),
                              clock: &Clock|
     -> Result<CustodyAumResult> {
        let custody: Custody = account_deserialize(custody_acc)?;
        require!(
            custody.oracle.oracle_type == perpetuals::OracleType::Pyth,
            ScopeError::UnexpectedJlpConfiguration
        );
        require_keys_eq!(
            custody.oracle.oracle_account,
            *oracle_acc.key,
            ScopeError::UnexpectedAccount
        );
        let dated_price = super::pyth::get_price(oracle_acc, clock)?;
        compute_custody_aum(&custody, &dated_price)
    };

    compute_price_from_custodies_and_prices(
        lp_token_supply,
        clock,
        custodies_and_prices_iter,
        aum_and_age_getter,
    )
}

fn compute_price_from_custodies_and_prices<T>(
    lp_token_supply: u64,
    clock: &Clock,
    custodies_and_prices_iter: impl Iterator<Item = T>,
    aum_and_age_getter: impl Fn(T, &Clock) -> Result<CustodyAumResult>,
) -> Result<DatedPrice> {
    let mut oldest_price_ts: u64 = clock.unix_timestamp.try_into().unwrap();
    let mut oldest_price_slot: u64 = clock.slot;

    let lp_value: u128 = {
        let mut pool_amount_usd: u128 = 0;
        let mut trader_short_profits: u128 = 0;

        for custody_and_price in custodies_and_prices_iter {
            // Compute custody AUM
            let custody_r = aum_and_age_getter(custody_and_price, clock)?;

            pool_amount_usd += custody_r.token_amount_usd;
            trader_short_profits += custody_r.trader_short_profits;

            // Update oldest price
            if custody_r.price_ts < oldest_price_ts {
                oldest_price_ts = custody_r.price_ts;
                oldest_price_slot = custody_r.price_slot;
            }
        }

        pool_amount_usd.saturating_sub(trader_short_profits)
    };

    // 5. Compute price
    let price_dec = Decimal::from(lp_value) / lp_token_supply;

    let dated_price = DatedPrice {
        price: price_dec.into(),
        last_updated_slot: oldest_price_slot,
        unix_timestamp: oldest_price_ts,
        ..Default::default()
    };

    Ok(dated_price)
}

fn check_accounts(
    jup_pool_pk: &Pubkey,
    jup_pool: &perpetuals::Pool,
    mint_acc: &AccountInfo,
    custodies_accs: &[&AccountInfo],
) -> Result<()> {
    check_mint_pk(jup_pool_pk, mint_acc.key, jup_pool.lp_token_bump)
        .map_err(|_| error!(ScopeError::UnexpectedAccount))?;

    for (expected_custody_pk, custody_acc) in jup_pool.custodies.iter().zip(custodies_accs.iter()) {
        require_keys_eq!(
            *expected_custody_pk,
            *custody_acc.key,
            ScopeError::UnexpectedAccount
        );
    }
    Ok(())
}

fn get_lp_token_supply(mint_acc: &AccountInfo) -> Result<u64> {
    let mint_borrow = mint_acc.data.borrow();
    let mint = Mint::unpack(&mint_borrow)?;

    // This is a sanity check to make sure the mint is configured as expected
    // This allows to just divide aum by the supply to get the price
    require_eq!(mint.decimals, POOL_VALUE_SCALE_DECIMALS);

    Ok(mint.supply)
}

struct CustodyAumResult {
    pub token_amount_usd: u128,
    pub trader_short_profits: u128,

    pub price_ts: u64,
    pub price_slot: u64,
}

/// Compute the AUM of a custody scaled by `POOL_VALUE_SCALE_DECIMALS` decimals
fn compute_custody_aum(custody: &Custody, dated_price: &DatedPrice) -> Result<CustodyAumResult> {
    let price = dated_price.price;

    let (token_amount_usd, trader_short_profits) = if custody.is_stable {
        (
            asset_amount_to_usd(&price, custody.assets.owned, custody.decimals),
            0,
        )
    } else {
        let mut pool_amount_usd: u128 = 0;
        let mut trader_short_profits: u128 = 0;
        // calculate global short profit / loss of pool
        if custody.assets.global_short_sizes > 0 {
            let (global_pnl_delta, trader_has_profit) = custody
                .get_global_short_pnl(
                    price
                        .to_scaled_value(POOL_VALUE_SCALE_DECIMALS)
                        .try_into()
                        .unwrap(),
                )
                .ok_or_else(|| error!(ScopeError::MathOverflow))?;

            // add global short profit / loss
            if trader_has_profit {
                trader_short_profits += global_pnl_delta;
            } else {
                pool_amount_usd += global_pnl_delta;
            }
        }

        // calculate long position profit / loss
        pool_amount_usd += u128::from(custody.assets.guaranteed_usd);

        let net_assets_token = custody
            .assets
            .owned
            .checked_sub(custody.assets.locked)
            .ok_or_else(|| error!(ScopeError::MathOverflow))?;
        let net_assets_usd = asset_amount_to_usd(&price, net_assets_token, custody.decimals);
        pool_amount_usd += net_assets_usd;

        (pool_amount_usd, trader_short_profits)
    };

    Ok(CustodyAumResult {
        token_amount_usd,
        trader_short_profits,
        price_ts: dated_price.unix_timestamp,
        price_slot: dated_price.last_updated_slot,
    })
}

/// Return the value of the number of tokens in USD scaled by `POOL_VALUE_SCALE_DECIMALS` decimals
fn asset_amount_to_usd(price: &Price, token_amount: u64, token_decimals: u8) -> u128 {
    let price_value: u128 = price.value.into();
    let token_amount: u128 = token_amount.into();
    let token_decimals: u8 = token_decimals;
    let price_decimals: u8 = price.exp.try_into().unwrap();

    // price * 10^(-price_decimals) * token_amount * 10^(-token_decimals) * 10^POOL_VALUE_SCALE_DECIMALS
    if price_decimals + token_decimals > POOL_VALUE_SCALE_DECIMALS {
        let diff = price_decimals + token_decimals - POOL_VALUE_SCALE_DECIMALS;
        let nom = price_value * token_amount;
        let denom = ten_pow(diff);

        nom / denom
    } else {
        let diff = POOL_VALUE_SCALE_DECIMALS - (price_decimals + token_decimals);
        price_value * token_amount * ten_pow(diff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(1, 0, 1, 0, 1_000_000)]
    #[test_case(1_000_000, 6, 1, 0, 1_000_000)]
    #[test_case(1, 0, 1, 6, 1)]
    #[test_case(5, 0, 5, 0, 25_000_000)]
    #[test_case(10, 0, 6, 0, 60_000_000)]
    #[test_case(25, 3, 1, 3, 25)]
    fn test_asset_amount_to_usd(
        price_value: u64,
        price_exp: u64,
        token_amount: u64,
        token_decimals: u8,
        expected: u128,
    ) {
        let price = Price {
            value: price_value,
            exp: price_exp,
        };
        let token_amount: u64 = token_amount;
        let token_decimals: u8 = token_decimals;

        let usd = asset_amount_to_usd(&price, token_amount, token_decimals);
        assert_eq!(usd, expected);
    }
}
