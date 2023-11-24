use anchor_lang::{prelude::*, Accounts};

use crate::oracles::check_context;

#[derive(Accounts)]
#[instruction(new_admin: Pubkey, feed_name: String)]
pub struct SetAdminCached<'info> {
    admin: Signer<'info>,

    #[account(mut, seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin)]
    pub configuration: AccountLoader<'info, crate::Configuration>,
}

pub fn process(ctx: Context<SetAdminCached>, new_admin: Pubkey, _: String) -> Result<()> {
    check_context(&ctx)?;

    let configuration = &mut ctx.accounts.configuration.load_mut()?;

    configuration.admin_cached = new_admin;

    Ok(())
}
