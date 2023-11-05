use std::convert::TryInto;

use anchor_lang::prelude::*;
use solana_program::{
    instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
    pubkey,
    sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked, ID as SYSVAR_INSTRUCTIONS_ID,
    },
};

use crate::{
    oracles::{get_price, get_twap_from_observations, OracleType},
    OracleTwaps, Price, ScopeError, TWAP_INTERVAL_SECONDS, TWAP_NUM_OBS,
};

const COMPUTE_BUDGET_ID: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,

    #[account(mut, has_one = oracle_prices, has_one = tokens_metadata)]
    pub oracle_twaps: AccountLoader<'info, crate::OracleTwaps>,

    #[account()]
    pub tokens_metadata: AccountLoader<'info, crate::TokenMetadatas>,

    /// CHECK: In ix, check the account is in `oracle_mappings`
    pub price_info: AccountInfo<'info>,

    /// CHECK: Sysvar fixed address
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account_info: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RefreshList<'info> {
    #[account()]
    pub tokens_metadata: AccountLoader<'info, crate::TokenMetadatas>,

    #[account(mut, has_one = oracle_mappings)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,

    #[account()]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,

    #[account(mut, has_one = oracle_prices)]
    pub oracle_twaps: AccountLoader<'info, crate::OracleTwaps>,

    /// CHECK: Sysvar fixed address
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account_info: AccountInfo<'info>,
    // Note: use remaining accounts as price accounts
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: usize) -> Result<()> {
    check_execution_ctx(&ctx.accounts.instruction_sysvar_account_info)?;

    let oracle_mappings = ctx.accounts.oracle_mappings.load()?;
    let price_info = &ctx.accounts.price_info;
    let tokens_metadata = ctx.accounts.tokens_metadata.load()?;
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_mut()?;
    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;

    // Check that the provided account is the one referenced in oracleMapping
    msg!("price_info: {:?}", price_info.key());
    msg!(
        "oracle_mappings.price_info_accounts[token]: {:?}",
        oracle_mappings.price_info_accounts[token]
    );
    msg!("Token: {:?}", token);
    if oracle_mappings.price_info_accounts[token] != price_info.key() {
        return err!(ScopeError::UnexpectedAccount);
    }

    let price_type: OracleType = oracle_mappings.price_types[token]
        .try_into()
        .map_err(|_| ScopeError::BadTokenType)?;

    let mut remaining_iter = ctx.remaining_accounts.iter();
    let clock = Clock::get()?;

    // If price type is normal (not twap, not derived) and twap is enabled, append twap
    let price = if price_type.is_twap() {
        msg!("Is Twap");
        // Then start calculating the twap
        let source = tokens_metadata.get_twap_source(token);
        let twap = get_twap_from_observations(price_type, &oracle_twaps, source, &clock)?;
        twap
    } else {
        msg!("Is Price");
        let mut price = get_price(price_type, price_info, &mut remaining_iter, &clock)?;

        // TODO: should we get rid of this
        price.index = token.try_into().unwrap();

        if tokens_metadata.should_store_twap_observations(token) {
            store_observation(
                &mut oracle_twaps,
                token,
                price.price,
                clock.unix_timestamp as u64,
                clock.slot,
            );
        }

        price
    };

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
    check_execution_ctx(&ctx.accounts.instruction_sysvar_account_info)?;

    let oracle_mappings = &ctx.accounts.oracle_mappings.load()?;
    let mut oracle_twaps = ctx.accounts.oracle_twaps.load_mut()?;
    let tokens_metadata = ctx.accounts.tokens_metadata.load()?;

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
            msg!(
                "Invalid price account: {}, expected: {}",
                received_account.key(),
                oracle_mappings.price_info_accounts[token_idx]
            );
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

                if tokens_metadata.should_store_twap_observations(token_idx) {
                    store_observation(
                        &mut oracle_twaps,
                        token_idx,
                        price.price,
                        clock.unix_timestamp as u64,
                        clock.slot,
                    );
                }

                *to_update = price;
                to_update.index = token_nb;
            }
            Err(_) => {
                // Skip the error, details is already logged in get_price and formatting here cost a lot of CU
                msg!(
                    "Price skipped as validation failed (token {}, type {:?})",
                    token_idx,
                    price_type
                );
            }
        };
    }

    Ok(())
}

/// Ensure that the refresh instruction is executed directly to avoid any manipulation:
///
/// - Check that the current instruction is executed by our program id (not in CPI).
/// - Check that instructions preceding the refresh are compute budget instructions.
fn check_execution_ctx(instruction_sysvar_account_info: &AccountInfo) -> Result<()> {
    let current_index: usize = load_current_index_checked(instruction_sysvar_account_info)?.into();

    // 1- Check that the current instruction is executed by our program id (not in CPI).
    let current_ix = load_instruction_at_checked(current_index, instruction_sysvar_account_info)?;

    // the current ix must be executed by our program id. otherwise, it's a CPI.
    if crate::ID != current_ix.program_id {
        return err!(ScopeError::RefreshInCPI);
    }

    // The current stack height must be the initial one. Otherwise, it's a CPI.
    if get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT {
        return err!(ScopeError::RefreshInCPI);
    }

    // 2- Check that instructions preceding the refresh are compute budget instructions.
    for ixn in 0..current_index {
        let ix = load_instruction_at_checked(ixn, instruction_sysvar_account_info)?;
        if ix.program_id != COMPUTE_BUDGET_ID {
            return err!(ScopeError::RefreshWithUnexpectedIxs);
        }
    }

    Ok(())
}

fn store_observation(
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    current_ts: u64,
    current_slot: u64,
) {
    let twap_buffer = &mut oracle_twaps.twap_buffers[token];
    let (curr_index, next_index): (usize, usize) = if twap_buffer.unix_timestamps[0] == 0 {
        let next_index = 0;
        let curr_index = 0;
        (curr_index, next_index)
    } else {
        let curr_index = twap_buffer.curr_index as usize;
        let next_index = (curr_index + 1) % TWAP_NUM_OBS;
        (curr_index, next_index)
    };

    let last_timestamp = twap_buffer.unix_timestamps[curr_index];
    if current_ts.saturating_sub(last_timestamp) >= TWAP_INTERVAL_SECONDS {
        twap_buffer.observations[next_index] = price;
        twap_buffer.unix_timestamps[next_index] = current_ts;
        twap_buffer.slots[next_index] = current_slot;
        twap_buffer.curr_index = next_index as u64;
    }
}
