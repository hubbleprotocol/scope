use anchor_lang::{
    prelude::{AccountInfo, Clock, Result, SolanaSysvar},
    AnchorSerialize,
};

use self::msol_stake_pool::{StakeSystem, State};

pub fn initialize(
    stake_pool_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> Result<()> {
    let pool = State {
        available_reserve_balance: total_liquidity,
        msol_supply: mint_total_supply,
        stake_system: StakeSystem {
            last_stake_delta_epoch: Clock::get()?.epoch,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut acc = stake_pool_account.data.borrow_mut();
    let mut data = &mut acc.as_mut()[8..];
    pool.serialize(&mut data)?;
    Ok(())
}

pub fn update(
    stake_pool_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> Result<()> {
    let pool = State {
        available_reserve_balance: total_liquidity,
        msol_supply: mint_total_supply,
        stake_system: StakeSystem {
            last_stake_delta_epoch: Clock::get()?.epoch,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut acc = stake_pool_account.data.borrow_mut();
    let mut data = &mut acc.as_mut()[8..];
    pool.serialize(&mut data)?;
    Ok(())
}

mod msol_stake_pool {
    use anchor_lang::prelude::borsh::BorshSchema;

    use anchor_lang::prelude::*;
    use solana_program::pubkey::Pubkey;

    #[derive(
        Clone,
        Copy,
        Debug,
        Default,
        AnchorSerialize,
        AnchorDeserialize,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
    )]
    pub struct Fee {
        pub basis_points: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct LiqPool {
        pub lp_mint: Pubkey,
        pub lp_mint_authority_bump_seed: u8,
        pub sol_leg_bump_seed: u8,
        pub msol_leg_authority_bump_seed: u8,
        pub msol_leg: Pubkey,

        //The next 3 values define the SOL/mSOL Liquidity pool fee curve params
        // We assume this pool is always UNBALANCED, there should be more SOL than mSOL 99% of the time
        ///Liquidity target. If the Liquidity reach this amount, the fee reaches lp_min_discount_fee
        pub lp_liquidity_target: u64, // 10_000 SOL initially
        /// Liquidity pool max fee
        pub lp_max_fee: Fee, //3% initially
        /// SOL/mSOL Liquidity pool min fee
        pub lp_min_fee: Fee, //0.3% initially
        /// Treasury cut
        pub treasury_cut: Fee, //2500 => 25% how much of the Liquid unstake fee goes to treasury_msol_account

        pub lp_supply: u64, // virtual lp token supply. May be > real supply because of burning tokens. Use UpdateLiqPool to align it with real value
        pub lent_from_sol_leg: u64,
        pub liquidity_sol_cap: u64,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, BorshSchema, Debug)]
    pub struct List {
        pub account: Pubkey,
        pub item_size: u32,
        pub count: u32,
        // For chunked change account
        pub new_account: Pubkey,
        pub copied_count: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct StakeSystem {
        pub stake_list: List,
        //pub last_update_epoch: u64,
        //pub updated_during_last_epoch: u32,
        pub delayed_unstake_cooling_down: u64,
        pub stake_deposit_bump_seed: u8,
        pub stake_withdraw_bump_seed: u8,

        /// set by admin, how much slots before the end of the epoch, stake-delta can start
        pub slots_for_stake_delta: u64,
        /// Marks the start of stake-delta operations, meaning that if somebody starts a delayed-unstake ticket
        /// after this var is set with epoch_num the ticket will have epoch_created = current_epoch+1
        /// (the user must wait one more epoch, because their unstake-delta will be execute in this epoch)
        pub last_stake_delta_epoch: u64,
        pub min_stake: u64, // Minimal stake account delegation
        /// can be set by validator-manager-auth to allow a second run of stake-delta to stake late stakers in the last minute of the epoch
        /// so we maximize user's rewards
        pub extra_stake_delta_runs: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct ValidatorSystem {
        pub validator_list: List,
        pub manager_authority: Pubkey,
        pub total_validator_score: u32,
        /// sum of all active lamports staked
        pub total_active_balance: u64,
        /// allow & auto-add validator when a user deposits a stake-account of a non-listed validator
        pub auto_add_validator_enabled: u8,
    }

    #[derive(Default, Clone, Debug, AnchorDeserialize, AnchorSerialize)]
    pub struct State {
        pub msol_mint: Pubkey,

        pub admin_authority: Pubkey,

        // Target for withdrawing rent reserve SOLs. Save bot wallet account here
        pub operational_sol_account: Pubkey,
        // treasury - external accounts managed by marinade DAO
        // pub treasury_sol_account: Pubkey,
        pub treasury_msol_account: Pubkey,

        // Bump seeds:
        pub reserve_bump_seed: u8,
        pub msol_mint_authority_bump_seed: u8,

        pub rent_exempt_for_token_acc: u64, // Token-Account For rent exempt

        // fee applied on rewards
        pub reward_fee: Fee,

        pub stake_system: StakeSystem,
        pub validator_system: ValidatorSystem, //includes total_balance = total stake under management

        // sum of all the orders received in this epoch
        // must not be used for stake-unstake amount calculation
        // only for reference
        // epoch_stake_orders: u64,
        // epoch_unstake_orders: u64,
        pub liq_pool: LiqPool,
        pub available_reserve_balance: u64, // reserve_pda.lamports() - self.rent_exempt_for_token_acc. Virtual value (real may be > because of transfers into reserve). Use Update* to align
        pub msol_supply: u64, // Virtual value (may be < because of token burn). Use Update* to align
        // For FE. Don't use it for token amount calculation
        pub msol_price: u64,

        ///count tickets for delayed-unstake
        pub circulating_ticket_count: u64,
        ///total lamports amount of generated and not claimed yet tickets
        pub circulating_ticket_balance: u64,
        pub lent_from_reserve: u64,
        pub min_deposit: u64,
        pub min_withdraw: u64,
        pub staking_sol_cap: u64,

        pub emergency_cooling_down: u64,
    }
}
