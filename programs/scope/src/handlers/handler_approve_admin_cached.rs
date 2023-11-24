use anchor_lang::{prelude::*, Accounts};

use crate::oracles::check_context;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct ApproveAdminCached<'info> {
    admin_cached: Signer<'info>,

    #[account(mut, seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin_cached)]
    pub configuration: AccountLoader<'info, crate::Configuration>,
}

pub fn process(ctx: Context<ApproveAdminCached>, _: String) -> Result<()> {
    check_context(&ctx)?;

    let configuration = &mut ctx.accounts.configuration.load_mut()?;

    msg!(
        "old admin {} new admin {}",
        configuration.admin,
        configuration.admin_cached
    );

    configuration.admin = configuration.admin_cached;

    Ok(())
}
