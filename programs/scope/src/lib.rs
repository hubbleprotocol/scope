pub use anchor_lang::prelude::*;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use std::convert::TryInto;
pub mod handlers;
pub mod utils;

pub use handlers::*;
pub use utils::*;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array(include!(concat!(env!("OUT_DIR"), "/pubkey.rs")));

declare_id!(PROGRAM_ID);

pub const MAX_ENTRIES: usize = 512;

#[program]
mod scope {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, feed_name: String) -> Result<()> {
        handler_initialize::process(ctx, feed_name)
    }

    //This handler only works for Pyth type tokens
    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u64) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_yi_token(ctx: Context<RefreshYiToken>, token: u64) -> Result<()> {
        let token: usize = token.try_into().map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_yitoken_prices::refresh_yi_token(ctx, token)
    }

    pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: Vec<u16>) -> Result<()> {
        handler_refresh_prices::refresh_price_list(ctx, &tokens)
    }

    pub fn update_mapping(ctx: Context<UpdateOracleMapping>, token: u64, price_type: u8) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_update_mapping::process(ctx, token, price_type)
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
    pub _reserved: [u64; 4],
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
    pub price_types: [u8; MAX_ENTRIES],
    pub _reserved2: [u64; MAX_ENTRIES],
}

// Configuration account of the program
#[account(zero_copy)]
pub struct Configuration {
    pub admin_pbk: Pubkey,
    pub oracle_mappings_pbk: Pubkey,
    pub oracle_prices_pbk: Pubkey,
    _padding: [u64; 1267],
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

    #[msg("The token type received is invalid")]
    BadTokenType,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}
