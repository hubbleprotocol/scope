use crate::program::Oracle;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    // Space = account discriminator + (price + exposant + timestamp)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (8+8+8)*256)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    // Space = account discriminator + (PubKey size)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (32)*256)]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    #[account()]
    pub admin: Signer<'info>,
    #[account(constraint = program.programdata_address() == Some(program_data.key()))]
    pub program: Program<'info, Oracle>,
    #[account(constraint = program_data.upgrade_authority_address == Some(admin.key()))]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}
