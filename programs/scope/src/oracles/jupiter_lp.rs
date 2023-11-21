use anchor_lang::{prelude::*, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::decimal::Decimal;
use solana_program::instruction::Instruction;
use solana_program::program::{get_return_data, invoke};
use solana_program::program_pack::Pack;

use crate::utils::account_deserialize;
use crate::{DatedPrice, Result, ScopeError};

pub use jup_perp_itf as perpetuals;
pub use perpetuals::utils::{check_mint_pk, get_mint_pk};
pub const POOL_VALUE_SCALE_DECIMALS: u8 = 6;

/// Gives the price of 1 JLP token in USD
///
/// Uses the AUM of the pool and the supply of the JLP token to compute the price
pub fn get_price_no_recompute<'a, 'b>(
    jup_pool_acc: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    let jup_pool_pk = jup_pool_acc.key;
    let jup_pool: perpetuals::Pool = account_deserialize(jup_pool_acc)?;

    let mint_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    check_mint_pk(jup_pool_pk, mint_acc.key, jup_pool.lp_token_bump)
        .map_err(|_| ScopeError::UnexpectedAccount)?;

    let mint = {
        let mint_borrow = mint_acc.data.borrow();
        Mint::unpack(&mint_borrow)
    }?;

    let lp_value = jup_pool.aum_usd;
    let lp_token_supply = mint.supply;

    // This is a sanity check to make sure the mint is configured as expected
    // This allows to just divide the two values to get the price
    require_eq!(mint.decimals, POOL_VALUE_SCALE_DECIMALS);

    let price_dec = Decimal::from(lp_value) / lp_token_supply;
    let dated_price = DatedPrice {
        price: price_dec.into(),
        // TODO: find a way to get the last update time
        last_updated_slot: clock.slot,
        unix_timestamp: u64::try_from(clock.unix_timestamp).unwrap(),
        ..Default::default()
    };

    Ok(dated_price)
}

pub fn validate_jlp_pool(account: &AccountInfo) -> Result<()> {
    let _jlp_pool: perpetuals::Pool = account_deserialize(account)?;
    Ok(())
}

/// Get the price of 1 JLP token in USD
///
/// This function will make a CPI call to the JLP program to get the AUM of the pool
/// Required extra accounts:
/// - Perpetuals program (PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu)
/// - Perpetuals account (H4ND9aYttUVLFmNypZqLjZ52FYiGvdEB45GmwNoKEjTj)
/// - Mint of the JLP token
/// - All custodies of the pool
/// - All oracles of the pool (from the custodies)
pub fn get_price_with_cpi<'a, 'b>(
    jup_pool_acc: &AccountInfo<'a>,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // 1. Get accounts
    let jup_pool_pk = jup_pool_acc.key;
    let jup_pool: perpetuals::Pool = account_deserialize(jup_pool_acc)?;

    let perpetuals_program = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let perpetuals_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let mint_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    // Get custodies and oracles later. They will be checked by the CPI call
    let num_custodies = jup_pool.custodies.len();
    // Custodies and oracles (in that order) are placed as extra accounts
    let num_extra_accounts = num_custodies * 2;

    // Create account infos for the CPI call
    // program, perpetuals, pool, custodies, oracles
    let mut account_infos = Vec::with_capacity(3 + num_extra_accounts);
    account_infos.push(perpetuals_program.clone());
    account_infos.push(perpetuals_acc.clone());
    account_infos.push(jup_pool_acc.clone());
    account_infos.extend(extra_accounts.take(num_extra_accounts).cloned());
    let cpi_extra_accounts = &account_infos[3..];

    // 2. Check accounts
    require_keys_eq!(
        *perpetuals_program.key,
        perpetuals::ID,
        ScopeError::UnexpectedAccount
    );

    require_keys_eq!(
        *perpetuals_acc.key,
        perpetuals::PERPETUAL_ACC,
        ScopeError::UnexpectedAccount
    );

    check_mint_pk(jup_pool_pk, mint_acc.key, jup_pool.lp_token_bump)
        .map_err(|_| ScopeError::UnexpectedAccount)?;

    // 3. Get mint supply

    let lp_token_supply = {
        let mint_borrow = mint_acc.data.borrow();
        let mint = Mint::unpack(&mint_borrow)?;
        // This is a sanity check to make sure the mint is configured as expected
        // This allows to just divide the two values to get the price
        require_eq!(mint.decimals, POOL_VALUE_SCALE_DECIMALS);
        mint.supply
    };

    // 4. Get AUM with CPI
    let lp_value: u128 = {
        let data = perpetuals::instruction::GetAssetsUnderManagement {
            mode: Some(perpetuals::PriceCalcMode::Min),
        }
        .data();
        let mut accounts = perpetuals::accounts::GetAssetsUnderManagement {
            perpetuals: perpetuals_acc.key(),
            pool: *jup_pool_pk,
        }
        .to_account_metas(None);

        accounts.extend(cpi_extra_accounts.iter().map(|info| AccountMeta {
            pubkey: info.key(),
            is_signer: false,
            is_writable: false,
        }));

        let ix = Instruction {
            program_id: perpetuals::ID,
            accounts,
            data,
        };

        invoke(&ix, &account_infos).unwrap();
        let (_, ret_data) = get_return_data().unwrap();
        borsh::BorshDeserialize::try_from_slice(&ret_data)?
    };

    // 5. Compute price
    let price_dec = Decimal::from(lp_value) / lp_token_supply;
    let dated_price = DatedPrice {
        price: price_dec.into(),
        last_updated_slot: clock.slot,
        unix_timestamp: u64::try_from(clock.unix_timestamp).unwrap(),
        ..Default::default()
    };

    Ok(dated_price)
}
