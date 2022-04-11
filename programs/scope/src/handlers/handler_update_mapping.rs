use crate::program::Scope;
use crate::utils::{check_context, pyth};
use crate::{utils, OracleMappings, ScopeError};
use anchor_lang::prelude::*;
use utils::PriceType;

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
    pub price_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<UpdateOracleMapping>, token: usize, price_type: u8) -> ProgramResult {
    check_context(&ctx)?;

    let new_price_pubkey = ctx.accounts.price_info.key();
    let mut oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;
    let current_price_pubkey = &mut oracle_mappings.price_info_accounts[token];

    if new_price_pubkey.eq(current_price_pubkey) {
        // Key already set
        return Ok(());
    }

    let price_type_enum: PriceType = price_type
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    if price_type_enum == PriceType::Pyth {
        let price_info = ctx.accounts.price_info.as_ref();
        let price_data = price_info.try_borrow_data()?;
        let pyth_price = pyth_client::cast::<pyth_client::Price>(&price_data);

        pyth::validate_pyth_price(pyth_price)?;
        // Every check succeeded, replace current with new
    }
    *current_price_pubkey = new_price_pubkey;

    //let stored_price_type = &mut oracle_mappings.price_types[token];
    oracle_mappings.price_types[token] = price_type;

    Ok(())
}
