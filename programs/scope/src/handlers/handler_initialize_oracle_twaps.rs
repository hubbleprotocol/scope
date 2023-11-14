use anchor_lang::prelude::*;

// todo: remove this handler after the setup of the TWAP account
#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct InitializeOracleTwaps<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin)]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    #[account(zero)]
    pub oracle_twaps: AccountLoader<'info, crate::OracleTwaps>,

    pub system_program: Program<'info, System>,
}

pub fn process(ctx: Context<InitializeOracleTwaps>, _: String) -> Result<()> {
    let twaps_pbk = ctx.accounts.oracle_twaps.key();

    let mut configuration: std::cell::RefMut<'_, crate::Configuration> =
        ctx.accounts.configuration.load_mut()?;
    configuration.oracle_twaps = twaps_pbk;

    // Initialize oracle twap account
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_init()?;
    oracle_twaps.oracle_prices = configuration.oracle_prices;
    oracle_twaps.oracle_mappings = configuration.oracle_mappings;

    Ok(())
}
