use crate::oracles::{check_context, validate_oracle_account, OracleType};
use crate::{OracleMappings, ScopeError};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(token:usize, price_type: u8, feed_name: String)]
pub struct UpdateOracleMapping<'info> {
    pub admin: Signer<'info>,
    #[account(seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin, has_one = oracle_mappings)]
    pub configuration: AccountLoader<'info, crate::Configuration>,
    #[account(mut)]
    pub oracle_mappings: AccountLoader<'info, OracleMappings>,
    /// CHECK: We trust the admin to provide a trustable account here. Some basic sanity checks are done based on type
    pub price_info: AccountInfo<'info>,
}

pub fn process(
    ctx: Context<UpdateOracleMapping>,
    token: usize,
    price_type: u8,
    _: String,
) -> Result<()> {
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
