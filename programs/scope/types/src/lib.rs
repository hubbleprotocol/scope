#![allow(clippy::result_large_err)] //Needed because we can't change Anchor result type

pub mod program_id;

// Reexports to deal with eventual conflicts
// Local use
use std::num::TryFromIntError;

pub use anchor_lang;
use anchor_lang::prelude::*;
use num_derive::FromPrimitive;
pub use num_enum;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use program_id::PROGRAM_ID;

declare_id!(PROGRAM_ID);

// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES_U16: u16 = 512;
// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES: usize = 512;

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
#[derive(Debug, Eq, PartialEq)]
pub struct DatedPrice {
    pub price: Price,
    pub last_updated_slot: u64,
    pub unix_timestamp: u64,
    pub _reserved: [u64; 2],
    pub _reserved2: [u16; 3],
    // Current index of the dated price.
    pub index: u16,
}

impl Default for DatedPrice {
    fn default() -> Self {
        Self {
            price: Default::default(),
            last_updated_slot: Default::default(),
            unix_timestamp: Default::default(),
            _reserved: Default::default(),
            _reserved2: Default::default(),
            index: MAX_ENTRIES_U16,
        }
    }
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
    pub admin: Pubkey,
    pub oracle_mappings: Pubkey,
    pub oracle_prices: Pubkey,
    _padding: [u64; 1267],
}

#[error_code]
#[derive(PartialEq, Eq, FromPrimitive)]
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

    #[msg("Invalid account discriminator")]
    InvalidAccountDiscriminator,

    #[msg("Unable to deserialize account")]
    UnableToDeserializeAccount,

    #[msg("Error while computing price with ScopeChain")]
    BadScopeChainOrPrices,

    #[msg("Refresh price instruction called in a CPI")]
    RefreshInCPI,

    #[msg("Refresh price instruction preceded by unexpected ixs")]
    RefreshWithUnexpectedIxs,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}

impl From<TryFromIntError> for ScopeError {
    fn from(_: TryFromIntError) -> Self {
        ScopeError::OutOfRangeIntegralConversion
    }
}
