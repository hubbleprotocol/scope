use anchor_lang::prelude::*;
use solana_program::borsh0_10::try_from_slice_unchecked;

use crate::{DatedPrice, Price, Result, ScopeError};

use self::msol_stake_pool::State;

const DECIMALS: u32 = 15u32;

// Gives the price of 1 staked SOL in SOL
pub fn get_price(
    msol_pool_account_info: &AccountInfo,
    current_clock: &Clock,
) -> Result<DatedPrice> {
    let stake_pool = try_from_slice_unchecked::<State>(&msol_pool_account_info.data.borrow()[8..])
        .map_err(|_| {
            msg!("Provided pubkey is not a MSOL Stake account");
            ScopeError::UnexpectedAccount
        })?;
    #[cfg(not(feature = "skip_price_validation"))]
    if stake_pool.stake_system.last_stake_delta_epoch != current_clock.epoch {
        // The price has not been refreshed this epoch
        msg!("MSOL Stake account has not been refreshed in current epoch");
        #[cfg(not(feature = "localnet"))]
        return Err(ScopeError::PriceNotValid.into());
    }

    let value = scaled_rate(&stake_pool)?;

    let price = Price {
        value,
        exp: DECIMALS.into(),
    };
    let dated_price = DatedPrice {
        price,
        last_updated_slot: current_clock.slot,
        unix_timestamp: u64::try_from(current_clock.unix_timestamp).unwrap(),
        ..Default::default()
    };

    Ok(dated_price)
}

fn scaled_rate(stake_pool: &State) -> Result<u64> {
    const FACTOR: u64 = 10u64.pow(DECIMALS);
    stake_pool.calc_lamports_from_msol_amount(FACTOR)
}

mod msol_stake_pool {
    use anchor_lang::prelude::borsh::BorshSchema;

    /// calculate amount*numerator/denominator
    /// as value  = shares * share_price where share_price=total_value/total_shares
    /// or shares = amount_value / share_price where share_price=total_value/total_shares
    ///     => shares = amount_value * 1/share_price where 1/share_price=total_shares/total_value
    pub fn proportional(amount: u64, numerator: u64, denominator: u64) -> Result<u64> {
        if denominator == 0 {
            return Ok(amount);
        }
        u64::try_from((amount as u128) * (numerator as u128) / (denominator as u128))
            .map_err(|_| ScopeError::MathOverflow.into())
    }

    #[inline] //alias for proportional
    pub fn value_from_shares(shares: u64, total_value: u64, total_shares: u64) -> Result<u64> {
        proportional(shares, total_value, total_shares)
    }

    use super::*;
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

    impl State {
        pub fn total_cooling_down(&self) -> Result<u64> {
            self.stake_system
                .delayed_unstake_cooling_down
                .checked_add(self.emergency_cooling_down)
                .ok_or_else(|| ScopeError::MathOverflow.into())
        }

        /// total_active_balance + total_cooling_down + available_reserve_balance
        pub fn total_lamports_under_control(&self) -> Result<u64> {
            self.validator_system
                .total_active_balance
                .checked_add(self.total_cooling_down()?)
                .ok_or_else(|| ScopeError::MathOverflow)?
                .checked_add(self.available_reserve_balance) // reserve_pda.lamports() - self.rent_exempt_for_token_acc
                .ok_or_else(|| ScopeError::MathOverflow.into())
        }

        pub fn total_virtual_staked_lamports(&self) -> Result<u64> {
            // if we get slashed it may be negative but we must use 0 instead
            Ok(self
                .total_lamports_under_control()?
                .saturating_sub(self.circulating_ticket_balance)) //tickets created -> cooling down lamports or lamports already in reserve and not claimed yet
        }

        pub fn calc_lamports_from_msol_amount(&self, msol_amount: u64) -> Result<u64> {
            value_from_shares(
                msol_amount,
                self.total_virtual_staked_lamports()?,
                self.msol_supply,
            )
        }
    }
}

#[cfg(test)]
mod test {
    use crate::oracles::msol_stake::msol_stake_pool::State;

    use super::*;

    #[test]
    pub fn minted_token_is_equal_to_token_in_vault() {
        let available_reserve_balance = 10u64.pow(5);
        let msol_supply = 10u64.pow(5);
        let stake_pool = State {
            available_reserve_balance,
            msol_supply,
            ..Default::default()
        };
        assert_eq!(scaled_rate(&stake_pool).unwrap(), 10u64.pow(DECIMALS));
    }

    #[test]
    pub fn minted_token_is_2x_token_in_vault() {
        // Note: this should never happen
        let available_reserve_balance = 10u64.pow(5);
        let msol_supply = 2 * 10u64.pow(5);
        let stake_pool = State {
            available_reserve_balance,
            msol_supply,
            ..Default::default()
        };
        // Expect staked token price to be 0.5 token
        assert_eq!(
            scaled_rate(&stake_pool).unwrap(),
            5 * 10u64.pow(DECIMALS - 1)
        );
    }

    #[test]
    pub fn token_in_vault_is_2x_token_minted() {
        let available_reserve_balance = 2 * 10u64.pow(5);
        let msol_supply = 10u64.pow(5);
        let stake_pool = State {
            available_reserve_balance,
            msol_supply,
            ..Default::default()
        };
        // Expect staked token price to be 2 tokens
        assert_eq!(scaled_rate(&stake_pool).unwrap(), 2 * 10u64.pow(DECIMALS));
    }
}
