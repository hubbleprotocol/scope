use std::convert::TryInto;

use anchor_lang::prelude::*;
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;

use crate::{
    oracles::{check_context, get_price, OracleType},
    ScopeError,
};

#[derive(Accounts)]
#[instruction(token:u64, feed_name: String)]
pub struct ResetTwap<'info> {
    pub admin: Signer<'info>,

    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account(seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin, has_one = oracle_mappings)]
    pub configuration: AccountLoader<'info, crate::Configuration>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    #[account(mut, has_one = oracle_prices)]
    pub oracle_twaps: AccountLoader<'info, crate::OracleTwaps>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub price_info: AccountInfo<'info>,
    /// CHECK: Sysvar fixed address
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<ResetTwap>, token: usize, _: String) -> Result<()> {
    check_context(&ctx)?;

    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let price_info = &ctx.accounts.price_info;
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_mut()?;

    let mut remaining_iter = ctx.remaining_accounts.iter();
    let clock = Clock::get()?;

    let price_type: OracleType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;
    let price = get_price(
        price_type,
        price_info,
        &mut remaining_iter,
        &clock,
        &oracle_twaps,
        &oracle_mappings,
        token,
    )?;

    if oracle_mappings.should_use_twap(token) {
        crate::oracles::twap::reset_twap(
            &oracle_mappings,
            &mut oracle_twaps,
            token,
            price.price,
            clock.unix_timestamp as u64,
            clock.slot,
        );
    };

    Ok(())
}
