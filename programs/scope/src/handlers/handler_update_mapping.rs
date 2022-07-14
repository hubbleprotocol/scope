use crate::program::Scope;
use crate::utils::{check_context, validate_oracle_account, OracleType};
use crate::{OracleMappings, ScopeError};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateOracleMapping<'info> {
    pub admin: Signer<'info>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
    pub program: Program<'info, Scope>,
    #[account(constraint = program_data.upgrade_authority_address == Some(admin.key()))]
    pub program_data: Account<'info, ProgramData>,
    #[account(mut)]
    pub oracle_mappings: AccountLoader<'info, OracleMappings>,
    /// CHECK: We trust the admin to provide a trustable account here. Some basic sanity checks are done based on type
    pub price_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<UpdateOracleMapping>, token: usize, price_type: u8) -> Result<()> {
    check_context(&ctx)?;

    let new_price_pubkey = ctx.accounts.price_info.key();
    let mut oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;
    let ref_price_pubkey = oracle_mappings
        .price_info_accounts
        .get_mut(token)
        .ok_or(ScopeError::BadTokenNb)?;
    let price_type: OracleType = price_type
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    let price_info = ctx.accounts.price_info.as_ref();

    validate_oracle_account(price_type, price_info)?;

    // Every check succeeded, replace current with new
    *ref_price_pubkey = new_price_pubkey;
    oracle_mappings.price_types[token] = price_type.into();

    Ok(())
}
