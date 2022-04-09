use crate::utils::yitoken::get_price;
use crate::utils::PriceType;
use crate::ScopeError;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

const YI_MINT_ACCOUNT: Pubkey = Pubkey::new_from_array([
    0xa7, 0x70, 0xf5, 0x7b, 0xa9, 0x0c, 0x5d, 0xf7, 0x42, 0x46, 0x3b, 0x34, 0xd3, 0x71, 0x97, 0x15,
    0xe7, 0x0d, 0x62, 0x98, 0xce, 0xc1, 0x11, 0x8a, 0xe6, 0x4e, 0x03, 0x36, 0x4d, 0xe2, 0xec, 0xec,
]);
const YI_UNDERLYING_TOKEN_ACCOUNT: Pubkey = Pubkey::new_from_array([
    0xc4, 0x51, 0x15, 0x7d, 0xe9, 0xe3, 0xf9, 0x72, 0x93, 0xe7, 0x1a, 0xf0, 0x65, 0x3b, 0x33, 0xfe,
    0xcf, 0x01, 0xa1, 0x2b, 0xd9, 0xc3, 0x5e, 0xe3, 0x18, 0xda, 0x19, 0x14, 0xba, 0xdf, 0xb4, 0x8f,
]);

#[derive(Accounts)]
pub struct RefreshYiToken<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    /// CHECK: In ix, check the account vs a constant
    #[account()]
    pub yi_underlying_tokens: Account<'info, TokenAccount>,
    /// CHECK: In ix, check the account vs a constant
    #[account()]
    pub yi_mint: Account<'info, Mint>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn refresh_yi_token(ctx: Context<RefreshYiToken>, token: usize) -> ProgramResult {
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let price_type: PriceType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    if YI_UNDERLYING_TOKEN_ACCOUNT != ctx.accounts.yi_underlying_tokens.key()
        || YI_MINT_ACCOUNT != ctx.accounts.yi_mint.key()
    {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let price = get_price(
        price_type,
        &ctx.accounts.yi_underlying_tokens,
        &ctx.accounts.yi_mint,
        ctx.accounts.clock.slot,
    )?;

    oracle.prices[token] = price;

    Ok(())
}
