use std::cell::Ref;

use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::decimal::U192;
pub use lb_clmm_itf as lb_clmm;
use solana_program::program_pack::Pack;

use crate::utils::{math, zero_copy_deserialize};
use crate::{DatedPrice, Result, ScopeError};

/// Gives the price of the given token pair in the given pool
pub fn get_price<'a, 'b>(
    a_to_b: bool,
    pool: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // Get extra accounts
    let mint_token_a_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;
    let mint_token_b_account_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    // Load main account
    let lb_pair_state: Ref<'_, lb_clmm::LbPair> = zero_copy_deserialize(pool)?;

    // Check extra accounts pubkeys
    require_keys_eq!(
        lb_pair_state.token_x_mint,
        mint_token_a_account_info.key(),
        ScopeError::AccountsAndTokenMismatch
    );

    require_keys_eq!(
        lb_pair_state.token_y_mint,
        mint_token_b_account_info.key(),
        ScopeError::AccountsAndTokenMismatch
    );

    // Load extra accounts
    let mint_a_decimals = {
        let mint_borrow = mint_token_a_account_info.data.borrow();
        Mint::unpack(&mint_borrow)?.decimals
    };

    let mint_b_decimals = {
        let mint_borrow = mint_token_b_account_info.data.borrow();
        Mint::unpack(&mint_borrow)?.decimals
    };

    // Compute price
    let q64x64_price =
        lb_clmm::get_x64_price_from_id(lb_pair_state.active_id, lb_pair_state.bin_step)
            .ok_or_else(|| {
                msg!("Math overflow when calculating dlmm price");
                error!(ScopeError::MathOverflow)
            })?;
    let q64x64_price = if a_to_b {
        U192::from(q64x64_price)
    } else {
        // Invert price
        (U192::one() << 128) / q64x64_price
    };

    let lamport_price = math::q64x64_price_to_price(q64x64_price)?;
    let price = math::price_of_lamports_to_price_of_tokens(
        lamport_price,
        mint_a_decimals.into(),
        mint_b_decimals.into(),
    );

    // Return price
    Ok(DatedPrice {
        price,
        last_updated_slot: clock.slot,
        unix_timestamp: clock.unix_timestamp as u64,
        ..Default::default()
    })
}

pub fn validate_pool_account(pool: &AccountInfo) -> Result<()> {
    let _: Ref<'_, lb_clmm::LbPair> = zero_copy_deserialize(pool)?;
    Ok(())
}
