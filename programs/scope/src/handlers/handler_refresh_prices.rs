use crate::utils::OracleType;
use crate::{utils::get_price, ScopeError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::{sol_log, sol_log_64};

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub price_info: AccountInfo<'info>,
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
    let price_info = &ctx.accounts.price_info;

    // Check that the provided account is the one referenced in oracleMapping
    if oracle_mappings.price_info_accounts[token] != price_info.key() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let price_type: OracleType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    let mut remaining_iter = ctx.remaining_accounts.iter();
    let price = get_price(price_type, price_info, &mut remaining_iter)?;

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
    // Check the received token list is at least as long as the number of provided accounts
    if tokens.len() > ctx.remaining_accounts.len() {
        return Err(ScopeError::AccountsAndTokenMismatch.into());
    }

    let zero_pk: Pubkey = Pubkey::default();

    let mut accounts_iter = ctx.remaining_accounts.iter();

    for &token_nb in tokens.iter() {
        let token_idx: usize = token_nb.into();
        let oracle_mapping = oracle_mappings
            .price_info_accounts
            .get(token_idx)
            .ok_or(ScopeError::BadTokenNb)?;
        let price_type: OracleType = oracle_mappings.price_types[token_idx]
            .try_into()
            .map_err(|_| ScopeError::BadTokenType)?;
        let received_account = accounts_iter
            .next()
            .ok_or(ScopeError::AccountsAndTokenMismatch)?;
        // Ignore unset mapping accounts
        if zero_pk == *oracle_mapping {
            continue;
        }
        // Check that the provided oracle accounts are the one referenced in oracleMapping
        if oracle_mappings.price_info_accounts[token_idx] != received_account.key() {
            return Err(ScopeError::UnexpectedAccount.into());
        }
        match get_price(price_type, received_account, &mut accounts_iter) {
            Ok(price) => {
                let to_update = oracle_prices
                    .get_mut(token_idx)
                    .ok_or(ScopeError::BadTokenNb)?;
                *to_update = price;
            }
            Err(_) => {
                sol_log("Price skipped as validation failed (token, type)");
                sol_log_64(token_idx as u64, price_type as u64, 0, 0, 0);
            }
        };
    }

    Ok(())
}
