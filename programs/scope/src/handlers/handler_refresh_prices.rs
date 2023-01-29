use std::convert::TryInto;

use anchor_lang::prelude::*;

use crate::{
    oracles::{get_price, OracleType},
    ScopeError,
};

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
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,

    pub clock: Sysvar<'info, Clock>,
    // Note: use remaining accounts as price accounts
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: usize) -> Result<()> {
    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let price_info = &ctx.accounts.price_info;

    // Check that the provided account is the one referenced in oracleMapping
    if oracle_mappings.price_info_accounts[token] != price_info.key() {
        return err!(ScopeError::UnexpectedAccount);
    }

    let price_type: OracleType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    let mut remaining_iter = ctx.remaining_accounts.iter();
    let clock = Clock::get()?;
    let mut price = get_price(price_type, price_info, &mut remaining_iter, &clock)?;
    price.index = token.try_into().unwrap();

    // Only load when needed, allows prices computation to use scope chain
    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    msg!(
        "tk {}, {:?}: {:?} to {:?} | prev_slot: {:?}, new_slot: {:?}, crt_slot: {:?}",
        token,
        price_type,
        oracle.prices[token].price.value,
        price.price.value,
        oracle.prices[token].last_updated_slot,
        price.last_updated_slot,
        clock.slot,
    );

    oracle.prices[token] = price;

    Ok(())
}

pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: &[u16]) -> Result<()> {
    let oracle_mappings = &ctx.accounts.oracle_mappings.load()?;

    // Check that the received token list is not too long
    if tokens.len() > crate::MAX_ENTRIES {
        return Err(ProgramError::InvalidArgument.into());
    }
    // Check the received token list is at least as long as the number of provided accounts
    if tokens.len() > ctx.remaining_accounts.len() {
        return err!(ScopeError::AccountsAndTokenMismatch);
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
            return err!(ScopeError::UnexpectedAccount);
        }
        let clock = Clock::get()?;
        match get_price(price_type, received_account, &mut accounts_iter, &clock) {
            Ok(price) => {
                // Only temporary load as mut to allow prices to be computed based on a scope chain
                // from the price feed that is currently updated
                let mut oracle_prices = ctx.accounts.oracle_prices.load_mut()?;
                let to_update = oracle_prices
                    .prices
                    .get_mut(token_idx)
                    .ok_or(ScopeError::BadTokenNb)?;

                msg!(
                    "tk {}, {:?}: {:?} to {:?} | prev_slot: {:?}, new_slot: {:?}, crt_slot: {:?}",
                    token_idx,
                    price_type,
                    to_update.price.value,
                    price.price.value,
                    to_update.last_updated_slot,
                    price.last_updated_slot,
                    clock.slot,
                );

                *to_update = price;
                to_update.index = token_nb;
            }
            Err(e) => {
                msg!(
                    "Price skipped as validation failed (token {}, type {:?}, err {:?})",
                    token_idx,
                    price_type,
                    e
                );
            }
        };
    }

    Ok(())
}
