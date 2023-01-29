mod kamino;
mod orca_state;

use anchor_lang::prelude::*;
pub use kamino::WhirlpoolStrategy;
use orca_state::{Position as PositionParser, Whirlpool as WhirlpoolParser};

use self::kamino::{get_price_per_full_share, TokenPrices};
use crate::{utils::zero_copy_deserialize, DatedPrice, Price, Result, ScopeError};

const USD_DECIMALS_PRECISION: u8 = 8;

// Gives the price of 1 kToken in USD
pub fn get_price<'a, 'b>(
    k_account: &AccountInfo,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // Get the root account
    let strategy_account_ref = WhirlpoolStrategy::from_account(k_account)?;

    // extract the accounts from extra iterator
    let whirlpool_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let position_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let scope_account = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let scope_chain_account_info = extra_accounts
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
        whirlpool_account_info,
        strategy_account_ref.whirlpool,
        "whirlpool",
    )?;
    account_check(
        position_account_info,
        strategy_account_ref.position,
        "position",
    )?;
    account_check(
        scope_account,
        strategy_account_ref.scope_prices,
        "scope_prices",
    )?;
    let (scope_chain_pk, _) = Pubkey::find_program_address(
        &[b"ScopeChain", &strategy_account_ref.scope_prices.to_bytes()],
        k_account.owner,
    );
    account_check(scope_chain_account_info, scope_chain_pk, "scope_chain")?;

    // Deserialize accounts
    let whirlpool = WhirlpoolParser::from_account_to_orca_whirlpool(whirlpool_account_info)?;
    let position = PositionParser::from_account_to_orca_position(position_account_info)?;
    let scope_prices_ref = zero_copy_deserialize::<crate::OraclePrices>(scope_account)?;
    let scope_chain_ref = zero_copy_deserialize::<crate::utils::scope_chain::ScopeChainAccount>(
        scope_chain_account_info,
    )?;

    let collateral_token_prices =
        TokenPrices::compute(&scope_prices_ref, &scope_chain_ref, &strategy_account_ref)?;
    let token_price = get_price_per_full_share(
        &strategy_account_ref,
        &whirlpool,
        &position,
        &collateral_token_prices,
    )?;

    let last_updated_slot = collateral_token_prices
        .price_a
        .last_updated_slot
        .min(collateral_token_prices.price_b.last_updated_slot);
    let unix_timestamp = collateral_token_prices
        .price_a
        .unix_timestamp
        .min(collateral_token_prices.price_b.unix_timestamp);
    let value: u64 = token_price.as_u64();
    let exp = USD_DECIMALS_PRECISION.into();

    Ok(DatedPrice {
        price: Price { value, exp },
        last_updated_slot,
        unix_timestamp,
        ..Default::default()
    })
}
