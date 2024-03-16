use anchor_lang::prelude::*;

use crate::MintsToScopeChains;

#[derive(Accounts)]
#[instruction(scope_chains: Vec<[u16; 4]>)]
pub struct CloseMintMap<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(has_one = admin)]
    pub configuration: AccountLoader<'info, crate::Configuration>,
    #[account(mut, close = admin, constraint = mappings.oracle_prices == configuration.load()?.oracle_prices)]
    pub mappings: Account<'info, MintsToScopeChains>,

    pub system_program: Program<'info, System>,
}

pub fn process(_ctx: Context<CloseMintMap>) -> Result<()> {
    Ok(())
}
