pub mod orca_clmm;
pub mod raydium_clmm;

use anchor_lang::prelude::Pubkey;

use crate::{
    oracles::ktokens::{
        kamino::{RewardsAmounts, TokenAmounts},
        DEX,
    },
    ScopeResult,
};

pub trait Clmm {
    fn get_tick_current_index(&self) -> i32;

    fn get_current_sqrt_price(&self) -> u128;

    fn get_position_tick_lower_index(&self) -> ScopeResult<i32>;

    fn get_position_tick_upper_index(&self) -> ScopeResult<i32>;

    fn sqrt_price_from_tick(&self, tick: i32) -> u128;

    fn get_position_pending_fees(&self) -> ScopeResult<TokenAmounts>;

    fn get_position_liquidity(&self) -> ScopeResult<u128>;

    fn get_position_pending_rewards(&self) -> ScopeResult<RewardsAmounts>;

    fn get_pool_tick_spacing(&self) -> u16;

    fn pool_reward_info_initialized(&self, index: usize) -> bool;

    fn get_pool_reward_vault(&self, index: usize) -> Pubkey;

    fn get_pool_reward_mint(&self, index: usize) -> Pubkey;

    fn get_pool_vaults(&self) -> (Pubkey, Pubkey);

    fn get_dex(&self) -> DEX;

    fn position_has_pending_fees(&self) -> ScopeResult<bool>;

    fn get_liquidity_from_amounts(
        &self,
        sqrt_price_current: u128,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        amount_a: u64,
        amount_b: u64,
        round_up: Option<bool>,
    ) -> u128;

    fn get_liquidity_for_amount(
        &self,
        is_token_a: bool,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        amount: u64,
        round_up: Option<bool>,
    ) -> u128;

    fn get_amounts_from_liquidity(
        &self,
        sqrt_price_current: u128,
        sqrt_price_lower: u128,
        sqrt_price_upper: u128,
        liquidity: i128,
    ) -> (u64, u64);
}
