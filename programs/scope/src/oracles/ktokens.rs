use std::ops::Deref;

use anchor_lang::{prelude::*, Result};
use kamino::{
    clmm::{orca_clmm::OrcaClmm, Clmm},
    raydium_amm_v3::states::{PersonalPositionState as RaydiumPosition, PoolState as RaydiumPool},
    raydium_clmm::RaydiumClmm,
    state::{CollateralInfos, GlobalConfig, WhirlpoolStrategy},
    utils::types::DEX,
    whirlpool::state::{Position as OrcaPosition, Whirlpool as OrcaWhirlpool},
};
use yvaults as kamino;
use yvaults::{
    operations::vault_operations::{common, common::get_price_per_full_share_impl},
    state::CollateralToken,
    utils::{
        enums::LiquidityCalculationMode,
        price::TokenPrices,
        scope::ScopePrices,
        types::{Holdings, RewardsAmounts},
    },
};

use crate::{
    utils::{account_deserialize, zero_copy_deserialize},
    DatedPrice, Price, ScopeError,
};

const USD_DECIMALS_PRECISION: u8 = 6;

/// Gives the price of 1 kToken in USD
///
/// This is the price of the underlying assets in USD divided by the number of shares issued
///
/// Underlying assets is the sum of invested, uninvested and fees of token_a and token_b
///
/// Reward tokens are excluded from the calculation as they are generally lower value/mcap and can be manipulated
///
/// When calculating invested amounts, a sqrt price derived from scope price_a and price_b is used to determine the 'correct' ratio of underlying assets, the sqrt price of the pool cannot be considered reliable
///
/// The kToken price timestamp is taken from the least-recently updated price in the scope price chains of token_a and token_b
pub fn get_price<'a, 'b>(
    k_account: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // Get the root account
    let strategy_account_ref = zero_copy_deserialize::<WhirlpoolStrategy>(k_account)?;

    // extract the accounts from extra iterator
    let global_config_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;
    // Get the global config account (checked below)
    let global_config_account_ref =
        zero_copy_deserialize::<GlobalConfig>(global_config_account_info)?;

    let collateral_infos_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let pool_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let position_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let scope_prices_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let account_check = |account: &AccountInfo, expected, name| {
        let pk = account.key();
        if pk != expected {
            msg!(
                "Ktoken received account {} for {} is not the one expected ({})",
                pk,
                name,
                expected
            );
            err!(ScopeError::UnexpectedAccount)
        } else {
            Ok(())
        }
    };

    // Check the pubkeys
    account_check(
        global_config_account_info,
        strategy_account_ref.global_config,
        "global_config",
    )?;
    account_check(
        collateral_infos_account_info,
        global_config_account_ref.token_infos,
        "collateral_infos",
    )?;
    account_check(pool_account_info, strategy_account_ref.pool, "pool")?;
    account_check(
        position_account_info,
        strategy_account_ref.position,
        "position",
    )?;
    account_check(
        scope_prices_account_info,
        strategy_account_ref.scope_prices,
        "scope_prices",
    )?;

    // Deserialize accounts
    let collateral_infos_ref =
        zero_copy_deserialize::<CollateralInfos>(collateral_infos_account_info)?;
    let scope_prices_ref =
        zero_copy_deserialize::<kamino::scope::OraclePrices>(scope_prices_account_info)?;

    let clmm = get_clmm(
        pool_account_info,
        position_account_info,
        &strategy_account_ref,
    )?;

    let token_prices = kamino::utils::scope::get_prices_from_data(
        scope_prices_ref.deref(),
        &collateral_infos_ref.infos,
        &strategy_account_ref,
        Some(clmm.as_ref()),
        clock.slot,
    )?;

    let holdings = holdings(&strategy_account_ref, clmm.as_ref(), &token_prices)?;

    let token_price = get_price_per_full_share_impl(
        &holdings.total_sum,
        strategy_account_ref.shares_issued,
        strategy_account_ref.shares_mint_decimals,
    )?;

    // Get the least-recently updated component price from both scope chains
    let (last_updated_slot, unix_timestamp) = get_component_px_last_update(
        &scope_prices_ref,
        &collateral_infos_ref,
        &strategy_account_ref,
    )?;
    let value: u64 = token_price.as_u64();
    let exp = USD_DECIMALS_PRECISION.into();

    Ok(DatedPrice {
        price: Price { value, exp },
        last_updated_slot,
        unix_timestamp,
        ..Default::default()
    })
}

fn get_clmm<'a, 'info>(
    pool: &'a AccountInfo<'info>,
    position: &'a AccountInfo<'info>,
    strategy: &WhirlpoolStrategy,
) -> Result<Box<dyn Clmm + 'a>> {
    let dex = DEX::try_from(strategy.strategy_dex).unwrap();
    let clmm: Box<dyn Clmm> = match dex {
        DEX::Orca => {
            let pool = account_deserialize::<OrcaWhirlpool>(pool)?;
            let position = if strategy.position != Pubkey::default() {
                let position = account_deserialize::<OrcaPosition>(position)?;
                Some(position)
            } else {
                None
            };
            Box::new(OrcaClmm {
                pool,
                position,
                lower_tick_array: None,
                upper_tick_array: None,
            })
        }
        DEX::Raydium => {
            let pool = zero_copy_deserialize::<RaydiumPool>(pool)?;
            let position = if strategy.position != Pubkey::default() {
                let position = account_deserialize::<RaydiumPosition>(position)?;
                Some(position)
            } else {
                None
            };
            Box::new(RaydiumClmm {
                pool,
                position,
                protocol_position: None,
                lower_tick_array: None,
                upper_tick_array: None,
            })
        }
    };
    Ok(clmm)
}

/// Returns the last updated slot and unix timestamp of the least-recently updated component price
/// Excludes rewards prices as they do not form part of the calculation
fn get_component_px_last_update(
    scope_prices: &ScopePrices,
    collateral_infos: &CollateralInfos,
    strategy: &WhirlpoolStrategy,
) -> Result<(u64, u64)> {
    let token_a = yvaults::state::CollateralToken::try_from(strategy.token_a_collateral_id)
        .map_err(|_| ScopeError::ConversionFailure)?;
    let token_b = yvaults::state::CollateralToken::try_from(strategy.token_b_collateral_id)
        .map_err(|_| ScopeError::ConversionFailure)?;

    let collateral_info_a = collateral_infos.infos[token_a.to_usize()];
    let collateral_info_b = collateral_infos.infos[token_b.to_usize()];
    let token_a_chain: yvaults::utils::scope::ScopeConversionChain =
        collateral_info_a
            .try_into()
            .map_err(|_| ScopeError::BadScopeChainOrPrices)?;
    let token_b_chain: yvaults::utils::scope::ScopeConversionChain =
        collateral_info_b
            .try_into()
            .map_err(|_| ScopeError::BadScopeChainOrPrices)?;

    let price_chain = token_a_chain
        .iter()
        .chain(token_b_chain.iter())
        .map(|&token_id| scope_prices.prices[usize::from(token_id)])
        .collect::<Vec<yvaults::scope::DatedPrice>>();

    let (last_updated_slot, unix_timestamp): (u64, u64) =
        price_chain
            .iter()
            .fold((0_u64, 0_u64), |(slot, ts), price| {
                if slot == 0 || price.last_updated_slot.lt(&slot) {
                    (price.last_updated_slot, price.unix_timestamp)
                } else {
                    (slot, ts)
                }
            });

    Ok((last_updated_slot, unix_timestamp))
}

/// Returns the holdings of the strategy
/// Use a sqrt price derived from price_a and price_b, not from the pool as it cannot be considered reliable
/// Exclude rewards from the holdings calculation, as they are generally low value/mcap and can be manipulated
pub fn holdings(
    strategy: &WhirlpoolStrategy,
    clmm: &dyn Clmm,
    prices: &TokenPrices,
) -> Result<Holdings> {
    // https://github.com/0xparashar/UniV3NFTOracle/blob/master/contracts/UniV3NFTOracle.sol#L27
    // We are using the sqrt price derived from price_a and price_b
    // instead of the whirlpool price which could be manipulated/stale
    let pool_sqrt_price = price_utils::sqrt_price_from_scope_prices(
        &prices.get(
            CollateralToken::try_from(strategy.token_a_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?,
        )?,
        &prices.get(
            CollateralToken::try_from(strategy.token_b_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?,
        )?,
        strategy.token_a_mint_decimals,
        strategy.token_b_mint_decimals,
    )?;

    if cfg!(feature = "debug") {
        let w = price_utils::calc_price_from_sqrt_price(
            clmm.get_current_sqrt_price(),
            strategy.token_a_mint_decimals,
            strategy.token_b_mint_decimals,
        );
        let o = price_utils::calc_price_from_sqrt_price(
            pool_sqrt_price,
            strategy.token_a_mint_decimals,
            strategy.token_b_mint_decimals,
        );
        let diff = (w - o).abs() / w;
        msg!("o: {} w: {} d: {}%", w, o, diff * 100.0);
    }

    holdings_no_rewards(strategy, clmm, prices, pool_sqrt_price)
}

pub fn holdings_no_rewards(
    strategy: &WhirlpoolStrategy,
    clmm: &dyn Clmm,
    prices: &TokenPrices,
    pool_sqrt_price: u128,
) -> Result<Holdings> {
    let (available, invested, fees) = common::underlying_inventory(
        strategy,
        clmm,
        LiquidityCalculationMode::Deposit,
        clmm.get_position_liquidity()?,
        pool_sqrt_price,
    )?;
    // exclude rewards
    let rewards = RewardsAmounts::default();

    let holdings = common::holdings_usd(strategy, available, invested, fees, rewards, prices)?;

    Ok(holdings)
}

mod price_utils {
    use decimal_wad::rate::U128;
    use num_traits::Pow;

    use super::*;

    const TARGET_EXPONENT: u64 = 12;

    // Helper
    fn sub(a: u64, b: u64) -> Result<u32> {
        let res = a.checked_sub(b).ok_or(ScopeError::IntegerOverflow)?;
        u32::try_from(res).map_err(|_e| error!(ScopeError::IntegerOverflow))
    }

    fn pow(base: u64, exp: u64) -> U128 {
        U128::from(base).pow(U128::from(exp))
    }

    fn abs_diff(a: i32, b: i32) -> u32 {
        if a > b {
            a.checked_sub(b).unwrap().try_into().unwrap()
        } else {
            b.checked_sub(a).unwrap().try_into().unwrap()
        }
    }

    fn decimals_factor(decimals_a: u64, decimals_b: u64) -> Result<(U128, u64)> {
        let decimals_a = i32::try_from(decimals_a).map_err(|_e| ScopeError::IntegerOverflow)?;
        let decimals_b = i32::try_from(decimals_b).map_err(|_e| ScopeError::IntegerOverflow)?;

        let diff = abs_diff(decimals_a, decimals_b);
        let factor = U128::from(10_u64.pow(diff));
        Ok((factor, u64::from(diff)))
    }

    pub fn a_to_b(
        a: &yvaults::utils::price::Price,
        b: &yvaults::utils::price::Price,
    ) -> Result<yvaults::utils::price::Price> {
        let exp = TARGET_EXPONENT;
        let exp = u64::max(exp, a.exp);
        let exp = u64::max(exp, b.exp);

        let extra_factor_a = 10_u64.pow(sub(exp, a.exp)?);
        let extra_factor_b = 10_u64.pow(sub(exp, b.exp)?);

        let px_a = U128::from(a.value.checked_mul(extra_factor_a).unwrap());
        let px_b = U128::from(b.value.checked_mul(extra_factor_b).unwrap());

        let final_factor = pow(10, exp);

        let price_a_to_b = px_a
            .checked_mul(final_factor)
            .unwrap()
            .checked_div(px_b)
            .unwrap();

        Ok(yvaults::utils::price::Price {
            value: price_a_to_b.as_u64(),
            exp,
        })
    }

    pub fn calc_sqrt_price_from_scope_price(
        price: &yvaults::utils::price::Price,
        decimals_a: u64,
        decimals_b: u64,
    ) -> Result<u128> {
        // Normally we calculate sqrt price from a float price as following:
        // px = sqrt(price * 10 ^ (decimals_b - decimals_a)) * 2 ** 64

        // But scope price is scaled by 10 ** exp so, to obtain it, we need to divide by sqrt(10 ** exp)
        // x = sqrt(scaled_price * 10 ^ (decimals_b - decimals_a)) * 2 ** 64
        // px = x / sqrt(10 ** exp)

        let (decimals_factor, decimals_diff) = decimals_factor(decimals_a, decimals_b)?;
        let px = U128::from(price.value);
        let (scaled_price, final_exp) = if decimals_b > decimals_a {
            (px.checked_mul(decimals_factor).unwrap(), price.exp)
        } else {
            // If we divide by 10 ^ (decimals_a - decimals_b) here we lose precision
            // So instead we lift the price even more (by the diff) and assume a bigger exp
            (px, price.exp.checked_add(decimals_diff).unwrap())
        };

        let two_factor = pow(2, 64);
        let x = scaled_price
            .integer_sqrt()
            .checked_mul(two_factor)
            .ok_or(ScopeError::IntegerOverflow)?;

        let sqrt_factor = pow(10, final_exp).integer_sqrt();

        Ok(x.checked_div(sqrt_factor)
            .ok_or(ScopeError::IntegerOverflow)?
            .as_u128())
    }

    pub fn sqrt_price_from_scope_prices(
        price_a: &yvaults::utils::price::Price,
        price_b: &yvaults::utils::price::Price,
        decimals_a: u64,
        decimals_b: u64,
    ) -> Result<u128> {
        calc_sqrt_price_from_scope_price(&a_to_b(price_a, price_b)?, decimals_a, decimals_b)
    }

    pub fn calc_price_from_sqrt_price(price: u128, decimals_a: u64, decimals_b: u64) -> f64 {
        let sqrt_price_x_64 = price as f64;
        (sqrt_price_x_64 / 2.0_f64.powf(64.0)).powf(2.0)
            * 10.0_f64.pow(decimals_a as i32 - decimals_b as i32)
    }
}

#[cfg(test)]
mod tests_price_utils {
    use num_traits::Pow;
    use price_utils::*;
    use yvaults::utils::price::Price;

    use super::*;

    pub fn calc_sqrt_price_from_float_price(price: f64, decimals_a: u64, decimals_b: u64) -> u128 {
        let px = (price * 10.0_f64.pow(decimals_b as i32 - decimals_a as i32)).sqrt();
        let res = (px * 2.0_f64.powf(64.0)) as u128;
        res
    }

    pub fn f(price: Price) -> f64 {
        let factor = 10_f64.pow(price.exp as f64);
        price.value as f64 / factor
    }

    fn p(price: f64, exp: u64) -> Price {
        let factor = 10_f64.pow(exp as f64);
        Price {
            value: (price * factor) as u64,
            exp,
        }
    }

    #[test]
    fn test_sqrt_price_from_scope_price() {
        // To USD
        let token_a_price = Price {
            value: 1_000_000_000,
            exp: 9,
        };

        // To USD
        let token_b_price = Price {
            value: 1_000_000_000,
            exp: 9,
        };

        let a_to_b_price = a_to_b(&token_a_price, &token_b_price);
        println!("a_to_b_price: {:?}", a_to_b_price);

        // assert_eq!(sqrt_price_from_scope_price(scope_price), sqrt_price);
    }

    #[test]
    fn test_sqrt_price_from_float() {
        let price = 1.0;
        let px1 = calc_sqrt_price_from_float_price(price, 6, 6);
        let px2 = calc_sqrt_price_from_float_price(price, 9, 9);
        let px3 = calc_sqrt_price_from_float_price(price, 6, 9);
        let px4 = calc_sqrt_price_from_float_price(price, 9, 6);

        println!("px1: {}", px1);
        println!("px2: {}", px2);
        println!("px3: {}", px3);
        println!("px4: {}", px4);
    }

    #[test]
    fn test_sqrt_price_from_price() {
        let px = Price {
            value: 1_000_000_000,
            exp: 9,
        };

        // sqrt_price_from_price = (price * 10 ^ (decimals_b - decimals_a)).sqrt() * 2 ^ 64;

        let x = calc_sqrt_price_from_scope_price(&px, 6, 6).unwrap();
        let y = calc_sqrt_price_from_float_price(f(px), 6, 6);

        println!("x: {}", x);
        println!("y: {}", y);

        for (decimals_a, decimals_b) in
            [(1, 10), (6, 6), (9, 6), (6, 9), (9, 9), (10, 1)].into_iter()
        {
            let x = calc_sqrt_price_from_float_price(f(px), decimals_a, decimals_b);
            let y = calc_sqrt_price_from_scope_price(&px, decimals_a, decimals_b).unwrap();

            let px_x = calc_price_from_sqrt_price(x, decimals_a, decimals_b);
            let px_y = calc_price_from_sqrt_price(y, decimals_a, decimals_b);

            let diff = (px_x - px_y).abs();
            println!("x: {}, y: {} diff: {}", x, y, diff);
        }
    }

    #[test]
    fn scope_prices_to_sqrt_prices() {
        let decimals_a: u64 = 6;
        let decimals_b: u64 = 6;

        let a = 1.0;
        let b = 2.0;

        let price = a / b;
        let expected = calc_sqrt_price_from_float_price(price, decimals_a, decimals_b);

        // Now go the other way around
        let a = p(a, decimals_a.into());
        let b = p(b, decimals_b.into());
        let actual = sqrt_price_from_scope_prices(&a, &b, decimals_a, decimals_b).unwrap();

        println!("expected: {}", expected);
        println!("actual: {}", actual);
    }

    #[test]
    fn test_numerical_examples() {
        let sol = Price {
            exp: 8,
            value: 3232064150,
        };
        let eth = Price {
            exp: 8,
            value: 128549278944,
        };
        let btc = Price {
            exp: 8,
            value: 1871800000000,
        };
        let usdh = Price {
            exp: 10,
            value: 9984094565,
        };
        let stsol = Price {
            exp: 8,
            value: 3420000000,
        };
        let usdc = Price {
            exp: 8,
            value: 99998498,
        };
        let usdt = Price {
            exp: 8,
            value: 99985005,
        };
        let ush = Price {
            exp: 10,
            value: 9942477073,
        };
        let uxd = Price {
            exp: 10,
            value: 10007754362,
        };
        let dust = Price {
            exp: 10,
            value: 11962756205,
        };
        let usdr = Price {
            exp: 10,
            value: 9935635809,
        };

        for (price_a, price_b, expected_sqrt, decimals_a, decimals_b, tolerance) in [
            (usdh, usdc, 18432369086522948808, 6, 6, 0.07),
            (sol, stsol, 17927878403230908080, 9, 9, 0.07),
            (usdc, usdt, 18446488013153244324, 6, 6, 0.07),
            (ush, usdc, 581657083814290012, 9, 6, 0.07),
            (usdr, usdc, 18387972314427037052, 6, 6, 0.07),
            (sol, dust, 95888115807158641354, 9, 9, 0.07),
            (sol, usdh, 3317976242955018545, 9, 6, 0.07),
            (uxd, usdc, 18454272046764295796, 6, 6, 0.07),
            (usdh, eth, 5149554401170243770, 6, 8, 0.4),
            (usdh, btc, 134876121531740447, 6, 6, 0.4),
        ] {
            let actual =
                sqrt_price_from_scope_prices(&price_a, &price_b, decimals_a, decimals_b).unwrap();

            let expected = calc_price_from_sqrt_price(expected_sqrt, decimals_a, decimals_b);
            let actual = calc_price_from_sqrt_price(actual, decimals_a, decimals_b);
            let diff_pct = (actual - expected) / expected * 100.0;
            println!("expected_sqrt: {}", expected_sqrt);
            println!("actual: {}", actual);
            println!("expected: {}", expected);
            println!("diff: {}%", diff_pct);
            println!("---");
            assert!(diff_pct.abs() < tolerance) // 0.07% diff
        }
    }
}

#[cfg(test)]
mod tests {
    use yvaults::{
        scope::{DatedPrice, OraclePrices, Price},
        state::CollateralInfo,
    };

    use super::*;

    #[test]
    pub fn test_get_component_px_last_update_single_link_chains() {
        let (scope_prices, collateral_infos, strategy) =
            new_mapped_prices(vec![(6000, 3000)], vec![(2000, 1000)]);

        let (slot, ts) =
            get_component_px_last_update(&scope_prices, &collateral_infos, &strategy).unwrap();

        assert_eq!(slot, 2000);
        assert_eq!(ts, 1000);
    }

    #[test]
    pub fn test_get_component_px_last_update_multi_link_chains() {
        let (scope_prices, collateral_infos, strategy) = new_mapped_prices(
            vec![(8000, 4000), (7000, 3500), (6000, 3000), (5000, 2500)],
            vec![(4000, 2000), (3000, 1500), (2000, 1000), (1000, 500)],
        );

        let (slot, ts) =
            get_component_px_last_update(&scope_prices, &collateral_infos, &strategy).unwrap();

        assert_eq!(slot, 1000);
        assert_eq!(ts, 500);
    }

    #[test]
    pub fn test_get_component_px_last_update_multi_and_single_link_chains() {
        let (scope_prices, collateral_infos, strategy) = new_mapped_prices(
            vec![(8000, 4000), (7000, 3500), (6000, 3000), (5000, 2500)],
            vec![(4000, 2000)],
        );

        let (slot, ts) =
            get_component_px_last_update(&scope_prices, &collateral_infos, &strategy).unwrap();

        assert_eq!(slot, 4000);
        assert_eq!(ts, 2000);
    }

    fn new_mapped_prices(
        token_a_chain: Vec<(u64, u64)>,
        token_b_chain: Vec<(u64, u64)>,
    ) -> (OraclePrices, CollateralInfos, WhirlpoolStrategy) {
        let oracle_prices = new_oracle_prices(&token_a_chain, &token_b_chain);
        let collateral_infos = new_collateral_infos(token_a_chain.len(), token_b_chain.len());
        let strategy = new_strategy();
        (oracle_prices, collateral_infos, strategy)
    }

    fn new_oracle_prices(
        token_a_chain: &Vec<(u64, u64)>,
        token_b_chain: &Vec<(u64, u64)>,
    ) -> OraclePrices {
        let price = DatedPrice {
            ..DatedPrice::default()
        };
        let mut oracle_prices = OraclePrices {
            oracle_mappings: Default::default(),
            prices: [price; crate::MAX_ENTRIES],
        };

        for (a, (a_slot, a_ts)) in token_a_chain.iter().enumerate() {
            oracle_prices.prices[a] = DatedPrice {
                price: Price {
                    value: 100000,
                    exp: 6,
                },
                last_updated_slot: *a_slot,
                unix_timestamp: *a_ts,
                ..Default::default()
            };
        }
        for (b, (b_slot, b_ts)) in token_b_chain.iter().enumerate() {
            oracle_prices.prices[b + 4] = DatedPrice {
                price: Price {
                    value: 100000,
                    exp: 6,
                },
                last_updated_slot: *b_slot,
                unix_timestamp: *b_ts,
                ..Default::default()
            };
        }
        oracle_prices
    }

    fn new_collateral_infos(token_a_chain_len: usize, token_b_chain_len: usize) -> CollateralInfos {
        let mut collateral_infos = CollateralInfos {
            infos: [CollateralInfo::default(); 256],
        };
        let mut token_a_chain = [u16::MAX, u16::MAX, u16::MAX, u16::MAX];
        for a in 0..token_a_chain_len {
            token_a_chain[a] = a as u16;
        }
        let mut token_b_chain = [u16::MAX, u16::MAX, u16::MAX, u16::MAX];
        for b in 0..token_b_chain_len {
            let b_offset = b + 4;
            token_b_chain[b] = b_offset as u16;
        }
        collateral_infos.infos[0].scope_price_chain = token_a_chain;
        collateral_infos.infos[1].scope_price_chain = token_b_chain;
        collateral_infos
    }

    fn new_strategy() -> WhirlpoolStrategy {
        WhirlpoolStrategy {
            token_a_collateral_id: 0,
            token_b_collateral_id: 1,
            ..WhirlpoolStrategy::default()
        }
    }
}
