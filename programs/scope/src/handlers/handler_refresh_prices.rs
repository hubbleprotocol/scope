use crate::utils::PriceType;
use crate::{utils::get_price, ScopeError};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub pyth_price_info: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct RefreshList<'info> {
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,

    pub clock: Sysvar<'info, Clock>,
    // Note: use remaining accounts as price accounts
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: usize) -> ProgramResult {
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let pyth_price_info = &ctx.accounts.pyth_price_info;

    // Check that the provided pyth account is the one referenced in oracleMapping
    if oracle_mappings.price_info_accounts[token] != pyth_price_info.key() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let price_type: PriceType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let price = get_price(price_type, pyth_price_info)?;

    oracle.prices[token] = price;

    Ok(())
}

pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: &[u16]) -> ProgramResult {
    let oracle_mappings = &ctx.accounts.oracle_mappings.load()?;
    let oracle_prices = &mut ctx.accounts.oracle_prices.load_mut()?.prices;

    // Check that the received token list is not too long
    if tokens.len() > crate::MAX_ENTRIES {
        return Err(ProgramError::InvalidArgument);
    }
    // Check the received token list is as long as the number of provided accounts
    if tokens.len() != ctx.remaining_accounts.len() {
        return Err(ScopeError::AccountsAndTokenMismatch.into());
    }

    let zero_pk: Pubkey = Pubkey::default();

    for (&token_nb, received_account) in tokens.iter().zip(ctx.remaining_accounts.iter()) {
        let token_idx: usize = token_nb.into();
        let oracle_mapping = oracle_mappings
            .price_info_accounts
            .get(token_idx)
            .ok_or(ScopeError::BadTokenNb)?;
        let price_type: PriceType = oracle_mappings.price_types[token_idx]
            .try_into()
            .map_err(|_| ScopeError::BadTokenType)?;
        // Ignore unset mapping accounts
        if zero_pk == *oracle_mapping {
            continue;
        }
        // Check that the provided pyth accounts are the one referenced in oracleMapping
        if oracle_mappings.price_info_accounts[token_idx] != received_account.key() {
            return Err(ScopeError::UnexpectedAccount.into());
        }
        match get_price(price_type, received_account) {
            Ok(price) => {
                let to_update = oracle_prices
                    .get_mut(token_idx)
                    .ok_or(ScopeError::BadTokenNb)?;
                *to_update = price;
            }
            Err(_) => msg!("Price skipped as validation failed"), // No format as its a bit costly
        };
    }

    Ok(())
}
