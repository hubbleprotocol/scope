pub mod pyth;

use crate::ScopeError;
use anchor_lang::prelude::{error, Context, Result};

pub fn check_context<T>(ctx: &Context<T>) -> Result<()> {
    //make sure there are no extra accounts
    if !ctx.remaining_accounts.is_empty() {
        return Err(error!(ScopeError::UnexpectedAccount));
    }

    Ok(())
}
