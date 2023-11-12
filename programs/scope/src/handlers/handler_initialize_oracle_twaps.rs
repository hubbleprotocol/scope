use anchor_lang::prelude::*;

use crate::OracleMappings;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct InitializeOracleTwaps<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin, has_one = oracle_prices)]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,

    pub oracle_mappings: AccountLoader<'info, OracleMappings>,

    #[account(zero)]
    pub oracle_twaps: AccountLoader<'info, crate::OracleTwaps>,

    pub system_program: Program<'info, System>,
}

pub fn process(ctx: Context<InitializeOracleTwaps>, _: String) -> Result<()> {
    let twaps_pbk = ctx.accounts.oracle_twaps.key();
    let prices_pbk = ctx.accounts.oracle_prices.key();

    // Initialize oracle twap account
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_init()?;
    oracle_twaps.oracle_prices = prices_pbk;

    let mut configuration: std::cell::RefMut<'_, crate::Configuration> =
        ctx.accounts.configuration.load_mut()?;
    configuration.oracle_twaps = twaps_pbk;

    Ok(())
}
