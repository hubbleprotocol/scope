use std::cell::Ref;

use anchor_lang::prelude::*;

use decimal_wad::common::{TryDiv, TryMul};
use decimal_wad::decimal::{Decimal, U192};
use decimal_wad::rate::U128;

use whirlpool::math::sqrt_price_from_tick_index;
pub use whirlpool::state::{Position, PositionRewardInfo, Whirlpool, WhirlpoolRewardInfo};

use crate::scope_chain::ScopeChainAccount;
use crate::utils::zero_copy_deserialize;
use crate::{DatedPrice, OraclePrices, ScopeError, ScopeResult};

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
        Ok(Decimal::from(underlying_unit(shares_decimals).as_u128())
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
    let invested = amounts_invested(whirlpool, position);
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
    U128::from(10_u64.pow(share_decimals as u32))
}

fn amounts_available(strategy: &WhirlpoolStrategy) -> TokenAmounts {
    TokenAmounts {
        a: strategy.token_a_amounts,
        b: strategy.token_b_amounts,
    }
}

fn amounts_invested(whirlpool: &Whirlpool, position: &Position) -> TokenAmounts {
    let (a, b) = if position.liquidity > 0 {
        let sqrt_price_lower = sqrt_price_from_tick_index(position.tick_lower_index);
        let sqrt_price_upper = sqrt_price_from_tick_index(position.tick_upper_index);

        let (delta_a, delta_b) = get_amounts_for_liquidity(
            whirlpool.sqrt_price,
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
pub struct WhirlpoolStrategy {
    // Admin
    pub admin_authority: Pubkey,

    pub global_config: Pubkey,

    // this is an u8 but we need to keep it as u64 for memory allignment
    pub base_vault_authority: Pubkey,
    pub base_vault_authority_bump: u64,

    // Whirlpool info
    pub whirlpool: Pubkey,
    pub whirlpool_token_vault_a: Pubkey,
    pub whirlpool_token_vault_b: Pubkey,

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
    pub swap_uneven_max_slippage: u64,

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

    pub padding_1: [u128; 20],
    pub padding_2: [u128; 32],
    pub padding_3: [u128; 32],
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

pub struct TokenPrices {
    pub price_a: DatedPrice,
    pub price_b: DatedPrice,
}

impl TokenPrices {
    pub fn compute(
        prices: &OraclePrices,
        scope_chain: &ScopeChainAccount,
        strategy: &WhirlpoolStrategy,
    ) -> ScopeResult<TokenPrices> {
        let price_a = scope_chain.get_price(prices, strategy.token_a_collateral_id.try_into()?)?;
        let price_b = scope_chain.get_price(prices, strategy.token_b_collateral_id.try_into()?)?;
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
