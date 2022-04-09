use crate::utils::PriceType;
use crate::{DatedPrice, Price, Result, ScopeError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;
use anchor_spl::token::{Mint, TokenAccount};

pub fn get_price(
    price_type: PriceType,
    yi_underlying_tokens: &Account<TokenAccount>,
    yi_mint: &Account<Mint>,
    clock_slot: clock::Slot,
) -> Result<DatedPrice> {
    match price_type {
        PriceType::Pyth => return Err(ScopeError::BadTokenType.into()),
        PriceType::Switchboard => todo!(),
        PriceType::YiToken => (),
    }
    let yi_underlying_tokens_amount = yi_underlying_tokens.amount;
    let yi_mint_supply = yi_mint.supply;
    let price_amount: u64 = 100_000_000u128
        .checked_mul(yi_underlying_tokens_amount.into())
        .ok_or(ScopeError::MathOverflow)?
        .checked_div(yi_mint_supply.into())
        .ok_or(ScopeError::MathOverflow)?
        .try_into()
        .map_err(|_| ScopeError::MathOverflow)?;
    let dated_price = DatedPrice {
        price: Price {
            value: price_amount,
            exp: 8,
        },
        last_updated_slot: clock_slot,
        ..Default::default()
    };
    Ok(dated_price)
}
