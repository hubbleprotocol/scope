use crate::{DatedPrice, Price, Result, ScopeError};

use anchor_lang::prelude::*;
use anchor_lang::solana_program::borsh::try_from_slice_unchecked;

use spl_stake_pool::state::StakePool;

const DECIMALS: u32 = 15u32;

// Gives the price of 1 staked SOL in SOL
pub fn get_price(
    stake_pool_account_info: &AccountInfo,
    current_clock: &Clock,
) -> Result<DatedPrice> {
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_account_info.data.borrow())
        .map_err(|_| ScopeError::UnexpectedAccount)?;

    #[cfg(not(feature = "skip_price_validation"))]
    if stake_pool.last_update_epoch != current_clock.epoch {
        // The price has not been refreshed this epoch
        msg!("SPL Stake account has not been refreshed in current epoch");
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
        _reserved: Default::default(),
    };

    Ok(dated_price)
}

fn scaled_rate(stake_pool: &StakePool) -> Result<u64> {
    const FACTOR: u64 = 10u64.pow(DECIMALS);
    stake_pool
        .calc_lamports_withdraw_amount(FACTOR)
        .ok_or_else(|| ScopeError::MathOverflow.into())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn minted_token_is_equal_to_token_in_vault() {
        let total_lamports = 10u64.pow(5);
        let pool_token_supply = 10u64.pow(5);
        let stake_pool = StakePool {
            total_lamports,
            pool_token_supply,
            ..Default::default()
        };
        assert_eq!(scaled_rate(&stake_pool).unwrap(), 10u64.pow(DECIMALS));
    }

    #[test]
    pub fn minted_token_is_2x_token_in_vault() {
        // Note: this should never happen
        let total_lamports = 10u64.pow(5);
        let pool_token_supply = 2 * 10u64.pow(5);
        let stake_pool = StakePool {
            total_lamports,
            pool_token_supply,
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
        let total_lamports = 2 * 10u64.pow(5);
        let pool_token_supply = 10u64.pow(5);
        let stake_pool = StakePool {
            total_lamports,
            pool_token_supply,
            ..Default::default()
        };
        // Expect staked token price to be 2 tokens
        assert_eq!(scaled_rate(&stake_pool).unwrap(), 2 * 10u64.pow(DECIMALS));
    }
}
