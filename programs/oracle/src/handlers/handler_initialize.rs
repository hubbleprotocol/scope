use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    // Space = account discriminator + (price + exposant + timestamp)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (8+8+8)*512)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    // Space = account discriminator + (PubKey size)*max_stored_prices
    #[account(init, payer = admin, space = 8 + (32)*512)]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    pub system_program: Program<'info, System>,
}
