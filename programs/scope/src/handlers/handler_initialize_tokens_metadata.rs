use anchor_lang::prelude::*;
use solana_program::config;

use crate::TokenMetadata;

#[derive(Accounts)]
#[instruction(feed_name: String)]
pub struct InitializeTokensMetadata<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin)]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    #[account(init, payer = admin, space = 8 + std::mem::size_of::<crate::TokensMetadata>())]
    pub token_metadatas: AccountLoader<'info, crate::TokensMetadata>,

    pub system_program: Program<'info, System>,
}

pub fn process(ctx: Context<InitializeTokensMetadata>, _: String) -> Result<()> {
    let mut token_metadatas = ctx.accounts.token_metadatas.load_init()?;
    token_metadatas.price_info_accounts = [TokenMetadata::default(); crate::MAX_ENTRIES];

    let mut configuration = ctx.accounts.configuration.load_mut()?;
    configuration.tokens_metadata = ctx.accounts.token_metadatas.key();
    Ok(())
}
