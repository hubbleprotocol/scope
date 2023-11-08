use std::ops::Deref;

use anchor_lang::{prelude::*, Result};
use yvaults::{
    self as kamino,
    clmm::{orca_clmm::OrcaClmm, Clmm},
    operations::vault_operations::common,
    raydium_amm_v3::states::{PersonalPositionState as RaydiumPosition, PoolState as RaydiumPool},
    raydium_clmm::RaydiumClmm,
    state::CollateralToken,
    state::{CollateralInfos, GlobalConfig, WhirlpoolStrategy},
    utils::types::DEX,
    utils::{enums::LiquidityCalculationMode, price::TokenPrices},
    whirlpool::state::{Position as OrcaPosition, Whirlpool as OrcaWhirlpool},
};

use crate::utils::math::u64_div_to_price;
use crate::{
    utils::{account_deserialize, zero_copy_deserialize},
    DatedPrice, ScopeError,
};

use super::ktokens::price_utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTypes {
    TokenA,
    TokenB,
}

/// Gives the number of token (A or B) lamports per kToken lamport
///
/// This is the total holdings of the given underlying asset divided by the number of shares issued
/// Underlying asset is the sum of invested, uninvested and fees of either token_a or token_b
/// Reward tokens are included if equal to token_a or token_b
///
/// The kToken price timestamp is current time
pub fn get_token_x_per_share<'a, 'b>(
    k_account: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
    token: TokenTypes,
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
                "Ktoken token per share: received account {} for {} is not the one expected ({})",
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

    let num_token_x =
        holdings_of_token_x(&strategy_account_ref, clmm.as_ref(), &token_prices, token)?;
    let num_shares = strategy_account_ref.shares_issued;

    // Get the least-recently updated component price from both scope chains
    let last_updated_slot = clock.slot;
    let unix_timestamp = u64::try_from(clock.unix_timestamp).expect("Unix timestamp negative");
    let price = u64_div_to_price(num_token_x, num_shares);

    Ok(DatedPrice {
        price,
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

/// Returns amount of token x in the strategy
/// Use a sqrt price derived from price_a and price_b, not from the pool as it cannot be considered reliable
pub fn holdings_of_token_x(
    strategy: &WhirlpoolStrategy,
    clmm: &dyn Clmm,
    prices: &TokenPrices,
    token: TokenTypes,
) -> Result<u64> {
    // https://github.com/0xparashar/UniV3NFTOracle/blob/master/contracts/UniV3NFTOracle.sol#L27
    // We are using the sqrt price derived from price_a and price_b
    // instead of the whirlpool price which could be manipulated/stale
    let pool_sqrt_price_from_oracle_prices = price_utils::sqrt_price_from_scope_prices(
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

    let pool_sqrt_price = clmm.get_current_sqrt_price();

    msg!("[KToken to Token X] pool_sqrt_price: {pool_sqrt_price} vs sqrt_price_from_oracle_prices: {pool_sqrt_price_from_oracle_prices}",);

    let (available, invested, fees) = common::underlying_inventory(
        strategy,
        clmm,
        LiquidityCalculationMode::Deposit,
        clmm.get_position_liquidity()?,
        pool_sqrt_price,
    )?;

    let (available, invested, fees) = match token {
        TokenTypes::TokenA => (available.a, invested.a, fees.a),
        TokenTypes::TokenB => (available.b, invested.b, fees.b),
    };

    // rewards
    let r = clmm
        .get_position_pending_rewards(Some(strategy.token_a_mint), Some(strategy.token_b_mint))?;

    let rewards = [
        &r.reward_0,
        &r.reward_1,
        &r.reward_2,
        &r.reward_3,
        &r.reward_4,
        &r.reward_5,
    ];
    let sum_rewards_x = rewards.into_iter().fold(0_u64, |acc, x| {
        if x.is_token_a && token == TokenTypes::TokenA
            || x.is_token_b && token == TokenTypes::TokenB
        {
            acc + x.amount
        } else {
            acc
        }
    });

    Ok(available + invested + fees + sum_rewards_x)
}
