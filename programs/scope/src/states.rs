use crate::{MAX_ENTRIES, MAX_ENTRIES_U16};
use anchor_lang::prelude::*;
use decimal_wad::decimal::Decimal;
use num_enum::{IntoPrimitive, TryFromPrimitive};

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

#[zero_copy]
#[derive(Debug, Eq, PartialEq)]
pub struct EmaTwap {
    pub last_update_slot: u64, // the slot when the last observation was added
    pub last_update_unix_timestamp: u64,

    pub current_ema_1h: u128,
    /// The sample tracker is a 64 bit number where each bit represents a point in time.
    pub updates_tracker_1h: u64,
    pub padding_0: u64,

    pub padding_1: [u128; 39],
}

impl Default for EmaTwap {
    fn default() -> Self {
        Self {
            current_ema_1h: 0,
            last_update_slot: 0,
            last_update_unix_timestamp: 0,
            updates_tracker_1h: 0,
            padding_0: 0,
            padding_1: [0_u128; 39],
        }
    }
}

impl EmaTwap {
    pub fn as_dated_price(&self, index: u16) -> DatedPrice {
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
