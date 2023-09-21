use anchor_lang::prelude::Pubkey;
use decimal_wad::decimal::U192;
use whirlpool::{
    math::checked_mul_div_round_up,
    state::{Position, TickArray, Whirlpool},
};
#[cfg(any(not(target_os = "solana"), feature = "no-entrypoint"))]
use {
    whirlpool::manager::liquidity_manager::ModifyLiquidityUpdate,
    whirlpool::manager::position_manager::next_position_modify_liquidity_update,
    whirlpool::manager::tick_manager::{
        next_fee_growths_inside, next_reward_growths_inside, next_tick_modify_liquidity_update,
    },
    whirlpool::manager::whirlpool_manager::{
        next_whirlpool_liquidity, next_whirlpool_reward_infos,
    },
    whirlpool::state::Tick,
};

use crate::{
    oracles::ktokens::{
        clmm::Clmm,
        kamino::{
            liquidity_calcs::{get_amount_a_for_liquidity, get_amount_b_for_liquidity},
            orca_pending_rewards, RewardsAmounts, TokenAmounts, DEX,
        },
    },
    ScopeError, ScopeResult,
};

pub struct OrcaClmm {
    pub pool: Whirlpool,
    pub position: Option<Position>,
    // These 2 fields should not be used onchain
    pub lower_tick_array: Option<Box<TickArray>>,
    pub upper_tick_array: Option<Box<TickArray>>,
}

impl OrcaClmm {
    pub fn get_position(&self) -> ScopeResult<&Position> {
        self.position.as_ref().ok_or(ScopeError::ConversionFailure)
    }

    #[cfg(any(not(target_os = "solana"), feature = "no-entrypoint"))]
    pub fn get_tick_arrays(&self) -> ScopeResult<(&TickArray, &TickArray)> {
        Ok((
            self.lower_tick_array
                .as_ref()
                .ok_or(ScopeError::ConversionFailure)?,
            self.upper_tick_array
                .as_ref()
                .ok_or(ScopeError::ConversionFailure)?,
        ))
    }

    #[cfg(any(not(target_os = "solana"), feature = "no-entrypoint"))]
    pub fn get_position_after_refresh(&self, current_timestamp: u64) -> ScopeResult<Position> {
        let mut position = self.get_position()?.clone();
        if position.liquidity == 0 {
            return Ok(position);
        }
        let (lower_tick_array, upper_tick_array) = self.get_tick_arrays()?;
        let tick_lower = lower_tick_array
            .get_tick(position.tick_lower_index, self.pool.tick_spacing)
            .map_err(|err| {
                dbg!("Got error while getting orca lower tick {:?}", err);
                ScopeError::ConversionFailure
            })?;
        let tick_upper = upper_tick_array
            .get_tick(position.tick_upper_index, self.pool.tick_spacing)
            .map_err(|err| {
                dbg!("Got error while getting orca upper tick {:?}", err);
                ScopeError::ConversionFailure
            })?;
        let update = calculate_modify_liquidity(
            &self.pool,
            &position,
            tick_lower,
            tick_upper,
            position.tick_lower_index,
            position.tick_upper_index,
            0,
            current_timestamp,
        )
        .map_err(|err| {
            dbg!("Got error while computing orca position update {:?}", err);
            ScopeError::ConversionFailure
        })?;
        position.update(&update.position_update);
        Ok(position)
    }
}

impl Clmm for OrcaClmm {
    fn get_tick_current_index(&self) -> i32 {
        self.pool.tick_current_index
    }

    fn get_current_sqrt_price(&self) -> u128 {
        self.pool.sqrt_price
    }

    fn get_position_tick_lower_index(&self) -> ScopeResult<i32> {
        Ok(self.get_position()?.tick_lower_index)
    }

    fn get_position_tick_upper_index(&self) -> ScopeResult<i32> {
        Ok(self.get_position()?.tick_upper_index)
    }

    fn get_position_pending_fees(&self) -> ScopeResult<TokenAmounts> {
        Ok(TokenAmounts {
            a: self.get_position()?.fee_owed_a,
            b: self.get_position()?.fee_owed_b,
        })
    }

    fn position_has_pending_fees(&self) -> ScopeResult<bool> {
        Ok(self.get_position()?.fee_owed_a > 0 || self.get_position()?.fee_owed_b > 0)
    }

    fn get_position_liquidity(&self) -> ScopeResult<u128> {
        Ok(self.get_position()?.liquidity)
    }

    fn get_position_pending_rewards(&self) -> ScopeResult<RewardsAmounts> {
        Ok(orca_pending_rewards(self.get_position()?, &self.pool))
    }

    fn get_pool_tick_spacing(&self) -> u16 {
        self.pool.tick_spacing
    }

    fn pool_reward_info_initialized(&self, index: usize) -> bool {
        self.pool.reward_infos[index].initialized()
    }

    fn get_pool_reward_vault(&self, index: usize) -> Pubkey {
        self.pool.reward_infos[index].vault
    }

    fn get_pool_vaults(&self) -> (Pubkey, Pubkey) {
        (self.pool.token_vault_a, self.pool.token_vault_b)
    }

    fn get_pool_reward_mint(&self, index: usize) -> Pubkey {
        self.pool.reward_infos[index].mint
    }

    fn get_dex(&self) -> DEX {
        DEX::Orca
    }

    fn sqrt_price_from_tick(&self, tick: i32) -> u128 {
        whirlpool::math::sqrt_price_from_tick_index(tick)
    }

    fn get_liquidity_from_amounts(
        &self,
        sqrt_price_current: u128,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        amount0: u64,
        amount1: u64,
        round_up: Option<bool>,
    ) -> u128 {
        get_liquidity_from_amounts(
            sqrt_price_current,
            sqrt_price_lower,
            sqrt_price_upper,
            amount0,
            amount1,
            round_up,
        )
    }

    fn get_amounts_from_liquidity(
        &self,
        sqrt_price_current: u128,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        liquidity: i128,
    ) -> (u64, u64) {
        get_amounts_from_liquidity(
            sqrt_price_current,
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
        )
    }

    fn get_liquidity_for_amount(
        &self,
        is_token_a: bool,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        amount: u64,
        round_up: Option<bool>,
    ) -> u128 {
        if is_token_a {
            get_liquidity_for_amount_a(
                amount,
                sqrt_price_lower,
                sqrt_price_upper,
                round_up.unwrap(),
            )
            .unwrap()
        } else {
            get_liquidity_for_amount_b(
                amount,
                sqrt_price_lower,
                sqrt_price_upper,
                round_up.unwrap(),
            )
            .unwrap()
        }
    }
}

// Copied from whirlpool program, but there it's not public
#[allow(clippy::too_many_arguments)]
#[cfg(any(not(target_os = "solana"), feature = "no-entrypoint"))]
fn calculate_modify_liquidity(
    whirlpool: &Whirlpool,
    position: &Position,
    tick_lower: &Tick,
    tick_upper: &Tick,
    tick_lower_index: i32,
    tick_upper_index: i32,
    liquidity_delta: i128,
    timestamp: u64,
) -> Result<ModifyLiquidityUpdate, whirlpool::errors::ErrorCode> {
    // Disallow only updating position fee and reward growth when position has zero liquidity
    if liquidity_delta == 0 && position.liquidity == 0 {
        return Err(whirlpool::errors::ErrorCode::LiquidityZero);
    }

    let next_reward_infos = next_whirlpool_reward_infos(whirlpool, timestamp)?;

    let next_global_liquidity = next_whirlpool_liquidity(
        whirlpool,
        position.tick_upper_index,
        position.tick_lower_index,
        liquidity_delta,
    )?;

    let tick_lower_update = next_tick_modify_liquidity_update(
        tick_lower,
        tick_lower_index,
        whirlpool.tick_current_index,
        whirlpool.fee_growth_global_a,
        whirlpool.fee_growth_global_b,
        &next_reward_infos,
        liquidity_delta,
        false,
    )?;

    let tick_upper_update = next_tick_modify_liquidity_update(
        tick_upper,
        tick_upper_index,
        whirlpool.tick_current_index,
        whirlpool.fee_growth_global_a,
        whirlpool.fee_growth_global_b,
        &next_reward_infos,
        liquidity_delta,
        true,
    )?;

    let (fee_growth_inside_a, fee_growth_inside_b) = next_fee_growths_inside(
        whirlpool.tick_current_index,
        tick_lower,
        tick_lower_index,
        tick_upper,
        tick_upper_index,
        whirlpool.fee_growth_global_a,
        whirlpool.fee_growth_global_b,
    );

    let reward_growths_inside = next_reward_growths_inside(
        whirlpool.tick_current_index,
        tick_lower,
        tick_lower_index,
        tick_upper,
        tick_upper_index,
        &next_reward_infos,
    );

    let position_update = next_position_modify_liquidity_update(
        position,
        liquidity_delta,
        fee_growth_inside_a,
        fee_growth_inside_b,
        &reward_growths_inside,
    )?;

    Ok(ModifyLiquidityUpdate {
        whirlpool_liquidity: next_global_liquidity,
        reward_infos: next_reward_infos,
        position_update,
        tick_lower_update,
        tick_upper_update,
    })
}

pub fn get_liquidity_from_amounts(
    current_sqrt_price: u128,
    mut sqrt_price_a: u128,
    mut sqrt_price_b: u128,
    amount_a: u64,
    amount_b: u64,
    round_up: Option<bool>,
) -> u128 {
    let round_up = round_up.unwrap();
    if sqrt_price_a > sqrt_price_b {
        std::mem::swap(&mut sqrt_price_a, &mut sqrt_price_b)
    }

    if current_sqrt_price <= sqrt_price_a {
        get_liquidity_for_amount_a(amount_a, sqrt_price_a, sqrt_price_b, round_up).unwrap()
    } else if current_sqrt_price < sqrt_price_b {
        let liquidity_a =
            get_liquidity_for_amount_a(amount_a, current_sqrt_price, sqrt_price_b, round_up)
                .unwrap();
        let liquidity_b =
            get_liquidity_for_amount_b(amount_b, sqrt_price_a, current_sqrt_price, round_up)
                .unwrap();
        u128::min(liquidity_a, liquidity_b)
    } else {
        get_liquidity_for_amount_b(amount_b, sqrt_price_a, sqrt_price_b, round_up).unwrap()
    }
}

pub fn get_amounts_from_liquidity(
    sqrt_price_current: u128,
    mut sqrt_lower: u128,
    mut sqrt_upper: u128,
    liquidity: i128,
) -> (u64, u64) {
    let liquidity: u128 = liquidity.try_into().unwrap();
    if sqrt_lower > sqrt_upper {
        std::mem::swap(&mut sqrt_lower, &mut sqrt_upper)
    }

    let (mut amount0, mut amount1) = (0, 0);
    let mode = false;
    if sqrt_price_current < sqrt_lower {
        amount0 = get_amount_a_for_liquidity(sqrt_lower, sqrt_upper, liquidity, mode);
    } else if sqrt_price_current < sqrt_upper {
        amount0 = get_amount_a_for_liquidity(sqrt_price_current, sqrt_upper, liquidity, mode);
        amount1 = get_amount_b_for_liquidity(sqrt_lower, sqrt_price_current, liquidity, mode);
    } else {
        amount1 = get_amount_b_for_liquidity(sqrt_lower, sqrt_upper, liquidity, mode);
    }

    (
        u64::try_from(amount0).unwrap(),
        u64::try_from(amount1).unwrap(),
    )
}

fn get_liquidity_for_amount_a(
    amount: u64,
    mut sqrt_price_lower: u128,
    mut sqrt_price_upper: u128,
    round_up: bool,
) -> ScopeResult<u128> {
    if sqrt_price_lower > sqrt_price_upper {
        std::mem::swap(&mut sqrt_price_lower, &mut sqrt_price_upper)
    }

    let sqrt_price_a = U192::from(sqrt_price_lower);
    let sqrt_price_b = U192::from(sqrt_price_upper);
    let q64 = U192::from(2_u128.pow(64));
    let price_diff = sqrt_price_b.checked_sub(sqrt_price_a).unwrap();

    let result = U192::from(amount)
        .checked_mul(sqrt_price_a)
        .unwrap()
        .checked_mul(sqrt_price_b)
        .unwrap()
        .checked_div(price_diff)
        .unwrap();

    let result = if round_up {
        let res = result.checked_div(q64).unwrap();
        if res.div_mod(q64).1 > U192::zero() {
            res + U192::one()
        } else {
            res
        }
    } else {
        result.checked_div(q64).unwrap()
    };
    Ok(result.as_u128())
}

fn get_liquidity_for_amount_b(
    amount: u64,
    mut sqrt_price_lower: u128,
    mut sqrt_price_upper: u128,
    round_up: bool,
) -> ScopeResult<u128> {
    if sqrt_price_lower > sqrt_price_upper {
        std::mem::swap(&mut sqrt_price_lower, &mut sqrt_price_upper)
    }

    let numerator: u128 = u128::from(amount) << 64;
    let denominator: u128 = sqrt_price_upper.checked_sub(sqrt_price_lower).unwrap();

    let res = if round_up {
        checked_mul_div_round_up(numerator, 1, denominator).unwrap()
    } else {
        numerator.checked_div(denominator).unwrap()
    };
    Ok(res)
}

#[cfg(test)]
pub fn calculate_liquidity_token_deltas(
    current_sqrt_price: u128,
    current_tick_index: i32,
    tick_lower_index: i32,
    tick_upper_index: i32,
    liquidity_delta: i128,
) -> (u64, u64) {
    assert!(liquidity_delta != 0);

    let mut delta_a: u64 = 0;
    let mut delta_b: u64 = 0;

    let liquidity: u128 = liquidity_delta.unsigned_abs();
    // round up if you are adding, rown down if you are withdrawing
    let round_up = liquidity_delta > 0;

    let lower_price = whirlpool::math::sqrt_price_from_tick_index(tick_lower_index);
    let upper_price = whirlpool::math::sqrt_price_from_tick_index(tick_upper_index);

    if current_tick_index < tick_lower_index {
        delta_a =
            whirlpool::math::get_amount_delta_a(lower_price, upper_price, liquidity, round_up)
                .unwrap();
    } else if current_tick_index < tick_upper_index {
        delta_a = whirlpool::math::get_amount_delta_a(
            current_sqrt_price,
            upper_price,
            liquidity,
            round_up,
        )
        .unwrap();
        delta_b = whirlpool::math::get_amount_delta_b(
            lower_price,
            current_sqrt_price,
            liquidity,
            round_up,
        )
        .unwrap();
    } else {
        delta_b =
            whirlpool::math::get_amount_delta_b(lower_price, upper_price, liquidity, round_up)
                .unwrap();
    }

    (delta_a, delta_b)
}
