use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct Initialize<'info> {
    /// CHECK: At creation admin can be anyone, this ix can't override an existing feed
    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,

    // Set space to max size here
    // The ability to create multiple feeds is mostly useful for tests
    #[account(init, seeds = [b"conf", feed_name.as_bytes()], bump, payer = admin, space = 8 + std::mem::size_of::<crate::Configuration>())]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    // Account is pre-reserved/payed outside the program
    #[account(zero)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,

    // Account is pre-reserved/payed outside the program
    #[account(zero)]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
}

pub fn process(ctx: Context<Initialize>, _: String) -> Result<()> {
    // Initialize oracle mapping account
    let _mappings = ctx.accounts.oracle_mappings.load_init()?;

    // Initialize oracle price account
    let oracle_pbk = ctx.accounts.oracle_mappings.key();
    let mut oracle_prices = ctx.accounts.oracle_prices.load_init()?;
    oracle_prices.oracle_mappings = oracle_pbk;

    // Initialize configuration account
    let prices_pbk = ctx.accounts.oracle_prices.key();
    let admin = ctx.accounts.admin.key();
    let mut configuration = ctx.accounts.configuration.load_init()?;
    configuration.admin = admin;
    configuration.oracle_mappings = oracle_pbk;
    configuration.oracle_prices = prices_pbk;

    Ok(())
}
