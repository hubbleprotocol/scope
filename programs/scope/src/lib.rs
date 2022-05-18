pub mod handlers;
pub mod program_id;
pub mod utils;

// Reexports to deal with eventual conflicts
pub use anchor_lang;
pub use num_enum;

// Local use
use std::convert::TryInto;

use anchor_lang::prelude::*;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};

use handlers::*;
use program_id::PROGRAM_ID;

declare_id!(PROGRAM_ID);

pub const MAX_ENTRIES: usize = 512;

#[program]
pub mod scope {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, feed_name: String) -> ProgramResult {
        handler_initialize::process(ctx, feed_name)
    }

    //This handler only works for Pyth type tokens
    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u64) -> ProgramResult {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: Vec<u16>) -> ProgramResult {
        handler_refresh_prices::refresh_price_list(ctx, &tokens)
    }

    pub fn update_mapping(
        ctx: Context<UpdateOracleMapping>,
        token: u64,
        price_type: u8,
    ) -> ProgramResult {
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

// Accounts holding source of prices
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

    #[msg("The number of tokens is different from the number of received accounts")]
    AccountsAndTokenMismatch,

    #[msg("The token index received is out of range")]
    BadTokenNb,

    #[msg("The token type received is invalid")]
    BadTokenType,

    #[msg("There was an error with the Switchboard V2 retrieval")]
    SwitchboardV2Error,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}
