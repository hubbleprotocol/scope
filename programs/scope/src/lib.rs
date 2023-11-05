#![allow(clippy::result_large_err)] //Needed because we can't change Anchor result type
pub mod oracles;
pub mod program_id;
pub mod utils;

mod handlers;

// Local use
use std::{convert::TryInto, num::TryFromIntError};

pub use anchor_lang;
use anchor_lang::prelude::*;
use decimal_wad::error::DecimalError;
use handlers::*;
use num_derive::FromPrimitive;
pub use num_enum;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use program_id::PROGRAM_ID;
#[cfg(feature = "yvaults")]
pub use yvaults;

pub use crate::utils::scope_chain;

declare_id!(PROGRAM_ID);

// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES_U16: u16 = 512;
// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES: usize = 512;
pub const VALUE_BYTE_ARRAY_LEN: usize = 32;
pub const TWAP_INTERVAL_SECONDS: u64 = 100;
pub const TWAP_NUM_OBS: usize = 30;

#[program]
pub mod scope {

    use handlers::handler_update_token_metadata::UpdateTokensMetadata;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, feed_name: String) -> Result<()> {
        handler_initialize::process(ctx, feed_name)
    }

    pub fn initialize_tokens_metadata(
        ctx: Context<InitializeTokensMetadata>,
        feed_name: String,
    ) -> Result<()> {
        handler_initialize_tokens_metadata::process(ctx, feed_name)
    }

    //This handler only works for Pyth type tokens
    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u64) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_price_list(ctx: Context<RefreshList>, tokens: Vec<u16>) -> Result<()> {
        handler_refresh_prices::refresh_price_list(ctx, &tokens)
    }

    pub fn update_mapping(
        ctx: Context<UpdateOracleMapping>,
        token: u64,
        price_type: u8,
        feed_name: String,
    ) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_update_mapping::process(ctx, token, price_type, feed_name)
    }

    pub fn update_token_metadata(
        ctx: Context<UpdateTokensMetadata>,
        index: u64,
        mode: u64,
        feed_name: String,
        value: Vec<u8>,
    ) -> Result<()> {
        msg!(
            "update_token_metadata index {} mode {} feed_name {}",
            index,
            mode,
            feed_name
        );
        let index: usize = index
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_update_token_metadata::process(ctx, index, mode, value, feed_name)
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

#[zero_copy]
#[derive(Debug, Eq, PartialEq)]
pub struct TwapBuffer {
    pub observations: [Price; TWAP_NUM_OBS],
    pub unix_timestamps: [u64; TWAP_NUM_OBS],
    pub slots: [u64; TWAP_NUM_OBS],

    /// The value of the last filled observation
    /// if unix_timestamps[0] == 0, then the buffer is empty
    pub curr_index: u64,
}

// Account to store dated prices
#[account(zero_copy)]
pub struct OraclePrices {
    pub oracle_mappings: Pubkey,
    pub prices: [DatedPrice; MAX_ENTRIES],
}

// Account to store dated prices
#[account(zero_copy)]
pub struct OracleTwaps {
    pub oracle_prices: Pubkey,
    pub tokens_metadata: Pubkey,
    pub twap_buffers: [TwapBuffer; MAX_ENTRIES],
}

// Accounts holding source of prices
#[account(zero_copy)]
pub struct OracleMappings {
    pub price_info_accounts: [Pubkey; MAX_ENTRIES],
    pub price_types: [u8; MAX_ENTRIES],
    pub twap_enabled: [u8; MAX_ENTRIES],
    pub _reserved2: [u16; MAX_ENTRIES * 3],
}

#[account(zero_copy)]
pub struct TokenMetadatas {
    pub metadatas_array: [TokenMetadata; MAX_ENTRIES],
}

impl TokenMetadatas {
    pub fn should_store_twap_observations(&self, token: usize) -> bool {
        self.metadatas_array[token].store_observations > 0
    }

    pub fn get_twap_source(&self, token: usize) -> usize {
        self.metadatas_array[token].twap_source as usize
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq, Default)]
pub struct TokenMetadata {
    pub name: [u8; 32],
    pub max_age_price_seconds: u64,
    pub twap_source: u16,
    pub store_observations: u8,
    pub _reserved: [u16; 2],
    pub _reserved2: [u64; 15],
}

// Configuration account of the program
#[account(zero_copy)]
pub struct Configuration {
    pub admin: Pubkey,
    pub oracle_mappings: Pubkey,
    pub oracle_prices: Pubkey,
    pub oracle_twaps: Pubkey,
    pub tokens_metadata: Pubkey,
    _padding: [u64; 1259],
}

#[derive(TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(u64)]
pub enum UpdateTokenMetadataMode {
    Name = 0,
    MaxPriceAgeSeconds = 1,
    StoreObservations = 2,
    TwapSource = 3,
}

impl UpdateTokenMetadataMode {
    pub fn to_u64(self) -> u64 {
        match self {
            UpdateTokenMetadataMode::Name => 0,
            UpdateTokenMetadataMode::MaxPriceAgeSeconds => 1,
            UpdateTokenMetadataMode::StoreObservations => 2,
            UpdateTokenMetadataMode::TwapSource => 3,
        }
    }
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

    #[msg("Invalid token metadata update mode")]
    InvalidTokenUpdateMode,

    #[msg("Too few observations for twap")]
    NotEnoughTwapObservations,
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

pub type ScopeResult<T = ()> = std::result::Result<T, ScopeError>;

impl From<DecimalError> for ScopeError {
    fn from(err: DecimalError) -> ScopeError {
        match err {
            DecimalError::MathOverflow => ScopeError::IntegerOverflow,
        }
    }
}
