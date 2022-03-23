use std::ops::RangeInclusive;

use crate::{utils::pyth::get_price, ScopeError};
use anchor_lang::prelude::*;

const BATCH_UPDATE_SIZE: usize = 8;

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct RefreshBatch<'info> {
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    // Array is an unnecessary complicated beast here
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_0: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_1: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_2: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_3: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_4: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_5: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_6: AccountInfo<'info>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info_7: AccountInfo<'info>,

    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct RefreshList<'info> {
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,

    pub clock: Sysvar<'info, Clock>,
    // Note: use remaining accounts as price accounts
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: usize) -> ProgramResult {
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let pyth_price_info = &ctx.accounts.pyth_price_info;

    // Check that the provided pyth account is the one referenced in oracleMapping
    if oracle_mappings.price_info_accounts[token] != pyth_price_info.key() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let price = get_price(pyth_price_info)?;

    oracle.prices[token] = price;

    Ok(())
}

pub fn refresh_batch_prices(ctx: Context<RefreshBatch>, first_token: usize) -> ProgramResult {
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let range = RangeInclusive::new(first_token, first_token + BATCH_UPDATE_SIZE);
    let partial_mappings = &oracle_mappings.price_info_accounts[range.clone()];
    let partial_prices = &mut oracle.prices[range];

    // Easy rebuild of the missing array
    let pyth_prices_info = [
        &ctx.accounts.pyth_price_info_0,
        &ctx.accounts.pyth_price_info_1,
        &ctx.accounts.pyth_price_info_2,
        &ctx.accounts.pyth_price_info_3,
        &ctx.accounts.pyth_price_info_4,
        &ctx.accounts.pyth_price_info_5,
        &ctx.accounts.pyth_price_info_6,
        &ctx.accounts.pyth_price_info_7,
    ];

    let zero_pk: Pubkey = Pubkey::default();

    for ((expected, received), to_update) in partial_mappings
        .iter()
        .zip(pyth_prices_info.into_iter())
        .zip(partial_prices.iter_mut())
    {
        // Ignore empty accounts
        if received.key() == zero_pk {
            continue;
        }
        // Check that the provided pyth accounts are the one referenced in oracleMapping
        if *expected != received.key() {
            return Err(ScopeError::UnexpectedAccount.into());
        }
        match get_price(received) {
            Ok(price) => *to_update = price,
            Err(_) => msg!("Price skipped as validation failed"), // No format as its a bit costly
        };
    }

    Ok(())
}

pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: &[u16]) -> ProgramResult {
    let oracle_mappings = &ctx.accounts.oracle_mappings.load()?.price_info_accounts;
    let oracle_prices = &mut ctx.accounts.oracle_prices.load_mut()?.prices;

    // Check that the received token list is not too long
    if tokens.len() > crate::MAX_ENTRIES {
        return Err(ProgramError::InvalidArgument.into());
    }
    // Check the received token list is as long as the number of provided accounts
    if tokens.len() != ctx.remaining_accounts.len() {
        return Err(ScopeError::AccountsAndTokenMismatch.into());
    }

    let zero_pk: Pubkey = Pubkey::default();

    for (&token_nb, received_account) in tokens.iter().zip(ctx.remaining_accounts.iter()) {
        let token_idx: usize = token_nb.into();
        let oracle_mapping = oracle_mappings
            .get(token_idx)
            .ok_or(ScopeError::BadTokenNb)?;
        // Ignore unset mapping accounts
        if zero_pk == *oracle_mapping {
            continue;
        }
        // Check that the provided pyth accounts are the one referenced in oracleMapping
        if oracle_mappings[token_idx] != received_account.key() {
            return Err(ScopeError::UnexpectedAccount.into());
        }
        match get_price(received_account) {
            Ok(price) => {
                let to_update = oracle_prices
                    .get_mut(token_idx)
                    .ok_or(ScopeError::BadTokenNb)?;
                *to_update = price;
            }
            Err(_) => msg!("Price skipped as validation failed"), // No format as its a bit costly
        };
    }

    Ok(())
}
