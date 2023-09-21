use std::cell::{Ref, RefCell};

use anchor_lang::prelude::*;
use raydium_amm_v3::states::{
    PersonalPositionState as RaydiumPersonalPosition, PoolState as RaydiumPool, PositionRewardInfo,
    ProtocolPositionState as RaydiumProtocolPosition, RewardInfo as RaydiumRewardInfo,
};

use crate::{
    utils::{account_deserialize, zero_copy_deserialize},
    ScopeResult,
};

pub const REWARD_NUM: usize = 3;

// External types
#[account(zero_copy)]
#[repr(packed)]
#[derive(Default, Debug)]
pub struct PoolState {
    /// Bump to identify PDA
    pub bump: [u8; 1],
    // Which config the pool belongs
    pub amm_config: Pubkey,
    // Pool creator
    pub owner: Pubkey,

    /// Token pair of the pool, where token_mint_0 address < token_mint_1 address
    pub token_mint_0: Pubkey,
    pub token_mint_1: Pubkey,

    /// Token pair vault
    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,

    /// observation account key
    pub observation_key: Pubkey,

    /// mint0 and mint1 decimals
    pub mint_decimals_0: u8,
    pub mint_decimals_1: u8,

    /// The minimum number of ticks between initialized ticks
    pub tick_spacing: u16,
    /// The currently in range liquidity available to the pool.
    pub liquidity: u128,
    /// The current price of the pool as a sqrt(token_1/token_0) Q64.64 value
    pub sqrt_price_x64: u128,
    /// The current tick of the pool, i.e. according to the last tick transition that was run.
    pub tick_current: i32,

    /// the most-recently updated index of the observations array
    pub observation_index: u16,
    pub observation_update_duration: u16,

    /// The fee growth as a Q64.64 number, i.e. fees of token_0 and token_1 collected per
    /// unit of liquidity for the entire life of the pool.
    pub fee_growth_global_0_x64: u128,
    pub fee_growth_global_1_x64: u128,

    /// The amounts of token_0 and token_1 that are owed to the protocol.
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    /// The amounts in and out of swap token_0 and token_1
    pub swap_in_amount_token_0: u128,
    pub swap_out_amount_token_1: u128,
    pub swap_in_amount_token_1: u128,
    pub swap_out_amount_token_0: u128,

    /// Bitwise representation of the state of the pool
    /// bit0, 1: disable open position and increase liquidity, 0: normal
    /// bit1, 1: disable decrease liquidity, 0: normal
    /// bit2, 1: disable collect fee, 0: normal
    /// bit3, 1: disable collect reward, 0: normal
    /// bit4, 1: disable swap, 0: normal
    pub status: u8,
    /// Leave blank for future use
    pub padding: [u8; 7],

    pub reward_infos: [RewardInfo; REWARD_NUM],

    /// Packed initialized tick array state
    pub tick_array_bitmap: [u64; 16],

    /// except protocol_fee and fund_fee
    pub total_fees_token_0: u64,
    /// except protocol_fee and fund_fee
    pub total_fees_claimed_token_0: u64,
    pub total_fees_token_1: u64,
    pub total_fees_claimed_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    // The timestamp allowed for swap in the pool.
    pub open_time: u64,

    // Unused bytes for future upgrades.
    pub padding1: [u64; 25],
    pub padding2: [u64; 32],
}

impl PoolState {
    pub fn to_raydium_pool<'a>(&'a self) -> Ref<'a, RaydiumPool> {
        let pool = RaydiumPool {
            bump: self.bump,
            amm_config: self.amm_config,
            owner: self.owner,
            token_mint_0: self.token_mint_0,
            token_mint_1: self.token_mint_1,
            token_vault_0: self.token_vault_0,
            token_vault_1: self.token_vault_1,
            observation_key: self.observation_key,
            mint_decimals_0: self.mint_decimals_0,
            mint_decimals_1: self.mint_decimals_1,
            tick_spacing: self.tick_spacing,
            liquidity: self.liquidity,
            sqrt_price_x64: self.sqrt_price_x64,
            tick_current: self.tick_current,
            observation_index: self.observation_index,
            observation_update_duration: self.observation_update_duration,
            fee_growth_global_0_x64: self.fee_growth_global_0_x64,
            fee_growth_global_1_x64: self.fee_growth_global_1_x64,
            protocol_fees_token_0: self.protocol_fees_token_0,
            protocol_fees_token_1: self.protocol_fees_token_1,
            swap_in_amount_token_0: self.swap_in_amount_token_0,
            swap_out_amount_token_1: self.swap_out_amount_token_1,
            swap_in_amount_token_1: self.swap_in_amount_token_1,
            swap_out_amount_token_0: self.swap_out_amount_token_0,
            padding: self.padding,
            reward_infos: self.reward_infos.map(|r| r.to_raydium_reward_info()),
            tick_array_bitmap: self.tick_array_bitmap,
            padding1: self.padding1,
            padding2: self.padding2,
            status: self.status,
            total_fees_token_0: self.total_fees_token_0,
            total_fees_claimed_token_0: self.total_fees_claimed_token_0,
            total_fees_token_1: self.total_fees_token_1,
            total_fees_claimed_token_1: self.total_fees_claimed_token_1,
            fund_fees_token_0: self.fund_fees_token_0,
            fund_fees_token_1: self.fund_fees_token_1,
            open_time: self.open_time,
        };
        RefCell::new(pool).borrow()
    }

    pub fn from_account_to_raydium_pool<'info>(
        account: &'info AccountInfo,
    ) -> ScopeResult<Ref<'info, RaydiumPool>> {
        let pool: Ref<Self> = zero_copy_deserialize(account)?;
        Ok(pool.to_raydium_pool())
    }
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize, Default, Debug, PartialEq, Eq)]
pub struct RewardInfo {
    /// Reward state
    pub reward_state: u8,
    /// Reward open time
    pub open_time: u64,
    /// Reward end time
    pub end_time: u64,
    /// Reward last update time
    pub last_update_time: u64,
    /// Q64.64 number indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: u128,
    /// The total amount of reward emissioned
    pub reward_total_emissioned: u64,
    /// The total amount of claimed reward
    pub reward_claimed: u64,
    /// Reward token mint.
    pub token_mint: Pubkey,
    /// Reward vault token account.
    pub token_vault: Pubkey,
    /// The owner that has permission to set reward param
    pub authority: Pubkey,
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub reward_growth_global_x64: u128,
}

impl RewardInfo {
    pub const LEN: usize = 1 + 8 + 8 + 8 + 16 + 8 + 8 + 32 + 32 + 32 + 16;
    /// Creates a new RewardInfo
    pub fn new(authority: Pubkey) -> Self {
        Self {
            authority,
            ..Default::default()
        }
    }

    /// Returns true if this reward is initialized.
    /// Once initialized, a reward cannot transition back to uninitialized.
    pub fn initialized(&self) -> bool {
        self.token_mint.ne(&Pubkey::default())
    }

    pub fn get_reward_growths(reward_infos: &[RewardInfo; REWARD_NUM]) -> [u128; REWARD_NUM] {
        let mut reward_growths = [0u128; REWARD_NUM];
        for i in 0..REWARD_NUM {
            reward_growths[i] = reward_infos[i].reward_growth_global_x64;
        }
        reward_growths
    }

    pub fn to_raydium_reward_info(&self) -> RaydiumRewardInfo {
        RaydiumRewardInfo {
            reward_state: self.reward_state,
            open_time: self.open_time,
            end_time: self.end_time,
            last_update_time: self.last_update_time,
            emissions_per_second_x64: self.emissions_per_second_x64,
            reward_total_emissioned: self.reward_total_emissioned,
            reward_claimed: self.reward_claimed,
            token_mint: self.token_mint,
            token_vault: self.token_vault,
            authority: self.authority,
            reward_growth_global_x64: self.reward_growth_global_x64,
        }
    }
}

#[account]
#[derive(Default, Debug)]
pub struct PersonalPositionState {
    /// Bump to identify PDA
    pub bump: u8,

    /// Mint address of the tokenized position
    pub nft_mint: Pubkey,

    /// The ID of the pool with which this token is connected
    pub pool_id: Pubkey,

    /// The lower bound tick of the position
    pub tick_lower_index: i32,

    /// The upper bound tick of the position
    pub tick_upper_index: i32,

    /// The amount of liquidity owned by this position
    pub liquidity: u128,

    /// The token_0 fee growth of the aggregate position as of the last action on the individual position
    pub fee_growth_inside_0_last_x64: u128,

    /// The token_1 fee growth of the aggregate position as of the last action on the individual position
    pub fee_growth_inside_1_last_x64: u128,

    /// The fees owed to the position owner in token_0, as of the last computation
    pub token_fees_owed_0: u64,

    /// The fees owed to the position owner in token_1, as of the last computation
    pub token_fees_owed_1: u64,

    // Position reward info
    pub reward_infos: [PositionRewardInfo; REWARD_NUM],
    // Unused bytes for future upgrades.
    pub padding: [u64; 8],
}

impl PersonalPositionState {
    pub fn to_raydium_position(&self) -> RaydiumPersonalPosition {
        RaydiumPersonalPosition {
            pool_id: self.pool_id,
            tick_lower_index: self.tick_lower_index,
            tick_upper_index: self.tick_upper_index,
            liquidity: self.liquidity,
            fee_growth_inside_0_last_x64: self.fee_growth_inside_0_last_x64,
            fee_growth_inside_1_last_x64: self.fee_growth_inside_1_last_x64,
            token_fees_owed_0: self.token_fees_owed_0,
            token_fees_owed_1: self.token_fees_owed_1,
            reward_infos: self.reward_infos,
            bump: self.bump,
            nft_mint: self.nft_mint,
            padding: self.padding,
        }
    }

    pub fn from_account_to_raydium_position(
        account: &AccountInfo<'_>,
    ) -> ScopeResult<RaydiumPersonalPosition> {
        let position: Self = account_deserialize(account)?;
        Ok(position.to_raydium_position())
    }
}

#[account]
#[derive(Default, Debug)]
pub struct ProtocolPositionState {
    /// Bump to identify PDA
    pub bump: u8,

    /// The ID of the pool with which this token is connected
    pub pool_id: Pubkey,

    /// The lower bound tick of the position
    pub tick_lower_index: i32,

    /// The upper bound tick of the position
    pub tick_upper_index: i32,

    /// The amount of liquidity owned by this position
    pub liquidity: u128,

    /// The token_0 fee growth per unit of liquidity as of the last update to liquidity or fees owed
    pub fee_growth_inside_0_last_x64: u128,

    /// The token_1 fee growth per unit of liquidity as of the last update to liquidity or fees owed
    pub fee_growth_inside_1_last_x64: u128,

    /// The fees owed to the position owner in token_0
    pub token_fees_owed_0: u64,

    /// The fees owed to the position owner in token_1
    pub token_fees_owed_1: u64,

    /// The reward growth per unit of liquidity as of the last update to liquidity
    pub reward_growth_inside: [u128; REWARD_NUM], // 24
    // Unused bytes for future upgrades.
    pub padding: [u64; 8],
}

impl ProtocolPositionState {
    pub fn to_raydium_position(&self) -> RaydiumProtocolPosition {
        RaydiumProtocolPosition {
            pool_id: self.pool_id,
            tick_lower_index: self.tick_lower_index,
            tick_upper_index: self.tick_upper_index,
            liquidity: self.liquidity,
            fee_growth_inside_0_last_x64: self.fee_growth_inside_0_last_x64,
            fee_growth_inside_1_last_x64: self.fee_growth_inside_1_last_x64,
            token_fees_owed_0: self.token_fees_owed_0,
            token_fees_owed_1: self.token_fees_owed_1,
            bump: self.bump,
            padding: self.padding,
            reward_growth_inside: self.reward_growth_inside,
        }
    }
}
