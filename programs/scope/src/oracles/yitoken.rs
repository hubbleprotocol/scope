use crate::{DatedPrice, Price, Result, ScopeError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;
use anchor_spl::token::{Mint, TokenAccount};

const YI_DECIMAL_NUMBER: u32 = 8;
const YI_COMPUTE_INIT: u128 = 10u128.pow(YI_DECIMAL_NUMBER);

// YiToken root account
#[account(zero_copy)]
#[derive(Debug, Default)]
pub struct YiToken {
    pub mint: Pubkey,
    pub bump: u8,
    pub _padding: [u8; 7],

    // The [`anchor_spl::token::Mint`] backing the [`YiToken`].
    pub token_mint: Pubkey,
    // [`anchor_spl::token::TokenAccount`] containing the staked tokens.
    pub token_account: Pubkey,

    // fees in millibps
    pub stake_fee: u32,
    pub unstake_fee: u32,
}

/// Compute the current price
///
/// Return `None` in case of overflow
pub fn price_compute(tokens_amount: u64, mint_supply: u64) -> Option<Price> {
    let value: u64 = YI_COMPUTE_INIT
        .checked_mul(tokens_amount.into())?
        .checked_div(mint_supply.into())?
        .try_into()
        .ok()?;
    Some(Price {
        value,
        exp: YI_DECIMAL_NUMBER.into(),
    })
}

pub fn get_price<'a, 'b>(
    yi_account: &AccountInfo,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    // Get the root account
    let yi_account_raw = yi_account.data.borrow();
    let yi_account = YiToken::try_deserialize(&mut &yi_account_raw[..])?;

    // extract the accounts from extra iterator
    let yi_mint_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    let yi_token_info = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    // Check that they are the expected accounts
    if yi_account.mint != yi_mint_info.key() || yi_account.token_account != yi_token_info.key() {
        return err!(ScopeError::UnexpectedAccount);
    }

    // Parse them
    let yi_mint = Account::<Mint>::try_from(yi_mint_info)?;
    let yi_underlying_tokens = Account::<TokenAccount>::try_from(yi_token_info)?;

    // Compute price
    let yi_underlying_tokens_amount = yi_underlying_tokens.amount;
    let yi_mint_supply = yi_mint.supply;
    let price = price_compute(yi_underlying_tokens_amount, yi_mint_supply)
        .ok_or(ScopeError::MathOverflow)?;
    let dated_price = DatedPrice {
        price,
        last_updated_slot: clock::Clock::get()?.slot,
        ..Default::default()
    };
    Ok(dated_price)
}
