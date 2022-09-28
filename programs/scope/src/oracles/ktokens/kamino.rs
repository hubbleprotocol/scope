use std::cell::Ref;
use std::convert::TryInto;

use anchor_lang::prelude::*;

use decimal_wad::common::{TryDiv, TryMul};
use decimal_wad::decimal::{Decimal, U192};
use decimal_wad::rate::U128;

use whirlpool::math::sqrt_price_from_tick_index;
pub use whirlpool::state::{Position, PositionRewardInfo, Whirlpool, WhirlpoolRewardInfo};

use crate::oracles::ktokens::kamino::price_utils::calc_price_from_sqrt_price;
use crate::scope_chain::ScopeChainAccount;
use crate::utils::zero_copy_deserialize;
use crate::{DatedPrice, OraclePrices, ScopeError, ScopeResult};
use num::traits::Pow;

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

mod price_utils {
    use crate::Price;

    use super::*;

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
        let exp = 12;
        let exp = u64::max(exp, a.exp);
        let exp = u64::max(exp, b.exp);

        let extra_factor_a = 10_u64.pow(sub(exp, a.exp)?);
        let extra_factor_b = 10_u64.pow(sub(exp, b.exp)?);

        let px_a = U128::from(a.value.checked_mul(extra_factor_a).unwrap());
        let px_b = U128::from(b.value.checked_mul(extra_factor_b).unwrap());

        let final_factor = pow(10, exp);

        println!(
            "a_to_b: a:{} b:{} px_a:{} px_b:{} final_factor:{} px_x*ff:{}",
            a.value,
            b.value,
            px_a,
            px_b,
            final_factor,
            px_a.checked_mul(final_factor).unwrap()
        );
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
            (
                U128::from(px.checked_mul(decimals_factor).unwrap()),
                price.exp,
            )
        } else {
            // If we divide by 10 ^ (decimals_a - decimals_b) here we lose precision
            // So instead we lift the price even more (by the diff) and assume a bigger exp
            (
                U128::from(px),
                price.exp.checked_add(decimals_diff).unwrap(),
            )
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

    use crate::{
        oracles::ktokens::kamino::price_utils::{
            a_to_b, calc_price_from_sqrt_price, calc_sqrt_price_from_scope_price,
        },
        Price,
    };

    use super::price_utils::sqrt_price_from_scope_prices;

    pub fn calc_sqrt_price_from_float_price(price: f64, decimals_a: u64, decimals_b: u64) -> u128 {
        let px = (price * 10.0_f64.pow(decimals_b as i32 - decimals_a as i32)).sqrt();
        let res = (px * 2.0_f64.powf(64.0)) as u128;

        println!("calc_sqrt_price_from_float_price: {} {}", price, res);
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

        let a_to_b_price = a_to_b(token_a_price, token_b_price);
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

        let x = calc_sqrt_price_from_scope_price(px, 6, 6).unwrap();
        let y = calc_sqrt_price_from_float_price(f(px), 6, 6);

        println!("x: {}", x);
        println!("y: {}", y);

        for (decimals_a, decimals_b) in
            [(1, 10), (6, 6), (9, 6), (6, 9), (9, 9), (10, 1)].into_iter()
        {
            let x = calc_sqrt_price_from_float_price(f(px), decimals_a, decimals_b);
            let y = calc_sqrt_price_from_scope_price(px, decimals_a, decimals_b).unwrap();

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
        let actual = sqrt_price_from_scope_prices(a, b, decimals_a, decimals_b).unwrap();

        println!("expected: {}", expected);
        println!("actual: {}", actual);

        println!(
            "initial: {}, final: {}",
            price,
            calc_price_from_sqrt_price(actual, decimals_a, decimals_b)
        );
    }

    enum TestResult {
        Res(f64),
        Dismiss,
    }

    fn run_test(decimals_a: i32, decimals_b: i32, ua: i32, ub: i32) -> TestResult {
        let price_float_factor = 10_000.0;
        let fa = ua as f64 / price_float_factor; // float a
        let fb = ub as f64 / price_float_factor; // float b
        let decimals_a = u64::try_from(decimals_a).unwrap();
        let decimals_b = u64::try_from(decimals_b).unwrap();

        let sa = p(fa, decimals_a.into()); // scope a
        let sb = p(fb, decimals_b.into()); // scope b

        println!("uA: {}, uB: {}", ua, ub);
        println!("fA: {}, fB: {}", fa, fb);
        println!("sA: {:?}, sB: {:?}", sa, sb);
        println!("dA: {}, dB: {}", decimals_a, decimals_b);

        if sa.value == 0 || sb.value == 0 {
            return TestResult::Dismiss;
        }

        let price = fa / fb;

        let expected = calc_sqrt_price_from_float_price(price, decimals_a, decimals_b);

        // Now go the other way around

        let actual = sqrt_price_from_scope_prices(sa, sb, decimals_a, decimals_b).unwrap();

        println!("expected: {}", expected);
        println!("actual: {}", actual);

        let float_expected = price;
        let float_actual = calc_price_from_sqrt_price(actual, decimals_a, decimals_b);
        let float_diff = (float_expected - float_actual).abs() / float_expected;
        println!(
            "initial: {}, final: {}, diff: {}%",
            float_expected,
            float_actual,
            float_diff * 100.0
        );
        TestResult::Res(float_diff)
    }

    #[test]
    fn scope_prices_to_sqrt_prices_prop_single() {
        let decimals_a = 11;
        let decimals_b = 7;

        let a = 1;
        let b = 1048;

        if let TestResult::Res(diff) = run_test(decimals_a, decimals_b, a, b) {
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

            if let TestResult::Res(float_diff) = run_test(decimals_a, decimals_b, a, b) {
                prop_assert!(float_diff < 0.001, "float_diff: {}", float_diff);
            } else {
                return Err(TestCaseError::Reject(Reason::from("Bad input")));
            }
        }
    }
}
