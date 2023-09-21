mod clmm;
mod kamino;
mod orca_state;
mod raydium_state;

use anchor_lang::prelude::*;
pub use kamino::{CollateralInfo, CollateralInfos, GlobalConfig, RebalanceRaw, WhirlpoolStrategy};
use orca_state::{Position as OrcaPositionParser, Whirlpool as WhirlpoolParser};
use raydium_state::{
    PersonalPositionState as RaydiumPositionParser, PoolState as RaydiumPoolParser,
};

use self::kamino::{get_price_per_full_share, TokenPrices};
use crate::{
    oracles::ktokens::{
        clmm::{orca_clmm::OrcaClmm, raydium_clmm::RaydiumClmm, Clmm},
        kamino::{scope::get_prices_from_data, DEX},
    },
    utils::zero_copy_deserialize,
    DatedPrice, Price, Result, ScopeError,
};

const USD_DECIMALS_PRECISION: u8 = 6;

// Gives the price of 1 kToken in USD
pub fn get_price<'a, 'b>(
    k_account: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // Get the root account
    let strategy_account_ref = WhirlpoolStrategy::from_account(k_account)?;

    // extract the accounts from extra iterator
    let global_config_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;
    // Get the global config account (checked below)
    let global_config_account_ref = GlobalConfig::from_account(global_config_account_info)?;

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
    account_check(pool_account_info, strategy_account_ref.pool, "whirlpool")?;
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

    let dex = DEX::try_from(strategy_account_ref.strategy_dex).unwrap();
    let clmm: Box<dyn Clmm> = match dex {
        DEX::Orca => {
            let pool = WhirlpoolParser::from_account_to_orca_whirlpool(pool_account_info)?;
            let position = if strategy_account_ref.position != Pubkey::default() {
                let position =
                    OrcaPositionParser::from_account_to_orca_position(position_account_info)?;
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
            let pool = RaydiumPoolParser::from_account_to_raydium_pool(pool_account_info)?;
            let position = if strategy_account_ref.position != Pubkey::default() {
                let position =
                    RaydiumPositionParser::from_account_to_raydium_position(position_account_info)?;
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
    let scope_prices_ref = zero_copy_deserialize::<crate::OraclePrices>(scope_prices_account_info)?;

    let token_prices = get_prices_from_data(
        &scope_prices_ref,
        &collateral_infos_ref.infos,
        &strategy_account_ref,
        Some(clmm.as_ref()),
        clock.slot,
    )?;

    let token_price =
        get_price_per_full_share(&strategy_account_ref, clmm.as_ref(), &token_prices)?;

    let last_updated_slot = token_prices
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
