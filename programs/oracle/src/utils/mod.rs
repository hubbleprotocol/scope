pub mod pyth;

use crate::ScopeError;
use anchor_lang::prelude::{Context, ProgramResult};

pub fn check_context<T>(ctx: &Context<T>) -> ProgramResult {
    //make sure there are no extra accounts
    if !ctx.remaining_accounts.is_empty() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    Ok(())
}
