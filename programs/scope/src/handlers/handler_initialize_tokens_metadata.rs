use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct InitializeTokensMetadata<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin)]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    #[account(zero)]
    pub token_metadatas: AccountLoader<'info, crate::TokenMetadatas>,

    pub system_program: Program<'info, System>,
}

pub fn process(ctx: Context<InitializeTokensMetadata>, _: String) -> Result<()> {
    let _ = ctx.accounts.token_metadatas.load_init()?;

    let mut configuration = ctx.accounts.configuration.load_mut()?;
    configuration.tokens_metadata = ctx.accounts.token_metadatas.key();
    Ok(())
}
