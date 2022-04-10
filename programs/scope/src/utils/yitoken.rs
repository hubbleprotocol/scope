use crate::utils::OracleType;
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

pub fn get_price(
    price_type: OracleType,
    yi_underlying_tokens: &Account<TokenAccount>,
    yi_mint: &Account<Mint>,
    clock_slot: clock::Slot,
) -> Result<DatedPrice> {
    match price_type {
        OracleType::Pyth => return Err(ScopeError::BadTokenType.into()),
        OracleType::Switchboard => todo!(),
        OracleType::YiToken => (),
    }
    let yi_underlying_tokens_amount = yi_underlying_tokens.amount;
    let yi_mint_supply = yi_mint.supply;
    let price = price_compute(yi_underlying_tokens_amount, yi_mint_supply)
        .ok_or(ScopeError::MathOverflow)?;
    let dated_price = DatedPrice {
        price,
        last_updated_slot: clock_slot,
        ..Default::default()
    };
    Ok(dated_price)
}
