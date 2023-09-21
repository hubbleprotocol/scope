use std::ops::Deref;

use anchor_lang::prelude::*;
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
    utils::{enums::LiquidityCalculationMode, scope::ScopePrices},
};

use crate::{
    utils::{account_deserialize, zero_copy_deserialize},
    DatedPrice, Price, Result, ScopeError, ScopeResult,
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

    let holdings = common::holdings(
        &strategy_account_ref,
        clmm.as_ref(),
        &token_prices,
        LiquidityCalculationMode::Deposit,
    )?;

    // exclude the value of uncompounded rewards
    let holdings_value = holdings
        .total_sum
        .checked_sub(holdings.rewards_usd)
        .ok_or(ScopeError::IntegerOverflow)?;

    let token_price = get_price_per_full_share_impl(
        &holdings_value,
        strategy_account_ref.shares_issued,
        strategy_account_ref.shares_mint_decimals,
    )?;

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
) -> ScopeResult<Box<dyn Clmm + 'a>> {
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
