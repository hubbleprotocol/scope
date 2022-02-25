use crate::program::Scope;
use anchor_lang::prelude::*;

#[derive(Accounts)]
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
    // Space = account discriminator + (price + exponent + timestamp)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (8+8+8)*256)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    // Space = account discriminator + (PubKey size)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (32)*256)]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
}
