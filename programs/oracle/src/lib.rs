use anchor_lang::prelude::*;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use std::convert::TryInto;
pub mod handlers;
pub mod utils;

pub use handlers::*;

declare_id!("GyQfv4aBAhZevnHdZ2rkJyZkhfdgGLboGoW7U7dKUosb");

#[program]
mod oracle {

    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> ProgramResult {
        msg!("ix=initialize");
        id();
        Ok(())
    }

    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u64) -> ProgramResult {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_batch_prices(ctx: Context<RefreshBatch>, first_token: u64) -> ProgramResult {
        let first_token: usize = first_token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_batch_prices(ctx, first_token)
    }

    pub fn update_mapping(ctx: Context<UpdateOracleMapping>, token: u64) -> ProgramResult {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_update_mapping::process(ctx, token)
    }
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, Default)]
pub struct Price {
    // Pyth price, integer + exponent representation
    // decimal price would be
    // as integer: 6462236900000, exponent: 8
    // as float:   64622.36900000

    // value is the scaled integer
    // for example, 6462236900000 for btc
    pub value: u64,

    // exponent represents the number of decimals
    // for example, 8 for btc
    pub exp: u64,
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, Default)]
pub struct DatedPrice {
    pub price: Price,
    pub last_updated_slot: u64,
}

// Account to store dated prices
#[account(zero_copy)]
pub struct OraclePrices {
    pub prices: [DatedPrice; 256],
}

// Accounts holding source of prices (all pyth for now)
#[account(zero_copy)]
pub struct OracleMappings {
    pub price_info_accounts: [Pubkey; 256],
}

#[error]
#[derive(PartialEq, Eq)]
pub enum ScopeError {
    #[msg("Integer overflow")]
    IntegerOverflow,

    #[msg("Conversion failure")]
    ConversionFailure,

    #[msg("Mathematical operation with overflow")]
    MathOverflow,

    #[msg("Out of range integral conversion attempted")]
    OutOfRangeIntegralConversion,

    #[msg("Unexpected account in instruction")]
    UnexpectedAccount,

    #[msg("Price is not valid")]
    PriceNotValid,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}
