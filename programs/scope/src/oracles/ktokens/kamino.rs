use std::{cell::Ref, convert::TryInto};

use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    *,
};
use decimal_wad::{
    common::{TryDiv, TryMul},
    decimal::{Decimal, U192},
    rate::U128,
};
use num::traits::Pow;
use whirlpool::math::sqrt_price_from_tick_index;
pub use whirlpool::state::{Position, PositionRewardInfo, Whirlpool, WhirlpoolRewardInfo};

use crate::{
    oracles::ktokens::kamino::price_utils::calc_price_from_sqrt_price, scope_chain,
    scope_chain::ScopeChainError, utils::zero_copy_deserialize, DatedPrice, OraclePrices,
    ScopeError, ScopeResult,
};

const TARGET_EXPONENT: u64 = 12;
const SIZE_REBALANCE_PARAMS: usize = 128;
const SIZE_REBALANCE_STATE: usize = 256;

use super::USD_DECIMALS_PRECISION;

pub fn get_price_per_full_share(
    strategy: &WhirlpoolStrategy,
    whirlpool: &Whirlpool,
    position: &Position,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let holdings = holdings(strategy, whirlpool, position, prices)?;

    let shares_issued = strategy.shares_issued;
    let shares_decimals = strategy.shares_mint_decimals;

    if shares_issued == 0 {
        Ok(U128::from(0_u128))
    } else {
        Ok(Decimal::from(underlying_unit(shares_decimals))
            .try_mul(holdings)?
            .try_div(shares_issued)?
            .try_ceil()?)
    }
}

fn holdings(
    strategy: &WhirlpoolStrategy,
    whirlpool: &Whirlpool,
    position: &Position,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let available = amounts_available(strategy);

    let decimals_a = strategy.token_a_mint_decimals;
    let decimals_b = strategy.token_b_mint_decimals;

    // https://github.com/0xparashar/UniV3NFTOracle/blob/master/contracts/UniV3NFTOracle.sol#L27
    // We are using the sqrt price derived from price_a and price_b
    // instead of the whirlpool price which could be manipulated/stale
    let sqrt_price_from_oracle = price_utils::sqrt_price_from_scope_prices(
        prices.price_a.price,
        prices.price_b.price,
        decimals_a,
        decimals_b,
    )?;

    if cfg!(feature = "debug") {
        let w = calc_price_from_sqrt_price(whirlpool.sqrt_price, decimals_a, decimals_b);
        let o = calc_price_from_sqrt_price(sqrt_price_from_oracle, decimals_a, decimals_b);
        let diff = (w - o).abs() / w;
        msg!("o: {} w: {} d: {}%", w, o, diff * 100.0);
    }

    let invested = amounts_invested(position, sqrt_price_from_oracle);
    // We want the minimum price we would get in the event of a liquidation so ignore pending fees and pending rewards

    let available_usd = amounts_usd(strategy, &available, prices)?;

    let invested_usd = amounts_usd(strategy, &invested, prices)?;

    let total_sum = available_usd
        .checked_add(invested_usd)
        .ok_or(ScopeError::IntegerOverflow)?;

    Ok(total_sum)
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

// We calculate the value of any tokens to USD
// Since all tokens are quoted to USD
// We calculate up to USD_DECIMALS_PRECISION (as exponent)
fn amounts_usd_token(
    strategy: &WhirlpoolStrategy,
    token_amount: u64,
    is_a: bool,
    prices: &TokenPrices,
) -> ScopeResult<U128> {
    let (price, token_mint_decimals) = match is_a {
        true => (prices.price_a.price, strategy.token_a_mint_decimals),
        false => (prices.price_b.price, strategy.token_b_mint_decimals),
    };
    let token_mint_decimal = u8::try_from(token_mint_decimals)?;

    if token_amount == 0 {
        return Ok(U128::from(0_u128));
    }

    U128::from(token_amount)
        .checked_mul(U128::from(price.value))
        .ok_or(ScopeError::MathOverflow)?
        .checked_div(ten_pow(
            token_mint_decimal
                .checked_add(price.exp.try_into()?)
                .ok_or(ScopeError::MathOverflow)?
                .checked_sub(USD_DECIMALS_PRECISION)
                .ok_or(ScopeError::MathOverflow)?,
        ))
        .ok_or(ScopeError::MathOverflow)
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

fn amounts_invested(position: &Position, pool_sqrt_price: u128) -> TokenAmounts {
    let (a, b) = if position.liquidity > 0 {
        let sqrt_price_lower = sqrt_price_from_tick_index(position.tick_lower_index);
        let sqrt_price_upper = sqrt_price_from_tick_index(position.tick_upper_index);

        let (delta_a, delta_b) = get_amounts_for_liquidity(
            pool_sqrt_price,
            sqrt_price_lower,
            sqrt_price_upper,
            position.liquidity,
        );

        (delta_a, delta_b)
    } else {
        (0, 0)
    };

    TokenAmounts { a, b }
}

fn get_amounts_for_liquidity(
    current_sqrt_price: u128,
    mut sqrt_price_a: u128,
    mut sqrt_price_b: u128,
    liquidity: u128,
) -> (u64, u64) {
    if sqrt_price_a > sqrt_price_b {
        std::mem::swap(&mut sqrt_price_a, &mut sqrt_price_b)
    }

    let (mut amount0, mut amount1) = (0, 0);
    if current_sqrt_price < sqrt_price_a {
        amount0 = get_amount_a_for_liquidity(sqrt_price_a, sqrt_price_b, liquidity);
    } else if current_sqrt_price < sqrt_price_b {
        amount0 = get_amount_a_for_liquidity(current_sqrt_price, sqrt_price_b, liquidity);
        amount1 = get_amount_b_for_liquidity(sqrt_price_a, current_sqrt_price, liquidity);
    } else {
        amount1 = get_amount_b_for_liquidity(sqrt_price_a, sqrt_price_b, liquidity);
    }

    (amount0 as u64, amount1 as u64)
}

fn get_amount_a_for_liquidity(
    mut sqrt_price_a: u128,
    mut sqrt_price_b: u128,
    liquidity: u128,
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
    numerator.checked_div(denominator).unwrap().as_u128()
}

fn get_amount_b_for_liquidity(
    mut sqrt_price_a: u128,
    mut sqrt_price_b: u128,
    liquidity: u128,
) -> u128 {
    if sqrt_price_a > sqrt_price_b {
        std::mem::swap(&mut sqrt_price_a, &mut sqrt_price_b)
    }

    let q64 = U192::from(2_u128.pow(64));

    let sqrt_price_a = U192::from(sqrt_price_a);
    let sqrt_price_b = U192::from(sqrt_price_b);
    let diff = sqrt_price_b.checked_sub(sqrt_price_a).unwrap();

    let numerator = U192::from(liquidity).checked_mul(diff).unwrap();
    let result = numerator.checked_div(q64).unwrap();
    result.as_u128()
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

pub struct TokenPrices {
    pub price_a: DatedPrice,
    pub price_b: DatedPrice,
}

impl TokenPrices {
    pub fn compute(
        prices: &OraclePrices,
        collateral_infos: &CollateralInfos,
        strategy: &WhirlpoolStrategy,
    ) -> ScopeResult<TokenPrices> {
        let price_a =
            collateral_infos.get_price(prices, strategy.token_a_collateral_id.try_into()?)?;
        let price_b =
            collateral_infos.get_price(prices, strategy.token_b_collateral_id.try_into()?)?;
        Ok(TokenPrices { price_a, price_b })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TokenAmounts {
    pub a: u64,
    pub b: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RewardsAmounts {
    pub reward_0: u64,
    pub reward_1: u64,
    pub reward_2: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

mod price_utils {
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
