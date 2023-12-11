#![allow(clippy::result_large_err)] //Needed because we can't change Anchor result type
pub mod oracles;
pub mod program_id;
pub mod utils;

mod handlers;

// Local use
use std::{convert::TryInto, num::TryFromIntError};

pub use anchor_lang;
use anchor_lang::prelude::*;
use decimal_wad::{decimal::Decimal, error::DecimalError};
use handlers::*;
pub use num_enum;
use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};
use program_id::PROGRAM_ID;
pub use whirlpool;
#[cfg(feature = "yvaults")]
pub use yvaults;

pub use crate::utils::scope_chain;

declare_id!(PROGRAM_ID);

// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES_U16: u16 = 512;
// Note: Need to be directly integer value to not confuse the IDL generator
pub const MAX_ENTRIES: usize = 512;
pub const VALUE_BYTE_ARRAY_LEN: usize = 32;

#[program]
pub mod scope {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, feed_name: String) -> Result<()> {
        handler_initialize::process(ctx, feed_name)
    }

    //This handler only works for Pyth type tokens
    pub fn refresh_one_price<'info>(
        ctx: Context<'_, '_, '_, 'info, RefreshOne<'info>>,
        token: u64,
    ) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn refresh_price_list<'info>(
        ctx: Context<'_, '_, '_, 'info, RefreshList<'info>>,
        tokens: Vec<u16>,
    ) -> Result<()> {
        handler_refresh_prices::refresh_price_list(ctx, &tokens)
    }

    pub fn update_mapping(
        ctx: Context<UpdateOracleMapping>,
        token: u64,
        price_type: u8,
        twap_enabled: bool,
        twap_source: u16,
        feed_name: String,
    ) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_update_mapping::process(
            ctx,
            token,
            price_type,
            twap_enabled,
            twap_source,
            feed_name,
        )
    }

    pub fn reset_twap(ctx: Context<ResetTwap>, token: u64, feed_name: String) -> Result<()> {
        let token: usize = token
            .try_into()
            .map_err(|_| ScopeError::OutOfRangeIntegralConversion)?;
        handler_reset_twap::process(ctx, token, feed_name)
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

    pub fn set_admin_cached(
        ctx: Context<SetAdminCached>,
        new_admin: Pubkey,
        feed_name: String,
    ) -> Result<()> {
        handler_set_admin_cached::process(ctx, new_admin, feed_name)
    }

    pub fn approve_admin_cached(ctx: Context<ApproveAdminCached>, feed_name: String) -> Result<()> {
        handler_approve_admin_cached::process(ctx, feed_name)
    }
}

#[zero_copy]
#[derive(Debug, Default)]
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum EmaType {
    Ema1h,
}

/// The sample tracker is a 64 bit number where each bit represents a point in time.
/// We only track one point per time slot. The time slot being the ema_period / 64.
/// The bit is set to 1 if there is a sample at that point in time slot.
#[derive(bytemuck::Zeroable, bytemuck::Pod, Debug, Eq, PartialEq, Clone, Copy, Default)]
#[repr(transparent)]
pub struct EmaTracker(pub u64);

#[zero_copy]
#[derive(Debug, Eq, PartialEq)]
pub struct EmaTwap {
    pub last_update_slot: u64, // the slot when the last observation was added
    pub last_update_unix_timestamp: u64,

    pub current_ema_1h: u128,
    pub updates_tracker_1h: EmaTracker,
    pub padding_0: u64,

    pub padding_1: [u128; 39],
}

impl Default for EmaTwap {
    fn default() -> Self {
        Self {
            current_ema_1h: 0,
            last_update_slot: 0,
            last_update_unix_timestamp: 0,
            updates_tracker_1h: EmaTracker::default(),
            padding_0: 0,
            padding_1: [0_u128; 39],
        }
    }
}

impl EmaTwap {
    fn as_dated_price(&self, index: u16) -> DatedPrice {
        DatedPrice {
            price: Decimal::from_scaled_val(self.current_ema_1h).into(),
            last_updated_slot: self.last_update_slot,
            unix_timestamp: self.last_update_unix_timestamp,
            _reserved: [0; 2],
            _reserved2: [0; 3],
            index,
        }
    }
}

// Account to store dated TWAP prices
#[account(zero_copy)]
pub struct OracleTwaps {
    pub oracle_prices: Pubkey,
    pub oracle_mappings: Pubkey,
    pub twaps: [EmaTwap; MAX_ENTRIES],
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
    pub twap_source: [u16; MAX_ENTRIES], // meaningful only if type == TWAP; the index of where we find the TWAP
    pub twap_enabled: [u8; MAX_ENTRIES], // true or false
    pub _reserved1: [u8; MAX_ENTRIES],
    pub _reserved2: [u32; MAX_ENTRIES],
}

impl OracleMappings {
    pub fn is_twap_enabled(&self, token: usize) -> bool {
        self.twap_enabled[token] > 0
    }

    pub fn get_twap_source(&self, token: usize) -> usize {
        usize::from(self.twap_source[token])
    }
}

#[account(zero_copy)]
pub struct TokenMetadatas {
    pub metadatas_array: [TokenMetadata; MAX_ENTRIES],
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq, Default)]
pub struct TokenMetadata {
    pub name: [u8; 32],
    pub max_age_price_seconds: u64,
    pub _reserved: [u64; 16],
}

// Configuration account of the program
#[account(zero_copy)]
pub struct Configuration {
    pub admin: Pubkey,
    pub oracle_mappings: Pubkey,
    pub oracle_prices: Pubkey,
    pub tokens_metadata: Pubkey,
    pub oracle_twaps: Pubkey,
    pub admin_cached: Pubkey,
    _padding: [u64; 1255],
}

#[derive(TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(u64)]
pub enum UpdateTokenMetadataMode {
    Name = 0,
    MaxPriceAgeSeconds = 1,
}

impl UpdateTokenMetadataMode {
    pub fn to_u64(self) -> u64 {
        self.to_u16().into()
    }

    pub fn to_u16(self) -> u16 {
        match self {
            UpdateTokenMetadataMode::Name => 0,
            UpdateTokenMetadataMode::MaxPriceAgeSeconds => 1,
        }
    }
}

#[error_code]
#[derive(PartialEq, Eq, TryFromPrimitive)]
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

    #[msg("Unable to derive PDA address")]
    UnableToDerivePDA,

    #[msg("Invalid timestamp")]
    BadTimestamp,

    #[msg("Invalid slot")]
    BadSlot,

    #[msg("TWAP price account is different than Scope ID")]
    PriceAccountNotExpected,

    #[msg("TWAP source index out of range")]
    TwapSourceIndexOutOfRange,

    #[msg("TWAP sample is too close to the previous one")]
    TwapSampleTooFrequent,

    #[msg("Unexpected JLP configuration")]
    UnexpectedJlpConfiguration,

    #[msg("Not enough price samples in period to compute TWAP")]
    TwapNotEnoughSamplesInPeriod,
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
