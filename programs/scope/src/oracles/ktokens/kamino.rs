use std::{cell::Ref, convert::TryInto};

use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    *,
};
use decimal_wad::{
    common::{TryDiv, TryMul},
    decimal::Decimal,
    rate::U128,
};
use num::traits::Pow;
use num_enum::{IntoPrimitive, TryFromPrimitive};
pub use whirlpool::state::{Position, PositionRewardInfo, Whirlpool, WhirlpoolRewardInfo};

use crate::{
    dbg_msg,
    oracles::ktokens::{clmm::Clmm, kamino::price_utils::calc_market_value_token_usd},
    scope_chain,
    scope_chain::ScopeChainError,
    utils::zero_copy_deserialize,
    DatedPrice, OraclePrices, ScopeError, ScopeResult,
};

const TARGET_EXPONENT: u64 = 12;
const SIZE_REBALANCE_PARAMS: usize = 128;
const SIZE_REBALANCE_STATE: usize = 256;

pub const ORCA_REWARDS_COUNT: usize = 3;

pub const KAMINO_REWARDS_UPPER_INDEX: usize = 5;

use super::USD_DECIMALS_PRECISION;

pub fn get_price_per_full_share(
    strategy: &WhirlpoolStrategy,
    clmm: &dyn Clmm,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let holdings = holdings(strategy, clmm, prices)?;

    get_price_per_full_share_impl(
        &holdings,
        strategy.shares_issued,
        strategy.shares_mint_decimals,
    )
}

pub fn get_price_per_full_share_impl(
    holdings: &Holdings,
    shares_issued: u64,
    shares_decimals: u64,
) -> ScopeResult<U128> {
    if shares_issued == 0 {
        Ok(underlying_unit(shares_decimals))
    } else {
        let res = Decimal::from(underlying_unit(shares_decimals))
            .try_mul(holdings.total_sum)?
            .try_div(shares_issued)?
            .try_ceil()?;

        Ok(res)
    }
}

pub fn holdings_usd(
    strategy: &WhirlpoolStrategy,
    available: TokenAmounts,
    invested: TokenAmounts,
    fees: TokenAmounts,
    rewards: RewardsAmounts,
    prices: &TokenPrices,
) -> ScopeResult<Holdings> {
    let available_usd = amounts_usd(strategy, &available, prices)?;

    let invested_usd = amounts_usd(strategy, &invested, prices)?;

    let fees_usd = amounts_usd(strategy, &fees, prices)?;

    let rewards_usd = rewards_total_usd_value(strategy, &rewards, prices)?;

    let total_sum = available_usd
        .checked_add(invested_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(fees_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(rewards_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?;

    Ok(Holdings {
        available,
        available_usd,
        invested,
        invested_usd,
        fees,
        fees_usd,
        rewards,
        rewards_usd,
        total_sum,
    })
}

pub fn holdings(
    strategy: &WhirlpoolStrategy,
    clmm: &dyn Clmm,
    prices: &TokenPrices,
) -> ScopeResult<Holdings> {
    // TODO: add uncollected rewards
    // Not adding rewards exposes the program to rent seekers
    // there would be a time window when
    // you can earn more than you rightfully should,
    // at the expense of those who actually should
    // but doesn't open the program to attacks

    let available = amounts_available(strategy);
    let invested = amounts_invested(clmm)?;
    let fees = clmm.get_position_pending_fees()?;
    let rewards = clmm.get_position_pending_rewards()?;

    holdings_usd(strategy, available, invested, fees, rewards, prices)
}

pub fn amounts_invested(clmm: &dyn Clmm) -> ScopeResult<TokenAmounts> {
    amounts_invested_from_liquidity(clmm, clmm.get_position_liquidity()?)
}

pub fn amounts_invested_from_liquidity(
    clmm: &dyn Clmm,
    liquidity: u128,
) -> ScopeResult<TokenAmounts> {
    let (a, b) = if liquidity > 0 {
        let sqrt_price_lower = clmm.sqrt_price_from_tick(clmm.get_position_tick_lower_index()?);
        let sqrt_price_upper = clmm.sqrt_price_from_tick(clmm.get_position_tick_upper_index()?);

        let (delta_a, delta_b) = clmm.get_amounts_from_liquidity(
            clmm.get_current_sqrt_price(),
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity.try_into().unwrap(),
        );

        (delta_a, delta_b)
    } else {
        (0, 0)
    };

    Ok(TokenAmounts { a, b })
}

// We calculate the value of any tokens to USD
// Since all tokens are quoted to USD
// We calculate up to USD_DECIMALS_PRECISION (as exponent)
pub fn amounts_usd_token(
    strategy: &WhirlpoolStrategy,
    token_amount: u64,
    is_a: bool,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let (token_collateral_id, token_mint_decimals) = match is_a {
        true => (
            CollateralToken::try_from(strategy.token_a_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?,
            strategy.token_a_mint_decimals,
        ),
        false => (
            CollateralToken::try_from(strategy.token_b_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?,
            strategy.token_b_mint_decimals,
        ),
    };

    if token_amount > 0 {
        calc_market_value_token_usd(
            token_amount,
            &prices.get(token_collateral_id)?,
            u8::try_from(token_mint_decimals)?,
        )
    } else {
        Ok(U128::from(0))
    }
}

pub fn pending_fees(position: &Position) -> TokenAmounts {
    TokenAmounts {
        a: position.fee_owed_a,
        b: position.fee_owed_b,
    }
}

pub fn pending_rewards(
    position: &Position,
    whirlpool: &Whirlpool,
    strategy: &WhirlpoolStrategy,
) -> RewardsAmounts {
    let reward_0 = if whirlpool.reward_infos[0].initialized() {
        position.reward_infos[0].amount_owed
    } else {
        0
    };
    let reward_1 = if whirlpool.reward_infos[1].initialized() {
        position.reward_infos[1].amount_owed
    } else {
        0
    };
    let reward_2 = if whirlpool.reward_infos[2].initialized() {
        position.reward_infos[2].amount_owed
    } else {
        0
    };
    let reward_3 = if strategy.kamino_rewards[0].initialized() {
        strategy.kamino_rewards[0].amount_uncollected
    } else {
        0
    };
    let reward_4 = if strategy.kamino_rewards[1].initialized() {
        strategy.kamino_rewards[1].amount_uncollected
    } else {
        0
    };
    let reward_5 = if strategy.kamino_rewards[2].initialized() {
        strategy.kamino_rewards[2].amount_uncollected
    } else {
        0
    };
    RewardsAmounts {
        reward_0,
        reward_1,
        reward_2,
        reward_3,
        reward_4,
        reward_5,
    }
}

pub fn orca_pending_rewards(position: &Position, whirlpool: &Whirlpool) -> RewardsAmounts {
    let reward_0 = if whirlpool.reward_infos[0].initialized() {
        position.reward_infos[0].amount_owed
    } else {
        0
    };
    let reward_1 = if whirlpool.reward_infos[1].initialized() {
        position.reward_infos[1].amount_owed
    } else {
        0
    };
    let reward_2 = if whirlpool.reward_infos[2].initialized() {
        position.reward_infos[2].amount_owed
    } else {
        0
    };
    RewardsAmounts {
        reward_0,
        reward_1,
        reward_2,
        reward_3: 0,
        reward_4: 0,
        reward_5: 0,
    }
}

use raydium_amm_v3::states::{PersonalPositionState as RaydiumPosition, PoolState};

pub fn raydium_pending_rewards(position: &RaydiumPosition, pool: &PoolState) -> RewardsAmounts {
    let reward_0 = if pool.reward_infos[0].initialized() {
        position.reward_infos[0].reward_amount_owed
    } else {
        0
    };
    let reward_1 = if pool.reward_infos[1].initialized() {
        position.reward_infos[1].reward_amount_owed
    } else {
        0
    };
    let reward_2 = if pool.reward_infos[2].initialized() {
        position.reward_infos[2].reward_amount_owed
    } else {
        0
    };
    RewardsAmounts {
        reward_0,
        reward_1,
        reward_2,
        reward_3: 0,
        reward_4: 0,
        reward_5: 0,
    }
}

fn amounts_usd(
    strategy: &WhirlpoolStrategy,
    amounts: &TokenAmounts,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let market_value_a = amounts_usd_token(strategy, amounts.a, true, prices)?;
    let market_value_b = amounts_usd_token(strategy, amounts.b, false, prices)?;

    market_value_a
        .checked_add(market_value_b)
        .ok_or(ScopeError::IntegerOverflow)
}

pub fn reward_amount_usd(
    strategy: &WhirlpoolStrategy,
    amount: u64,
    prices: &TokenPrices,
    reward_index: u8,
    reward_collateral_id: u8,
) -> ScopeResult<U128> {
    let reward_decimals = op_utils::strategy_reward_decimals(strategy, reward_index)?;

    if amount == 0 {
        return Ok(U128::from(0));
    }

    let price = prices.get(
        CollateralToken::try_from(reward_collateral_id)
            .map_err(|_| ScopeError::ConversionFailure)?,
    )?;

    Ok(calc_market_value_token_usd(amount, &price, reward_decimals).unwrap())
}

pub fn rewards_total_usd_value(
    strategy: &WhirlpoolStrategy,
    rewards: &RewardsAmounts,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let rewards_0_usd =
        if strategy.reward_0_vault == strategy.base_vault_authority || rewards.reward_0 == 0 {
            U128::from(0)
        } else {
            reward_amount_usd(
                strategy,
                rewards.reward_0,
                prices,
                0,
                strategy.reward_0_collateral_id.try_into().unwrap(),
            )?
        };

    let rewards_1_usd =
        if strategy.reward_1_vault == strategy.base_vault_authority || rewards.reward_1 == 0 {
            U128::from(0)
        } else {
            reward_amount_usd(
                strategy,
                rewards.reward_1,
                prices,
                1,
                strategy.reward_1_collateral_id.try_into().unwrap(),
            )?
        };

    let rewards_2_usd =
        if strategy.reward_2_vault == strategy.base_vault_authority || rewards.reward_2 == 0 {
            U128::from(0)
        } else {
            reward_amount_usd(
                strategy,
                rewards.reward_2,
                prices,
                2,
                strategy.reward_2_collateral_id.try_into().unwrap(),
            )?
        };

    let rewards_3_usd = if !strategy.kamino_rewards[0].initialized() || rewards.reward_3 == 0 {
        U128::from(0)
    } else {
        reward_amount_usd(
            strategy,
            rewards.reward_3,
            prices,
            3,
            strategy.kamino_rewards[0]
                .reward_collateral_id
                .try_into()
                .unwrap(),
        )?
    };

    let rewards_4_usd = if !strategy.kamino_rewards[1].initialized() || rewards.reward_4 == 0 {
        U128::from(0)
    } else {
        reward_amount_usd(
            strategy,
            rewards.reward_4,
            prices,
            4,
            strategy.kamino_rewards[1]
                .reward_collateral_id
                .try_into()
                .unwrap(),
        )?
    };

    let rewards_5_usd = if !strategy.kamino_rewards[2].initialized() || rewards.reward_5 == 0 {
        U128::from(0)
    } else {
        reward_amount_usd(
            strategy,
            rewards.reward_5,
            prices,
            5,
            strategy.kamino_rewards[2]
                .reward_collateral_id
                .try_into()
                .unwrap(),
        )?
    };

    rewards_0_usd
        .checked_add(rewards_1_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(rewards_2_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(rewards_3_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(rewards_4_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
        .checked_add(rewards_5_usd)
        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))
}

/// The decimal scalar for vault underlying and operations involving exchangeRate().
fn underlying_unit(share_decimals: u64) -> U128 {
    ten_pow(share_decimals.try_into().unwrap())
}

fn amounts_available(strategy: &WhirlpoolStrategy) -> TokenAmounts {
    TokenAmounts {
        a: strategy.token_a_amounts,
        b: strategy.token_b_amounts,
    }
}

pub mod liquidity_calcs {
    use decimal_wad::decimal::U192;

    use super::*;

    pub fn get_amount_b_for_liquidity(
        mut sqrt_price_a: u128,
        mut sqrt_price_b: u128,
        liquidity: u128,
        round_up: bool,
    ) -> u128 {
        // println!(
        //     "get_amount_b_for_liquidity sqrt_price_a={:?} sqrt_price_b={:?} liquidity={:?} round_up={:?}",
        //     sqrt_price_a, sqrt_price_b, liquidity, round_up
        // );
        if sqrt_price_a > sqrt_price_b {
            std::mem::swap(&mut sqrt_price_a, &mut sqrt_price_b)
        }

        let q64 = U192::from(2_u128.pow(64));

        let sqrt_price_a = U192::from(sqrt_price_a);
        let sqrt_price_b = U192::from(sqrt_price_b);
        let diff = sqrt_price_b.checked_sub(sqrt_price_a).unwrap();

        let numerator = U192::from(liquidity).checked_mul(diff).unwrap();
        let result = numerator.checked_div(q64).unwrap();
        let result = if round_up {
            if numerator.div_mod(q64).1.as_u128() != 0 {
                result.checked_add(U192::from(1)).unwrap()
            } else {
                result
            }
        } else {
            result
        };
        result.as_u128()
    }

    pub fn get_amount_a_for_liquidity(
        mut sqrt_price_a: u128,
        mut sqrt_price_b: u128,
        liquidity: u128,
        round_up: bool,
    ) -> u128 {
        if sqrt_price_a > sqrt_price_b {
            std::mem::swap(&mut sqrt_price_a, &mut sqrt_price_b)
        }

        let sqrt_price_a = U192::from(sqrt_price_a);
        let sqrt_price_b = U192::from(sqrt_price_b);
        let liquidity = U192::from(liquidity);

        let diff = sqrt_price_b.checked_sub(sqrt_price_a).unwrap();
        let numerator = liquidity.checked_mul(diff).unwrap() << 64;
        let denominator = sqrt_price_b.checked_mul(sqrt_price_a).unwrap();
        let res = if round_up {
            math::div_round_up(numerator, denominator).unwrap()
        } else {
            numerator.checked_div(denominator).unwrap()
        };

        res.as_u128()
    }
}

pub mod math {
    use decimal_wad::decimal::U192;

    use crate::{ScopeError, ScopeResult};

    pub fn div_round_up(n: U192, d: U192) -> ScopeResult<U192> {
        div_round_up_if(n, d, true)
    }

    pub fn div_round_up_if(n: U192, d: U192, round_up: bool) -> ScopeResult<U192> {
        let zero = U192::zero();
        if d == zero {
            return Err(ScopeError::ConversionFailure); // todo - elliot
        }

        let q = n / d;

        Ok(if round_up && n % d > zero { q + 1 } else { q })
    }
}

fn ten_pow(exponent: u8) -> U128 {
    match exponent {
        16 => U128::from(10_000_000_000_000_000_u128),
        15 => U128::from(1_000_000_000_000_000_u128),
        14 => U128::from(100_000_000_000_000_u128),
        13 => U128::from(10_000_000_000_000_u128),
        12 => U128::from(1_000_000_000_000_u128),
        11 => U128::from(100_000_000_000_u128),
        10 => U128::from(10_000_000_000_u128),
        9 => U128::from(1_000_000_000_u128),
        8 => U128::from(100_000_000_u128),
        7 => U128::from(10_000_000_u128),
        6 => U128::from(1_000_000_u128),
        5 => U128::from(100_000_u128),
        4 => U128::from(10_000_u128),
        3 => U128::from(1_000_u128),
        2 => U128::from(100_u128),
        1 => U128::from(10_u128),
        0 => U128::from(1_u128),
        exponent => U128::from(10_u128).pow(U128::from(exponent)),
    }
}

// Zero copy
#[account(zero_copy)]
#[derive(Debug, Default)]
pub struct WhirlpoolStrategy {
    // Admin
    pub admin_authority: Pubkey,

    pub global_config: Pubkey,

    // this is an u8 but we need to keep it as u64 for memory alignment
    pub base_vault_authority: Pubkey,
    pub base_vault_authority_bump: u64,

    // pool info
    pub pool: Pubkey,
    pub pool_token_vault_a: Pubkey,
    pub pool_token_vault_b: Pubkey,

    // Current position info
    pub tick_array_lower: Pubkey,
    pub tick_array_upper: Pubkey,
    pub position: Pubkey,
    pub position_mint: Pubkey,
    pub position_metadata: Pubkey,
    pub position_token_account: Pubkey,

    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub token_a_vault_authority: Pubkey,
    pub token_b_vault_authority: Pubkey,
    pub token_a_vault_authority_bump: u64,
    pub token_b_vault_authority_bump: u64,

    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_mint_decimals: u64,
    pub token_b_mint_decimals: u64,

    pub token_a_amounts: u64,
    pub token_b_amounts: u64,

    pub token_a_collateral_id: u64,
    pub token_b_collateral_id: u64,

    pub scope_prices: Pubkey,
    pub scope_program: Pubkey,

    // shares
    pub shares_mint: Pubkey,
    pub shares_mint_decimals: u64,
    pub shares_mint_authority: Pubkey,
    pub shares_mint_authority_bump: u64,
    pub shares_issued: u64,

    // status
    pub status: u64,

    // rewards
    pub reward_0_amount: u64,
    pub reward_0_vault: Pubkey,
    pub reward_0_collateral_id: u64,
    pub reward_0_decimals: u64,

    pub reward_1_amount: u64,
    pub reward_1_vault: Pubkey,
    pub reward_1_collateral_id: u64,
    pub reward_1_decimals: u64,

    pub reward_2_amount: u64,
    pub reward_2_vault: Pubkey,
    pub reward_2_collateral_id: u64,
    pub reward_2_decimals: u64,

    pub deposit_cap_usd: u64,

    pub fees_a_cumulative: u64,
    pub fees_b_cumulative: u64,
    pub reward_0_amount_cumulative: u64,
    pub reward_1_amount_cumulative: u64,
    pub reward_2_amount_cumulative: u64,

    pub deposit_cap_usd_per_ixn: u64,

    pub withdrawal_cap_a: WithdrawalCaps,
    pub withdrawal_cap_b: WithdrawalCaps,

    pub max_price_deviation_bps: u64,
    // Maximum slippage vs current oracle price
    pub swap_vault_max_slippage_bps: u32,
    // Maximum slippage vs price reference see `reference_swap_price_x`
    pub swap_vault_max_slippage_from_reference_bps: u32,

    // Strategy type can be NON_PEGGED=0, PEGGED=1, STABLE=2
    pub strategy_type: u64,

    // Fees taken by strategy
    pub deposit_fee: u64,
    pub withdraw_fee: u64,
    pub fees_fee: u64,
    pub reward_0_fee: u64,
    pub reward_1_fee: u64,
    pub reward_2_fee: u64,

    // Timestamp when current position was opened.
    pub position_timestamp: u64,
    pub kamino_rewards: [KaminoRewardInfo; 3],

    pub strategy_dex: u64, // enum for strat ORCA=0, RAYDIUM=1, CREMA=2
    pub raydium_protocol_position_or_base_vault_authority: Pubkey,
    pub allow_deposit_without_invest: u64,
    pub raydium_pool_config_or_base_vault_authority: Pubkey,

    pub deposit_blocked: u8,
    // a strategy creation can be IGNORED=0, SHADOW=1, LIVE=2, DEPRECATED=3, STAGING=4
    // check enum CreationStatus
    pub creation_status: u8,
    pub invest_blocked: u8,
    /// share_calculation_method can be either DOLAR_BASED=0 or PROPORTION_BASED=1
    pub share_calculation_method: u8,
    pub withdraw_blocked: u8,
    pub reserved_flag_2: u8,
    pub local_admin_blocked: u8,
    pub flash_vault_swap_allowed: u8,

    // Reference price saved when initializing a rebalance or emergency swap
    // Used to ensure that prices does not shift during a rebalance/emergency swap
    pub reference_swap_price_a: KaminoPrice,
    pub reference_swap_price_b: KaminoPrice,

    pub is_community: u8,
    pub rebalance_type: u8,
    pub padding_0: [u8; 6],
    pub rebalance_raw: RebalanceRaw,
    pub padding_1: [u8; 7],
    // token_a / token_b _fees_from_rewards_cumulative represents the rewards that are token_a/token_b and are collected directly in the token vault
    pub token_a_fees_from_rewards_cumulative: u64,
    pub token_b_fees_from_rewards_cumulative: u64,
    pub strategy_lookup_table: Pubkey,
    pub padding_3: [u128; 26],
    pub padding_4: [u128; 32],
    pub padding_5: [u128; 32],
    pub padding_6: [u128; 32],
}

impl WhirlpoolStrategy {
    pub fn from_account<'info>(
        account: &'info AccountInfo,
    ) -> ScopeResult<Ref<'info, WhirlpoolStrategy>> {
        zero_copy_deserialize(account)
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq)]
pub struct RebalanceRaw {
    pub params: [u8; SIZE_REBALANCE_PARAMS],
    pub state: [u8; SIZE_REBALANCE_STATE],
    pub reference_price_type: u8,
}

impl Default for RebalanceRaw {
    fn default() -> Self {
        Self {
            params: [0; SIZE_REBALANCE_PARAMS],
            state: [0; SIZE_REBALANCE_STATE],
            reference_price_type: 0,
        }
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, PartialEq, Eq)]
pub struct KaminoRewardInfo {
    pub decimals: u64,
    pub reward_vault: Pubkey,
    pub reward_mint: Pubkey,
    pub reward_collateral_id: u64,

    pub last_issuance_ts: u64,
    pub reward_per_second: u64,
    pub amount_uncollected: u64,
    pub amount_issued_cumulative: u64,
    pub amount_available: u64,
}

impl KaminoRewardInfo {
    pub fn initialized(&self) -> bool {
        self.reward_mint.ne(&Pubkey::default()) && self.decimals > 0
    }
}

#[account(zero_copy)]
#[derive(Debug)]
pub struct GlobalConfig {
    pub emergency_mode: u64,
    pub block_deposit: u64,
    pub block_invest: u64,
    pub block_withdraw: u64,
    pub block_collect_fees: u64,
    pub block_collect_rewards: u64,
    pub block_swap_rewards: u64,
    pub block_swap_uneven_vaults: u32,
    pub block_emergency_swap: u32,
    pub fees_bps: u64,
    pub scope_program_id: Pubkey,
    pub scope_price_id: Pubkey,

    // 128 types of tokens, indexed by token
    pub swap_rewards_discount_bps: [u64; 256],
    // actions_authority is an allowed entity (the bot) that has permissions to perform some permissioned actions
    pub actions_authority: Pubkey,
    pub admin_authority: Pubkey,
    pub treasury_fee_vaults: [Pubkey; 256],

    pub token_infos: Pubkey,
    pub block_local_admin: u64,
    pub min_performance_fee_bps: u64,

    pub _padding: [u64; 2042],
}

impl GlobalConfig {
    pub fn from_account<'info>(
        account: &'info AccountInfo,
    ) -> ScopeResult<Ref<'info, GlobalConfig>> {
        zero_copy_deserialize(account)
    }
}

impl Default for GlobalConfig {
    #[inline(never)]
    fn default() -> GlobalConfig {
        let vaults: [Pubkey; 256] = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

        GlobalConfig {
            emergency_mode: 0,
            block_deposit: 0,
            block_invest: 0,
            block_withdraw: 0,
            block_collect_fees: 0,
            block_collect_rewards: 0,
            block_swap_rewards: 0,
            block_swap_uneven_vaults: 0,
            block_emergency_swap: 0,
            fees_bps: 0,
            scope_program_id: Pubkey::default(),
            scope_price_id: Pubkey::default(),
            swap_rewards_discount_bps: [0; 256],
            actions_authority: Pubkey::default(),
            admin_authority: Pubkey::default(),
            token_infos: Pubkey::default(),
            treasury_fee_vaults: vaults,
            block_local_admin: 0,
            min_performance_fee_bps: 0,
            _padding: [0; 2042],
        }
    }
}

#[account(zero_copy)]
#[derive(Debug, AnchorSerialize)]
pub struct CollateralInfos {
    pub infos: [CollateralInfo; 256],
}

impl CollateralInfos {
    pub fn default() -> Self {
        Self {
            infos: [CollateralInfo::default(); 256],
        }
    }
}

impl CollateralInfos {
    pub fn get_price(
        &self,
        prices: &OraclePrices,
        token_id: usize,
    ) -> std::result::Result<DatedPrice, ScopeChainError> {
        let chain = self
            .infos
            .get(token_id)
            .ok_or(ScopeChainError::NoChainForToken)?
            .scope_price_chain;

        scope_chain::get_price_from_chain(prices, &chain)
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq)]
pub struct CollateralInfo {
    // The index is the collateral_id
    pub mint: Pubkey,
    pub lower_heuristic: u64,
    pub upper_heuristic: u64,
    pub exp_heuristic: u64,
    pub max_twap_divergence_bps: u64,
    // This is the scope_id twap, unlike scope_price_chain, it's a single value
    // and it's always a dollar denominated (twap)
    pub scope_price_id_twap: u64,
    // This is the scope_id price chain that results in a price for the token
    pub scope_price_chain: [u16; 4],
    pub name: [u8; 32],
    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,
    pub max_ignorable_amount_as_reward: u64, // 0 means the rewards in this token can be always ignored
    pub disabled: u8,
    pub _padding0: [u8; 7],
    pub _padding: [u64; 9],
}

impl Default for CollateralInfo {
    #[inline]
    fn default() -> CollateralInfo {
        CollateralInfo {
            mint: Pubkey::default(),
            lower_heuristic: u64::default(),
            upper_heuristic: u64::default(),
            exp_heuristic: u64::default(),
            max_twap_divergence_bps: u64::default(),
            scope_price_id_twap: u64::MAX,
            scope_price_chain: [u16::MAX; 4],
            name: [0; 32],
            max_age_price_seconds: 0,
            max_age_twap_seconds: 0,
            max_ignorable_amount_as_reward: 0,
            disabled: 0,
            _padding0: [0; 7],
            _padding: [0; 9],
        }
    }
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Default)]
pub struct KaminoPrice {
    // Pyth price, integer + exponent representation
    // decimal price would be
    // as integer: 6462236900000, exponent: 8
    // as float:   64622.36900000

    // value is the scaled integer
    // for example, 6462236900000 for btc
    pub value: u64,

    // exponent represents the number of decimals
    // for example, 8 for btc
    pub exp: u64,
}

impl KaminoPrice {
    pub fn is_zero(&self) -> bool {
        self.value == 0
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct CollateralToken(u64);

impl CollateralToken {
    pub fn to_usize(self) -> usize {
        let amount: u64 = self.0;
        usize::try_from(amount).unwrap()
    }
}

impl From<u8> for CollateralToken {
    fn from(collateral_id: u8) -> Self {
        CollateralToken(u64::from(collateral_id))
    }
}

impl From<CollateralToken> for u64 {
    fn from(val: CollateralToken) -> Self {
        val.0
    }
}

impl From<u64> for CollateralToken {
    fn from(collateral_id: u64) -> Self {
        CollateralToken(collateral_id)
    }
}

#[derive(Debug)]
pub struct TokenPrices {
    pub prices: [Option<KaminoPrice>; 128],
}

impl Default for TokenPrices {
    fn default() -> TokenPrices {
        TokenPrices {
            prices: [None; 128],
        }
    }
}

impl TokenPrices {
    pub fn get(&self, token: impl TryInto<CollateralToken>) -> ScopeResult<KaminoPrice> {
        let token: CollateralToken = token
            .try_into()
            .map_err(|_| ScopeError::ConversionFailure)?;
        let res = self.prices[token.to_usize()];
        match res {
            Some(price) => Ok(price),
            None => {
                #[cfg(target_arch = "bpf")]
                msg!(
                    "Trying to get price for {:?} [{}] failed, w prices {:?}",
                    token,
                    token.to_usize(),
                    self
                );
                Err(ScopeError::PriceNotValid)
            }
        }
    }

    pub fn set(
        &mut self,
        token: impl Into<CollateralToken>,
        price: impl Into<Option<KaminoPrice>>,
    ) {
        self.prices[token.into().to_usize()] = price.into();
    }

    /// We calculate up to USD_DECIMALS_PRECISION (as exponent)
    pub fn get_market_value_of_token(
        &self,
        token: impl Into<CollateralToken>,
        amount: u64,
        token_decimals: u8,
    ) -> ScopeResult<U128> {
        let token: CollateralToken = token.into();
        let price = self.get(token)?;
        calc_market_value_token_usd(amount, &price, token_decimals)
    }
}

pub mod scope {
    use std::ops::Sub;

    use solana_program::clock;

    use super::*;

    /// Extract the price of a token from the provided Scope price account
    ///
    /// Warning: This API requires prevalidation of the account
    pub fn get_price_usd_twap_unchecked(
        scope_prices: &OraclePrices,
        token: impl TryInto<ScopeConversionChain>,
        current_slot: clock::Slot,
        max_age: u64,
    ) -> Result<KaminoPrice> {
        let tokens_chain: ScopeConversionChain = token
            .try_into()
            .map_err(|_| dbg_msg!(ScopeError::BadScopeChainOrPrices))?;

        // Collect here to avoid revalidating the prices for all operation
        let price_chain = tokens_chain
            .iter()
            .map(|&token_id| get_price(scope_prices, token_id, current_slot, max_age))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Early return if there is only one price
        if price_chain.len() == 1 {
            return Ok(price_chain[0]);
        }

        let total_decimals: u64 = price_chain
            .iter()
            .try_fold(0u64, |acc, price| acc.checked_add(price.exp))
            .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?;

        // Final number of decimals is the last element one's which should be the quotation price.
        let exp = price_chain.last().unwrap().exp; // chain is never empty by construction

        // Compute token value by multiplying all value of the chain
        let product = price_chain
            .iter()
            .try_fold(U128::from(1u128), |acc, price| {
                acc.checked_mul(price.value.into())
            })
            .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?;

        // Compute final value by removing extra decimals
        let scale_down_decimals: u32 = total_decimals.checked_sub(exp).unwrap().try_into().unwrap(); // Cannot fail by construction of `total_decimals`
        let scale_down_factor = U128::from(10u128)
            .checked_pow(U128::from(scale_down_decimals))
            .unwrap();
        let value = product.checked_div(scale_down_factor).unwrap(); // Cannot fail thanks to the early return

        auto_scale_u128_price(value, exp)
    }

    /// Automatically scale down a value on U128 to be stored in a `Price`
    /// by reducing the `exp` factor.
    ///
    /// Heuristic: returned `exp` should be at least 8 and received `exp`
    /// should be at most 15. We cover reducing factors of at most 10^7
    /// We expect the price to always fit
    fn auto_scale_u128_price(value: U128, exp: u64) -> Result<KaminoPrice> {
        let (scale_down_decimals, scale_down_factor) = match value.0[1] {
            // No need to scale
            0 => {
                return Ok(KaminoPrice {
                    value: value.0[0],
                    exp,
                })
            }
            1..=10 => (1, 10),
            11..=100 => (2, 100),
            101..=1_000 => (3, 1_000),
            1_001..=10_000 => (4, 10_000),
            10_001..=100_000 => (5, 100_000),
            100_001..=1_000_000 => (6, 1_000_000),
            1_000_001..=10_000_000 => (7, 10_000_000),
            _ => return err!(ScopeError::IntegerOverflow),
        };
        let value: u64 = (value / U128::from(scale_down_factor)) // Cannot fail thanks choice of factors
            .try_into()
            .unwrap(); // This should not happen or we should have returned the error earlier
        let exp = exp
            .checked_sub(scale_down_decimals)
            .ok_or_else(|| error!(ScopeError::MathOverflow))?;
        Ok(KaminoPrice { value, exp })
    }

    pub fn seconds_to_slots(seconds: u64) -> clock::Slot {
        seconds
            .checked_mul(1000)
            .unwrap()
            .checked_div(clock::DEFAULT_MS_PER_SLOT)
            .unwrap()
    }

    pub fn get_twap_prices_from_data(
        scope_prices: &OraclePrices,
        token_infos: &[CollateralInfo],
        strategy: &WhirlpoolStrategy,
    ) -> Result<Prices> {
        let token_a = CollateralToken::try_from(strategy.token_a_collateral_id)
            .map_err(|_| ScopeError::ConversionFailure)?;
        let token_b = CollateralToken::try_from(strategy.token_b_collateral_id)
            .map_err(|_| ScopeError::ConversionFailure)?;
        Ok(Prices {
            a: <crate::Price as Into<KaminoPrice>>::into(
                get_twap(scope_prices, token_a, token_infos)?.0.price,
            ),
            b: <crate::Price as Into<KaminoPrice>>::into(
                get_twap(scope_prices, token_b, token_infos)?.0.price,
            ),
        })
    }

    pub fn get_prices_from_data(
        scope_prices: &OraclePrices,
        token_infos: &[CollateralInfo],
        strategy: &WhirlpoolStrategy,
        clmm: Option<&dyn Clmm>,
        slot: u64,
    ) -> ScopeResult<Box<TokenPrices>> {
        let token_a = CollateralToken::try_from(strategy.token_a_collateral_id)
            .map_err(|_| ScopeError::ConversionFailure)?;
        let token_b = CollateralToken::try_from(strategy.token_b_collateral_id)
            .map_err(|_| ScopeError::ConversionFailure)?;

        let pending_rewards = match clmm {
            Some(clmm) => clmm.get_position_pending_rewards()?,
            None => RewardsAmounts::default(),
        };

        let price_a = get_price_usd(scope_prices, token_infos, token_a, slot)?;
        let price_b = get_price_usd(scope_prices, token_infos, token_b, slot)?;

        let mut prices = Box::<TokenPrices>::default();
        prices.set(token_a, price_a);
        prices.set(token_b, price_b);

        // Poor man's verification but it does the job
        // checking if reward vault is initialized
        if strategy.reward_0_decimals > 0
            && (strategy.reward_0_amount > 0 || pending_rewards.reward_0 > 0)
        {
            let coll = CollateralToken::try_from(strategy.reward_0_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?;
            let price = get_price_usd(scope_prices, token_infos, coll, slot).ok();
            prices.set(coll, price);
        }
        if strategy.reward_1_decimals > 0
            && (strategy.reward_1_amount > 0 || pending_rewards.reward_1 > 0)
        {
            let coll = CollateralToken::try_from(strategy.reward_1_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?;
            let price = get_price_usd(scope_prices, token_infos, coll, slot).ok();
            prices.set(coll, price);
        }
        if strategy.reward_2_decimals > 0
            && (strategy.reward_2_amount > 0 || pending_rewards.reward_2 > 0)
        {
            let coll = CollateralToken::try_from(strategy.reward_2_collateral_id)
                .map_err(|_| ScopeError::ConversionFailure)?;
            let price = get_price_usd(scope_prices, token_infos, coll, slot).ok();
            prices.set(coll, price);
        }
        for kamino_reward in strategy.kamino_rewards.iter() {
            // it is possible that the rewards were collected so we have amount_uncollected==0 but after setting the price we will collect again in the deposit_and_invest effects, and if there is an amount available we will have rewards so we will need the price
            if kamino_reward.initialized()
                && (kamino_reward.amount_uncollected > 0 || kamino_reward.amount_available > 0)
            {
                let coll = CollateralToken::try_from(kamino_reward.reward_collateral_id)
                    .map_err(|_| ScopeError::ConversionFailure)?;
                let price = get_price_usd(scope_prices, token_infos, coll, slot).ok();
                prices.set(coll, price);
            }
        }

        Ok(prices)
    }

    pub fn get_price_usd(
        scope_prices: &OraclePrices,
        token_infos: &[CollateralInfo], // TODO: just take the one, no need for token id
        token: impl Into<CollateralToken>,
        current_slot: clock::Slot,
    ) -> Result<KaminoPrice> {
        let token: CollateralToken = token.into();
        let price_max_age = seconds_to_slots(token_infos[token.to_usize()].max_age_price_seconds);
        let twap_max_age = seconds_to_slots(token_infos[token.to_usize()].max_age_twap_seconds);
        let price_label =
            std::str::from_utf8(&token_infos[token.to_usize()].name).unwrap_or("CannotDecodeToken");

        let usd_price = get_price_usd_twap_unchecked(
            scope_prices,
            token_infos[token.to_usize()],
            current_slot,
            price_max_age,
        )?;

        // Check that TWAP is within range

        let acceptable_twap_tolerance_bps = token_infos[token.to_usize()].max_twap_divergence_bps;
        let is_twap_enabled = acceptable_twap_tolerance_bps > 0;

        if is_twap_enabled {
            let (twap, twap_token) = get_twap(scope_prices, token, token_infos)?;
            if is_price_too_old(current_slot, twap.last_updated_slot, twap_max_age) {
                //     xmsg!(
                //     "Twap of {:?} Price is too old token=ScopePrice[{:?}] price.last_updated_slot={:?} current_slot={:?} max_age={:?}",
                //     price_label,
                //     twap_token,
                //     twap.last_updated_slot,
                //     current_slot,
                //     twap_max_age
                // );
                return Err(ScopeError::ConversionFailure.into());
            }

            let (is_acceptable, diff_bps) =
                is_within_tolerance(usd_price, twap.price.into(), acceptable_twap_tolerance_bps);
            if !is_acceptable {
                // xmsg!("Price is too far from TWAP token={:?} price={:?} twap={:?} tolerance_bps={:?} diff={:?}", token, usd_price, twap, acceptable_twap_tolerance_bps, diff_bps);
                return Err(ScopeError::ConversionFailure.into());
            }
        }
        Ok(usd_price)
    }

    /// Extract the price of a token from the provided Scope price account
    ///
    /// Warning: This API requires prevalidation of the account
    pub fn get_price(
        scope_prices: &OraclePrices,
        token: impl TryInto<ScopePriceId>,
        current_slot: clock::Slot,
        max_age: u64,
    ) -> Result<KaminoPrice> {
        let token: ScopePriceId = token
            .try_into()
            .map_err(|_| dbg_msg!(VaultError::IntegerOverflow))?;

        let price = scope_prices.prices[usize::from(token)];

        // Check that the price is not too old
        if is_price_too_old(current_slot, price.last_updated_slot, max_age) {
            // xmsg!(
            // "Price is too old token=ScopePrice[{:?}] price.last_updated_slot={:?} current_slot={:?} max_age={:?}",
            // token,
            // price.last_updated_slot,
            // current_slot,
            // max_age
            // );
            return Err(ScopeError::ConversionFailure.into());
        }
        Ok(price.price.into())
    }

    fn is_price_too_old(
        current_slot: clock::Slot,
        last_updated_slot: clock::Slot,
        max_age: u64,
    ) -> bool {
        let oldest_acceptable_slot = current_slot.saturating_sub(max_age);
        let slot_diff = current_slot.sub(last_updated_slot);
        if oldest_acceptable_slot > last_updated_slot {
            //     xmsg!(
            //     "Price is too old oldest_acceptable_slot={:?}, price.last_updated_slot={:?} diff={:?} max_age={:?}",
            //     oldest_acceptable_slot,
            //     last_updated_slot,
            //     slot_diff,
            //     max_age
            // );
            true
        } else {
            false
        }
    }

    fn get_twap(
        scope_prices: &OraclePrices,
        token: CollateralToken,
        token_infos: &[CollateralInfo],
    ) -> Result<(DatedPrice, ScopePriceId)> {
        let token: usize = token.to_usize();
        let infos = token_infos[token];
        if infos.scope_price_id_twap == u64::MAX {
            return Err(ScopeError::ConversionFailure.into());
        }
        if infos.scope_price_id_twap == 0 {
            return Err(ScopeError::ConversionFailure.into());
        }
        let scope_twap_id = u16::try_from(infos.scope_price_id_twap).unwrap();
        let scope_twap_id = ScopePriceId(scope_twap_id);
        let twap: usize = scope_twap_id.into();
        Ok((scope_prices.prices[twap], scope_twap_id))
    }

    fn to_scaled_normalized(price: KaminoPrice, target_exp: u64) -> u64 {
        // Cast
        let extra_exp = i64::try_from(target_exp).unwrap() - i64::try_from(price.exp).unwrap();

        let px = i128::from(price.value);
        let exp = u32::try_from(extra_exp).unwrap();

        let result = px.checked_mul(10_i128.pow(exp)).unwrap();
        u64::try_from(result).unwrap()
    }

    fn diff_in_bps(price: KaminoPrice, twap: KaminoPrice) -> u64 {
        const MIN_EXP: u64 = 11;
        let target_exp = u64::max(MIN_EXP, u64::max(price.exp, twap.exp));
        let price_scaled = to_scaled_normalized(price, target_exp);
        let twap_scaled = to_scaled_normalized(twap, target_exp);

        let abs_diff = u128::try_from(i128::abs(
            i128::from(price_scaled)
                .checked_sub(i128::from(twap_scaled))
                .unwrap(),
        ))
        .unwrap();

        let diff_bps = abs_diff
            .checked_mul(10_000)
            .unwrap()
            .checked_div(u128::from(price_scaled))
            .unwrap();

        u64::try_from(diff_bps).unwrap()
    }

    fn is_within_tolerance(
        px: KaminoPrice,
        twap: KaminoPrice,
        acceptable_tolerance_bps: u64,
    ) -> (bool, u64) {
        let diff_bps = diff_in_bps(twap, px);
        (diff_bps < acceptable_tolerance_bps, diff_bps)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TokenAmounts {
    pub a: u64,
    pub b: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Default, Copy)]
pub struct RewardsAmounts {
    pub reward_0: u64,
    pub reward_1: u64,
    pub reward_2: u64,
    pub reward_3: u64,
    pub reward_4: u64,
    pub reward_5: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

#[derive(Debug)]
pub struct Holdings {
    pub available: TokenAmounts,
    pub available_usd: U128,
    pub invested: TokenAmounts,
    pub invested_usd: U128,
    pub fees: TokenAmounts,
    pub fees_usd: U128,
    pub rewards: RewardsAmounts,
    pub rewards_usd: U128,
    pub total_sum: U128,
}

#[derive(TryFromPrimitive, Debug, PartialEq, Eq, Clone, Copy, IntoPrimitive)]
#[repr(u64)]
pub enum DEX {
    Orca = 0,
    Raydium = 1,
}

mod price_utils {
    use std::cmp::Ordering;

    use super::*;
    use crate::Price;

    // Helper
    fn sub(a: u64, b: u64) -> ScopeResult<u32> {
        let res = a.checked_sub(b).ok_or(ScopeError::IntegerOverflow)?;
        u32::try_from(res).map_err(|_e| ScopeError::IntegerOverflow)
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

    fn decimals_factor(decimals_a: u64, decimals_b: u64) -> ScopeResult<(U128, u64)> {
        let decimals_a = i32::try_from(decimals_a).map_err(|_e| ScopeError::IntegerOverflow)?;
        let decimals_b = i32::try_from(decimals_b).map_err(|_e| ScopeError::IntegerOverflow)?;

        let diff = abs_diff(decimals_a, decimals_b);
        let factor = U128::from(10_u64.pow(diff));
        Ok((factor, u64::from(diff)))
    }

    pub fn a_to_b(a: Price, b: Price) -> ScopeResult<Price> {
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

        Ok(Price {
            value: price_a_to_b.as_u64(),
            exp,
        })
    }

    pub fn calc_sqrt_price_from_scope_price(
        price: Price,
        decimals_a: u64,
        decimals_b: u64,
    ) -> ScopeResult<u128> {
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
        price_a: Price,
        price_b: Price,
        decimals_a: u64,
        decimals_b: u64,
    ) -> ScopeResult<u128> {
        calc_sqrt_price_from_scope_price(a_to_b(price_a, price_b)?, decimals_a, decimals_b)
    }

    pub fn calc_price_from_sqrt_price(price: u128, decimals_a: u64, decimals_b: u64) -> f64 {
        let sqrt_price_x_64 = price as f64;
        (sqrt_price_x_64 / 2.0_f64.powf(64.0)).powf(2.0)
            * 10.0_f64.pow(decimals_a as i32 - decimals_b as i32)
    }

    // We calculate the value of any tokens to USD
    // Since all tokens are quoted to USD
    // We calculate up to USD_DECIMALS_PRECISION (as exponent)
    pub fn calc_market_value_token_usd<'a>(
        amount: u64,
        price: impl Into<Option<&'a KaminoPrice>>,
        token_decimals: u8,
    ) -> ScopeResult<U128> {
        let price: Option<&KaminoPrice> = price.into();
        // Don't check if the price is valid unless we really need it (amount > 0)
        if amount == 0 {
            return Ok(U128::from(0_u128));
        }

        let price = price.ok_or(ScopeError::PriceNotValid)?;

        if price.is_zero() {
            return Ok(U128::from(0_u128));
        }

        let numerator = U128::from(amount)
            .checked_mul(U128::from(price.value))
            .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?;

        let total_decimals = token_decimals
            .checked_add(u8::try_from(price.exp).unwrap())
            .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?;

        Ok(match total_decimals.cmp(&USD_DECIMALS_PRECISION) {
            Ordering::Less => {
                let factor = ten_pow(
                    USD_DECIMALS_PRECISION
                        .checked_sub(total_decimals)
                        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?,
                );

                numerator
                    .checked_mul(U128::from(factor))
                    .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
            }

            Ordering::Equal => numerator,
            Ordering::Greater => {
                let factor = ten_pow(
                    total_decimals
                        .checked_sub(USD_DECIMALS_PRECISION)
                        .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?,
                );
                numerator
                    .checked_div(U128::from(factor))
                    .ok_or_else(|| dbg_msg!(ScopeError::IntegerOverflow))?
            }
        })
    }
}

pub mod op_utils {
    use super::*;

    pub fn strategy_reward_decimals(
        strategy: &WhirlpoolStrategy,
        reward_index: u8,
    ) -> ScopeResult<u8> {
        let reward_index = usize::from(reward_index);
        match reward_index {
            0 => Ok(strategy.reward_0_decimals as u8),
            1 => Ok(strategy.reward_1_decimals as u8),
            2 => Ok(strategy.reward_2_decimals as u8),
            ORCA_REWARDS_COUNT..=KAMINO_REWARDS_UPPER_INDEX => Ok(strategy.kamino_rewards
                [reward_index.saturating_sub(ORCA_REWARDS_COUNT)]
            .decimals
            .try_into()
            .unwrap()),
            _ => Err(ScopeError::ConversionFailure),
        }
    }
}

#[cfg(test)]
mod tests {
    use num::traits::Pow;

    use super::price_utils::sqrt_price_from_scope_prices;
    use crate::{
        oracles::ktokens::kamino::price_utils::{
            a_to_b, calc_price_from_sqrt_price, calc_sqrt_price_from_scope_price,
        },
        Price,
    };

    pub fn calc_sqrt_price_from_float_price(price: f64, decimals_a: u64, decimals_b: u64) -> u128 {
        let px = (price * 10.0_f64.pow(decimals_b as i32 - decimals_a as i32)).sqrt();
        (px * 2.0_f64.powf(64.0)) as u128
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

        let a_to_b_price = a_to_b(token_a_price, token_b_price);
        println!("a_to_b_price: {a_to_b_price:?}");

        // assert_eq!(sqrt_price_from_scope_price(scope_price), sqrt_price);
    }

    #[test]

    fn test_sqrt_price_from_float() {
        let price = 1.0;
        let px1 = calc_sqrt_price_from_float_price(price, 6, 6);
        let px2 = calc_sqrt_price_from_float_price(price, 9, 9);
        let px3 = calc_sqrt_price_from_float_price(price, 6, 9);
        let px4 = calc_sqrt_price_from_float_price(price, 9, 6);

        println!("px1: {px1}");
        println!("px2: {px2}");
        println!("px3: {px3}");
        println!("px4: {px4}");
    }

    #[test]

    fn test_sqrt_price_from_price() {
        let px = Price {
            value: 1_000_000_000,
            exp: 9,
        };

        // sqrt_price_from_price = (price * 10 ^ (decimals_b - decimals_a)).sqrt() * 2 ^ 64;

        let x = calc_sqrt_price_from_scope_price(px, 6, 6).unwrap();
        let y = calc_sqrt_price_from_float_price(f(px), 6, 6);

        println!("x: {x}");
        println!("y: {y}");

        for (decimals_a, decimals_b) in
            [(1, 10), (6, 6), (9, 6), (6, 9), (9, 9), (10, 1)].into_iter()
        {
            let x = calc_sqrt_price_from_float_price(f(px), decimals_a, decimals_b);
            let y = calc_sqrt_price_from_scope_price(px, decimals_a, decimals_b).unwrap();

            let px_x = calc_price_from_sqrt_price(x, decimals_a, decimals_b);
            let px_y = calc_price_from_sqrt_price(y, decimals_a, decimals_b);

            let diff = (px_x - px_y).abs();
            println!("x: {x}, y: {y} diff: {diff}");
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
        let a = p(a, decimals_a);
        let b = p(b, decimals_b);
        let actual = sqrt_price_from_scope_prices(a, b, decimals_a, decimals_b).unwrap();

        println!("expected: {expected}");
        println!("actual: {actual}");

        println!(
            "initial: {}, final: {}",
            price,
            calc_price_from_sqrt_price(actual, decimals_a, decimals_b)
        );
    }

    fn run_test(decimals_a: i32, decimals_b: i32, ua: i32, ub: i32) -> Option<f64> {
        let price_float_factor = 10_000.0;
        let fa = ua as f64 / price_float_factor; // float a
        let fb = ub as f64 / price_float_factor; // float b
        let decimals_a = u64::try_from(decimals_a).unwrap();
        let decimals_b = u64::try_from(decimals_b).unwrap();

        let sa = p(fa, decimals_a); // scope a
        let sb = p(fb, decimals_b); // scope b

        println!("uA: {ua}, uB: {ub}");
        println!("fA: {fa}, fB: {fb}");
        println!("sA: {sa:?}, sB: {sb:?}");
        println!("dA: {decimals_a}, dB: {decimals_b}");

        if sa.value == 0 || sb.value == 0 {
            return None;
        }

        let price = fa / fb;

        let expected = calc_sqrt_price_from_float_price(price, decimals_a, decimals_b);

        // Now go the other way around

        let actual = sqrt_price_from_scope_prices(sa, sb, decimals_a, decimals_b).unwrap();

        println!("expected: {expected}");
        println!("actual: {actual}");

        let float_expected = price;
        let float_actual = calc_price_from_sqrt_price(actual, decimals_a, decimals_b);
        let float_diff = (float_expected - float_actual).abs() / float_expected;
        println!(
            "initial: {}, final: {}, diff: {}%",
            float_expected,
            float_actual,
            float_diff * 100.0
        );
        Some(float_diff)
    }

    #[test]
    fn scope_prices_to_sqrt_prices_prop_single() {
        let decimals_a = 11;
        let decimals_b = 7;

        let a = 1;
        let b = 1048;

        if let Some(diff) = run_test(decimals_a, decimals_b, a, b) {
            assert!(diff < 0.001);
        } else {
            println!("Test result dismissed");
        }
    }

    use proptest::{prelude::*, test_runner::Reason};
    proptest! {
        #[test]
        fn scope_prices_to_sqrt_prices_prop_gen(
            decimals_a in 2..12,
            decimals_b in 2..12,
            a in 1..200_000_000,
            b in 1..200_000_000,
        ) {

            if let Some(float_diff) = run_test(decimals_a, decimals_b, a, b) {
                prop_assert!(float_diff < 0.001, "float_diff: {}", float_diff);
            } else {
                return Err(TestCaseError::Reject(Reason::from("Bad input")));
            }
        }
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
                sqrt_price_from_scope_prices(price_a, price_b, decimals_a, decimals_b).unwrap();

            let expected = calc_price_from_sqrt_price(expected_sqrt, decimals_a, decimals_b);
            let actual = calc_price_from_sqrt_price(actual, decimals_a, decimals_b);
            let diff_pct = (actual - expected) / expected * 100.0;
            println!("expected_sqrt: {expected_sqrt}");
            println!("actual: {actual}");
            println!("expected: {expected}");
            println!("diff: {diff_pct}%");
            println!("---");
            assert!(diff_pct.abs() < tolerance) // 0.07% diff
        }
    }
}
