use crate::program::Scope;
use crate::utils::{check_context, pyth};
use crate::OracleMappings;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateOracleMapping<'info> {
    pub admin: Signer<'info>,
    #[account(constraint = program.programdata_address() == Some(program_data.key()))]
    pub program: Program<'info, Scope>,
    #[account(constraint = program_data.upgrade_authority_address == Some(admin.key()))]
    pub program_data: Account<'info, ProgramData>,
    #[account(mut)]
    pub oracle_mappings: AccountLoader<'info, OracleMappings>,
    /// CHECK: We trust the admin to provide a trustable account here.
    pub pyth_price_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<UpdateOracleMapping>, token: usize) -> ProgramResult {
    check_context(&ctx)?;

    let new_price_pubkey = ctx.accounts.pyth_price_info.key();
    let mut oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;
    let current_price_pubkey = &mut oracle_mappings.price_info_accounts[token];

    if new_price_pubkey.eq(current_price_pubkey) {
        // Key already set
        return Ok(());
    }

    let pyth_price_info = ctx.accounts.pyth_price_info.as_ref();
    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth_client::cast::<pyth_client::Price>(&pyth_price_data);

    pyth::validate_pyth_price(pyth_price)?;

    // Every check succeeded, replace current with new
    *current_price_pubkey = new_price_pubkey;

    Ok(())
}
