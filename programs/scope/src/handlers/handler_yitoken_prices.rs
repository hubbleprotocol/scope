use anchor_lang::prelude::*;
use std::str::FromStr;
use crate::{ScopeError};
use crate::utils::{PriceType};
use crate::utils::yitoken::get_price;
use anchor_spl::token::{Mint, TokenAccount};

static YI_MINT_ACC_STR: &str = "CGczF9uYdSVXmSr9swMafhF1ktHsi6ygcgTHWL71XNZ9";
static YI_UNDERLYING_TOKEN_ACC_STR: &str = "EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc";

#[derive(Accounts)]
pub struct RefreshYiToken<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    #[account()]
    pub yi_underlying_tokens: Account<'info, TokenAccount>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    #[account()]
    pub yi_mint: Account<'info, Mint>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn refresh_yi_token(ctx: Context<RefreshYiToken>, token: usize) -> Result<()> {
    let yi_mint_account: Pubkey = Pubkey::from_str(YI_MINT_ACC_STR).unwrap();
    let yi_underlying_token_account: Pubkey = Pubkey::from_str(YI_UNDERLYING_TOKEN_ACC_STR).unwrap();
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let price_type: PriceType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    if yi_underlying_token_account != ctx.accounts.yi_underlying_tokens.key() || yi_mint_account != ctx.accounts.yi_mint.key() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let price = get_price(price_type, &ctx.accounts.yi_underlying_tokens, &ctx.accounts.yi_mint, ctx.accounts.clock.slot)?;

    oracle.prices[token] = price;

    Ok(())
}