use std::ops::Deref;

use anchor_lang::prelude::Pubkey;
use raydium_amm_v3::{
    libraries::liquidity_math,
    states::{
        PersonalPositionState as RaydiumPosition, PoolState, ProtocolPositionState, TickArrayState,
    },
};

use crate::{
    oracles::ktokens::{
        clmm::Clmm,
        kamino::{raydium_pending_rewards, RewardsAmounts, TokenAmounts, DEX},
    },
    ScopeError, ScopeResult,
};

pub struct RaydiumClmm<T> {
    pub pool: T,
    pub position: Option<RaydiumPosition>,
    // These 3 fields should not be used onchain
    pub protocol_position: Option<ProtocolPositionState>,
    pub lower_tick_array: Option<Box<TickArrayState>>,
    pub upper_tick_array: Option<Box<TickArrayState>>,
}

// we need our own pool state wrapper because we can't implement the deref trait in the PoolState and we need to be able to deref a PoolState when we deserialize it in the bot
pub struct WrappedPoolState(pub PoolState);

impl Deref for WrappedPoolState {
    fn deref(&self) -> &PoolState {
        &self.0
    }

    type Target = PoolState;
}

impl<T> RaydiumClmm<T>
where
    T: Deref<Target = PoolState>,
{
    pub fn get_position(&self) -> ScopeResult<&RaydiumPosition> {
        self.position.as_ref().ok_or(ScopeError::ConversionFailure)
    }
}

impl<T> Clmm for RaydiumClmm<T>
where
    T: Deref<Target = PoolState>,
{
    fn get_tick_current_index(&self) -> i32 {
        self.pool.tick_current
    }

    fn get_current_sqrt_price(&self) -> u128 {
        self.pool.sqrt_price_x64
    }

    fn get_position_tick_lower_index(&self) -> ScopeResult<i32> {
        Ok(self.get_position()?.tick_lower_index)
    }

    fn get_position_tick_upper_index(&self) -> ScopeResult<i32> {
        Ok(self.get_position()?.tick_upper_index)
    }

    fn sqrt_price_from_tick(&self, tick: i32) -> u128 {
        raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick).unwrap()
    }

    fn get_position_pending_fees(&self) -> ScopeResult<TokenAmounts> {
        Ok(TokenAmounts {
            a: self.get_position()?.token_fees_owed_0,
            b: self.get_position()?.token_fees_owed_1,
        })
    }

    fn get_position_liquidity(&self) -> ScopeResult<u128> {
        Ok(self.get_position()?.liquidity)
    }

    fn get_position_pending_rewards(&self) -> ScopeResult<RewardsAmounts> {
        Ok(raydium_pending_rewards(self.get_position()?, &self.pool))
    }

    fn get_pool_tick_spacing(&self) -> u16 {
        self.pool.tick_spacing
    }

    fn pool_reward_info_initialized(&self, index: usize) -> bool {
        self.pool.reward_infos[index].initialized()
    }

    fn get_pool_reward_vault(&self, index: usize) -> Pubkey {
        self.pool.reward_infos[index].token_vault
    }

    fn get_pool_reward_mint(&self, index: usize) -> Pubkey {
        self.pool.reward_infos[index].token_mint
    }

    fn get_pool_vaults(&self) -> (Pubkey, Pubkey) {
        (self.pool.token_vault_0, self.pool.token_vault_1)
    }

    fn get_dex(&self) -> DEX {
        DEX::Raydium
    }

    fn position_has_pending_fees(&self) -> ScopeResult<bool> {
        Ok(
            self.get_position()?.token_fees_owed_0 > 0
                || self.get_position()?.token_fees_owed_1 > 0,
        )
    }

    fn get_liquidity_from_amounts(
        &self,
        sqrt_price_current: u128,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        amount_a: u64,
        amount_b: u64,
        round_up: Option<bool>,
    ) -> u128 {
        get_liquidity_from_amounts(
            sqrt_price_current,
            sqrt_price_lower,
            sqrt_price_upper,
            amount_a,
            amount_b,
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
        _round_up: Option<bool>,
    ) -> u128 {
        if is_token_a {
            raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_amount_0(
                sqrt_price_lower,
                sqrt_price_upper,
                amount,
            )
        } else {
            raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_amount_1(
                sqrt_price_lower,
                sqrt_price_upper,
                amount,
            )
        }
    }
}

pub fn get_liquidity_from_amounts(
    sqrt_price_current: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    amount_a: u64,
    amount_b: u64,
    _round_up: Option<bool>,
) -> u128 {
    raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_amounts(
        sqrt_price_current,
        sqrt_price_lower,
        sqrt_price_upper,
        amount_a,
        amount_b,
    )
}

pub fn get_amounts_from_liquidity(
    sqrt_price_current: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    liquidity: i128,
) -> (u64, u64) {
    let (amount_a, amount_b) = get_delta_amounts_signed_v2(
        sqrt_price_current,
        sqrt_price_lower,
        sqrt_price_upper,
        liquidity,
    );
    let amount_a: u64 = amount_a.try_into().unwrap();
    let amount_b: u64 = amount_b.try_into().unwrap();
    (amount_a, amount_b)
}

pub fn get_delta_amounts_signed_v2(
    sqrt_price_x64_current: u128,
    sqrt_price_x64_lower: u128,
    sqrt_price_x64_upper: u128,
    liquidity_delta: i128,
) -> (i64, i64) {
    let mut amount_a = 0;
    let mut amount_b = 0;
    if sqrt_price_x64_current < sqrt_price_x64_lower {
        amount_a = liquidity_math::get_delta_amount_0_signed(
            sqrt_price_x64_lower,
            sqrt_price_x64_upper,
            liquidity_delta,
        );
    } else if sqrt_price_x64_current < sqrt_price_x64_upper {
        amount_a = liquidity_math::get_delta_amount_0_signed(
            sqrt_price_x64_current,
            sqrt_price_x64_upper,
            liquidity_delta,
        );
        amount_b = liquidity_math::get_delta_amount_1_signed(
            sqrt_price_x64_lower,
            sqrt_price_x64_current,
            liquidity_delta,
        );
    } else {
        amount_b = liquidity_math::get_delta_amount_1_signed(
            sqrt_price_x64_lower,
            sqrt_price_x64_upper,
            liquidity_delta,
        );
    }
    (amount_a, amount_b)
}
