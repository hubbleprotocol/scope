use std::ops::RangeInclusive;

use crate::{utils::pyth::get_price, ScopeError};
use anchor_lang::prelude::*;

const BATCH_UPDATE_SIZE: usize = 8;

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
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
    pub pyth_price_info_0: AccountInfo<'info>,
    pub pyth_price_info_1: AccountInfo<'info>,
    pub pyth_price_info_2: AccountInfo<'info>,
    pub pyth_price_info_3: AccountInfo<'info>,
    pub pyth_price_info_4: AccountInfo<'info>,
    pub pyth_price_info_5: AccountInfo<'info>,
    pub pyth_price_info_6: AccountInfo<'info>,
    pub pyth_price_info_7: AccountInfo<'info>,

    pub clock: Sysvar<'info, Clock>,
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: usize) -> ProgramResult {
    msg!("ix=refresh_one_price");
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let pyth_price_info = &ctx.accounts.pyth_price_info;

    // Check that the provided pyth account is the one referenced in oracleMapping
    if oracle_mappings.price_info_accounts[token] != pyth_price_info.key() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;
    let clock = &ctx.accounts.clock;

    let price = get_price(pyth_price_info)?;

    let to_update = &mut oracle.prices[token];

    to_update.price = price;
    to_update.last_updated_slot = clock.slot; // TODO Use price `valid_slot` for pyth prices

    Ok(())
}

pub fn refresh_batch_prices(ctx: Context<RefreshBatch>, first_token: usize) -> ProgramResult {
    msg!("ix=refresh_batch_prices");
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;
    let clock = ctx.accounts.clock.slot; // TODO Use price `valid_slot` for pyth prices

    let range = RangeInclusive::new(first_token, first_token + BATCH_UPDATE_SIZE);
    let partial_mappings = &oracle_mappings.price_info_accounts[range.clone()];
    let partial_prices = &mut oracle.prices[range];

    //Easy rebuild of the missing array
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
    //Pubkey::new_from_array([0_u8; 32]);

    for ((expected, received), to_update) in partial_mappings
        .iter()
        .zip(pyth_prices_info.into_iter())
        .zip(partial_prices.iter_mut())
    {
        // Ignore empty accounts
        // TODO is that possible?
        if received.key() == zero_pk {
            continue;
        }
        // Check that the provided pyth accounts are the one referenced in oracleMapping
        if *expected != received.key() {
            return Err(ScopeError::UnexpectedAccount.into());
        }
        let price = get_price(received)?;
        to_update.price = price;
        to_update.last_updated_slot = clock;
    }

    Ok(())
}
