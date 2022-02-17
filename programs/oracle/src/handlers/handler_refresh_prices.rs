use std::convert::TryInto;

use crate::{utils::pyth::get_price, ScopeError};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    pub pyth_price_info: AccountInfo<'info>,
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

    let price = get_price(pyth_price_info, token.try_into().unwrap())?;

    let to_update = &mut oracle.prices[token];

    to_update.price = price;
    to_update.last_updated_slot = clock.slot; // TODO Use price `valid_slot` for pyth prices

    Ok(())
}
