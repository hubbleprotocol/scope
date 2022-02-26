use crate::program::Scope;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    // `program` could be removed as the check could use find program address with id()
    // to find program_data...but compute units is not constant
    #[account(constraint = program.programdata_address() == Some(program_data.key()))]
    pub program: Program<'info, Scope>,

    // program_data is findProgramAddress(programId, "BPFLoaderUpgradeab1e11111111111111111111111")
    #[account(constraint = program_data.upgrade_authority_address == Some(admin.key()))]
    pub program_data: Account<'info, ProgramData>,

    pub system_program: Program<'info, System>,

    // Space = account discriminator + account size
    #[account(init, seeds = [b"prices", feed_name.as_bytes()], bump, payer = admin, space = 8 + std::mem::size_of::<crate::OraclePrices>())]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,

    // Space = account discriminator + account size
    #[account(init, seeds = [b"mappings", feed_name.as_bytes()], bump, payer = admin, space = 8 + std::mem::size_of::<crate::OracleMappings>())]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
}

pub fn process(ctx: Context<Initialize>, _: String) -> ProgramResult {
    let oracle_pbk = ctx.accounts.oracle_mappings.key();
    let mut oracle_prices = ctx.accounts.oracle_prices.load_init()?;
    oracle_prices.oracle_mappings = oracle_pbk;
    Ok(())
}
