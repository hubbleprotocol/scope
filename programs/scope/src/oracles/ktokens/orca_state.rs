use anchor_lang::prelude::*;
use whirlpool::state::{
    Position as OrcaPosition, PositionRewardInfo as OrcaPositionRewardInfo,
    Whirlpool as OrcaWhirlpool, WhirlpoolRewardInfo as OrcaWhirlpoolRewardInfo,
};

use crate::{utils::account_deserialize, ScopeResult};

// Number of rewards supported by Whirlpools
pub const NUM_REWARDS_ORCA: usize = 3;
pub const TICK_ARRAY_SIZE_USIZE: usize = 88;

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize, Default, Debug, PartialEq, Eq)]
pub struct PositionRewardInfo {
    // Q64.64
    pub growth_inside_checkpoint: u128,
    pub amount_owed: u64,
}

impl PositionRewardInfo {
    pub fn to_orca_reward_info(self) -> OrcaPositionRewardInfo {
        OrcaPositionRewardInfo {
            growth_inside_checkpoint: self.growth_inside_checkpoint,
            amount_owed: self.amount_owed,
        }
    }
}

/// External types
#[account]
#[derive(Debug, Copy, PartialEq, Eq, Default)]
pub struct Whirlpool {
    pub whirlpools_config: Pubkey, // 32
    pub whirlpool_bump: [u8; 1],   // 1

    pub tick_spacing: u16,          // 2
    pub tick_spacing_seed: [u8; 2], // 2

    // Stored as hundredths of a basis point
    // u16::MAX corresponds to ~6.5%
    pub fee_rate: u16, // 2

    // Denominator for portion of fee rate taken (1/x)%
    pub protocol_fee_rate: u16, // 2

    // Maximum amount that can be held by Solana account
    pub liquidity: u128, // 16

    // MAX/MIN at Q32.64, but using Q64.64 for rounder bytes
    // Q64.64
    pub sqrt_price: u128,        // 16
    pub tick_current_index: i32, // 4

    pub protocol_fee_owed_a: u64, // 8
    pub protocol_fee_owed_b: u64, // 8

    pub token_mint_a: Pubkey,  // 32
    pub token_vault_a: Pubkey, // 32

    // Q64.64
    pub fee_growth_global_a: u128, // 16

    pub token_mint_b: Pubkey,  // 32
    pub token_vault_b: Pubkey, // 32

    // Q64.64
    pub fee_growth_global_b: u128, // 16

    pub reward_last_updated_timestamp: u64, // 8

    pub reward_infos: [WhirlpoolRewardInfo; NUM_REWARDS_ORCA], // 384
}

impl Whirlpool {
    pub fn to_orca_whirlpool(self) -> OrcaWhirlpool {
        OrcaWhirlpool {
            whirlpools_config: self.whirlpools_config,
            whirlpool_bump: self.whirlpool_bump,
            tick_spacing: self.tick_spacing,
            tick_spacing_seed: self.tick_spacing_seed,
            fee_rate: self.fee_rate,
            protocol_fee_rate: self.protocol_fee_rate,
            liquidity: self.liquidity,
            sqrt_price: self.sqrt_price,
            tick_current_index: self.tick_current_index,
            protocol_fee_owed_a: self.protocol_fee_owed_a,
            protocol_fee_owed_b: self.protocol_fee_owed_b,
            token_mint_a: self.token_mint_a,
            token_vault_a: self.token_vault_a,
            fee_growth_global_a: self.fee_growth_global_a,
            token_mint_b: self.token_mint_b,
            token_vault_b: self.token_vault_b,
            fee_growth_global_b: self.fee_growth_global_b,
            reward_last_updated_timestamp: self.reward_last_updated_timestamp,
            reward_infos: self.reward_infos.map(|r| r.to_orca_reward_info()),
        }
    }

    pub fn from_account_to_orca_whirlpool(account: &AccountInfo<'_>) -> ScopeResult<OrcaWhirlpool> {
        let position: Self = account_deserialize(account)?;
        Ok(position.to_orca_whirlpool())
    }
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize, Default, Debug, PartialEq, Eq)]
pub struct WhirlpoolRewardInfo {
    /// Reward token mint.
    pub mint: Pubkey,
    /// Reward vault token account.
    pub vault: Pubkey,
    /// Authority account that has permission to initialize the reward and set emissions.
    pub authority: Pubkey,
    /// Q64.64 number that indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: u128,
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub growth_global_x64: u128,
}

impl WhirlpoolRewardInfo {
    pub fn to_orca_reward_info(self) -> OrcaWhirlpoolRewardInfo {
        OrcaWhirlpoolRewardInfo {
            mint: self.mint,
            vault: self.vault,
            authority: self.authority,
            emissions_per_second_x64: self.emissions_per_second_x64,
            growth_global_x64: self.growth_global_x64,
        }
    }
}

#[account]
#[derive(Default, Debug)]
pub struct Position {
    pub whirlpool: Pubkey,     // 32
    pub position_mint: Pubkey, // 32
    pub liquidity: u128,       // 16
    pub tick_lower_index: i32, // 4
    pub tick_upper_index: i32, // 4

    // Q64.64
    pub fee_growth_checkpoint_a: u128, // 16
    pub fee_owed_a: u64,               // 8
    // Q64.64
    pub fee_growth_checkpoint_b: u128, // 16
    pub fee_owed_b: u64,               // 8

    pub reward_infos: [PositionRewardInfo; NUM_REWARDS_ORCA], // 72
}

impl Position {
    pub fn to_orca_position(&self) -> OrcaPosition {
        OrcaPosition {
            whirlpool: self.whirlpool,
            position_mint: self.position_mint,
            liquidity: self.liquidity,
            tick_lower_index: self.tick_lower_index,
            tick_upper_index: self.tick_upper_index,
            fee_growth_checkpoint_a: self.fee_growth_checkpoint_a,
            fee_owed_a: self.fee_owed_a,
            fee_growth_checkpoint_b: self.fee_growth_checkpoint_b,
            fee_owed_b: self.fee_owed_b,
            reward_infos: self.reward_infos.map(|x| x.to_orca_reward_info()),
        }
    }

    pub fn from_account_to_orca_position(account: &AccountInfo<'_>) -> ScopeResult<OrcaPosition> {
        let position: Self = account_deserialize(account)?;
        Ok(position.to_orca_position())
    }
}

#[zero_copy]
#[repr(packed)]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Tick {
    // Total 137 bytes
    pub initialized: bool,     // 1
    pub liquidity_net: i128,   // 16
    pub liquidity_gross: u128, // 16

    // Q64.64
    pub fee_growth_outside_a: u128, // 16
    // Q64.64
    pub fee_growth_outside_b: u128, // 16

    // Array of Q64.64
    pub reward_growths_outside: [u128; NUM_REWARDS_ORCA], // 48 = 16 * 3
}

#[account(zero_copy)]
#[repr(packed)]
pub struct TickArray {
    pub start_tick_index: i32,
    pub ticks: [Tick; TICK_ARRAY_SIZE_USIZE],
    pub whirlpool: Pubkey,
}

impl Default for TickArray {
    #[inline]
    fn default() -> TickArray {
        TickArray {
            whirlpool: Pubkey::default(),
            ticks: [Tick::default(); TICK_ARRAY_SIZE_USIZE],
            start_tick_index: 0,
        }
    }
}

#[test]
fn test_state_position_to_orca_position() {
    let whirlpool = Pubkey::new_unique();
    let position_mint = Pubkey::new_unique();
    let reward_infos = [
        PositionRewardInfo {
            growth_inside_checkpoint: 123,
            amount_owed: 0,
        },
        PositionRewardInfo {
            growth_inside_checkpoint: 1243543,
            amount_owed: 453540,
        },
        PositionRewardInfo {
            growth_inside_checkpoint: 5478547943,
            amount_owed: 985454,
        },
    ];
    let liquidity = 433454434;
    let tick_lower_index = -43;
    let tick_upper_index = 54;
    let fee_growth_checkpoint_a = 438321;
    let fee_owed_a = 3123243;
    let fee_growth_checkpoint_b = 12843;
    let fee_owed_b = 4354;

    let position = Position {
        whirlpool,
        position_mint,
        liquidity,
        tick_lower_index,
        tick_upper_index,
        fee_growth_checkpoint_a,
        fee_owed_a,
        fee_growth_checkpoint_b,
        fee_owed_b,
        reward_infos,
    };

    let orca_position = position.to_orca_position();
    assert_eq!(orca_position.whirlpool, whirlpool);
    assert_eq!(orca_position.position_mint, position_mint);
    assert_eq!(orca_position.liquidity, liquidity);
    assert_eq!(orca_position.tick_lower_index, tick_lower_index);
    assert_eq!(orca_position.tick_upper_index, tick_upper_index);
    assert_eq!(
        orca_position.fee_growth_checkpoint_a,
        fee_growth_checkpoint_a
    );
    assert_eq!(orca_position.fee_owed_a, fee_owed_a);
    assert_eq!(
        orca_position.fee_growth_checkpoint_b,
        fee_growth_checkpoint_b
    );
    assert_eq!(orca_position.fee_owed_b, fee_owed_b);
    assert_eq!(
        orca_position.reward_infos[0],
        reward_infos[0].to_orca_reward_info()
    );
    assert_eq!(
        orca_position.reward_infos[1],
        reward_infos[1].to_orca_reward_info()
    );
    assert_eq!(
        orca_position.reward_infos[2],
        reward_infos[2].to_orca_reward_info()
    );
}
