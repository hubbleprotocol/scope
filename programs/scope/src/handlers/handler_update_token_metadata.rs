use crate::{ScopeError, UpdateTokenMetadataMode};
use anchor_lang::prelude::*;
use num_enum::TryFromPrimitive;

#[derive(Accounts)]
#[instruction(token:usize, price_type: u8, feed_name: String)]
pub struct UpdateTokensMetadata<'info> {
    pub admin: Signer<'info>,
    #[account(seeds = [b"conf", feed_name.as_bytes()], bump, has_one = admin, has_one = tokens_metadata)]
    pub configuration: AccountLoader<'info, crate::Configuration>,

    #[account(mut)]
    pub tokens_metadata: AccountLoader<'info, crate::TokensMetadata>,
}

pub fn process(
    ctx: Context<UpdateTokensMetadata>,
    index: u64,
    mode: u64,
    value: Vec<u8>,
    _: String,
) -> Result<()> {
    let mut tokens_metadata = ctx.accounts.tokens_metadata.load_mut()?;

    let token_metadata = tokens_metadata
        .price_info_accounts
        .get_mut(index as usize)
        .ok_or(ScopeError::BadTokenNb)?;

    let mode: UpdateTokenMetadataMode = mode
        .try_into()
        .map_err(|_| ScopeError::InvalidTokenUpdateMode)?;
    match mode {
        UpdateTokenMetadataMode::MaxPriceAgeSeconds => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Setting token max age for index {:?} to {}", index, value);
            token_metadata.max_age_price_seconds = value;
        }
        UpdateTokenMetadataMode::Name => {
            token_metadata.name.copy_from_slice(&value);
            let str_name = std::str::from_utf8(&token_metadata.name).unwrap();
            msg!("Setting token name for index {} to {}", index, str_name);
        }
    }

    Ok(())
}
