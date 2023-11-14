use anchor_lang::prelude::*;
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;

use crate::oracles::check_context;

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
    /// CHECK: Sysvar fixed address
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<ResetTwap>, token: usize, _: String) -> Result<()> {
    check_context(&ctx)?;

    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let oracle = ctx.accounts.oracle_prices.load_mut()?;
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_mut()?;

    let clock = Clock::get()?;

    let price = oracle.prices[token].price;

    crate::oracles::twap::reset_twap(
        &oracle_mappings,
        &mut oracle_twaps,
        token,
        price,
        clock.unix_timestamp as u64,
        clock.slot,
    )?;

    Ok(())
}
