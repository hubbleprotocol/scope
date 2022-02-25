use anchor_lang::prelude::*;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use std::convert::TryInto;
pub mod handlers;
pub mod utils;

pub use handlers::*;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array(include!(concat!(env!("OUT_DIR"), "/pubkey.rs")));

declare_id!(PROGRAM_ID);

pub const MAX_ENTRIES: usize = 255;

#[program]
mod scope {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, feed_name: String) -> Result<()> {
        handler_initialize::process(ctx, feed_name)
    }

    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u64) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_batch_prices(ctx: Context<RefreshBatch>, first_token: u64) -> Result<()> {
        let first_token: usize = first_token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_batch_prices(ctx, first_token)
    }

    pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: Vec<u8>) -> Result<()> {
        handler_refresh_prices::refresh_price_list(ctx, &tokens)
    }

    pub fn update_mapping(ctx: Context<UpdateOracleMapping>, token: u64) -> Result<()> {
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
    pub _reserved: [u64; 2],
}

// Account to store dated prices
#[account(zero_copy)]
pub struct OraclePrices {
    pub oracle_mappings: Pubkey,
    pub prices: [DatedPrice; MAX_ENTRIES],
}

// Accounts holding source of prices (all pyth for now)
#[account(zero_copy)]
pub struct OracleMappings {
    pub price_info_accounts: [Pubkey; MAX_ENTRIES],
}

#[error_code]
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

    #[msg("The number of tokens is different from the number of received accounts")]
    AccountsAndTokenMismatch,

    #[msg("The token index received is out of range")]
    BadTokenNb,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}
