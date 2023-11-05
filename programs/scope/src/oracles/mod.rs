pub mod ctokens;
#[cfg(feature = "yvaults")]
pub mod ktokens;
pub mod pyth;
pub mod pyth_ema;
pub mod spl_stake;
pub mod switchboard_v1;
pub mod switchboard_v2;

use anchor_lang::prelude::{err, AccountInfo, Clock, Context, Result};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

use crate::{DatedPrice, OracleTwaps, ScopeError, TWAP_NUM_OBS};

pub fn check_context<T>(ctx: &Context<T>) -> Result<()> {
    //make sure there are no extra accounts
    if !ctx.remaining_accounts.is_empty() {
        return err!(ScopeError::UnexpectedAccount);
    }

    Ok(())
}

#[derive(
    Serialize, Deserialize, IntoPrimitive, TryFromPrimitive, Clone, Copy, PartialEq, Eq, Debug,
)]
#[repr(u8)]
pub enum OracleType {
    Pyth = 0,
    SwitchboardV1 = 1,
    SwitchboardV2 = 2,
    /// Deprecated (formerly YiToken)
    // Do not remove - breaks the typescript idl codegen
    DeprecatedPlaceholder = 3,
    /// Solend tokens
    CToken = 4,
    /// SPL Stake Pool token (like scnSol)
    SplStake = 5,
    /// KTokens from Kamino
    KToken = 6,
    /// Pyth Exponentially-Weighted Moving Average
    PythEMA = 7,
    /// Scope twap
    ScopeTwap = 8,
}

impl OracleType {
    pub fn is_twap(&self) -> bool {
        match self {
            OracleType::ScopeTwap => true,
            _ => false,
        }
    }

    pub fn min_twap_observations(&self) -> usize {
        match self {
            OracleType::ScopeTwap => 2,
            _ => unimplemented!(),
        }
    }

    pub fn twap_duration_seconds(&self) -> u64 {
        match self {
            OracleType::ScopeTwap => 60 * 60, // 1H
            _ => unimplemented!(),
        }
    }

    /// Get the number of compute unit needed to refresh the price of a token
    pub fn get_update_cu_budget(&self) -> u32 {
        match self {
            OracleType::Pyth => 15000,
            OracleType::SwitchboardV1 => 15000,
            OracleType::SwitchboardV2 => 30000,
            OracleType::CToken => 130000,
            OracleType::SplStake => 20000,
            OracleType::KToken => 120000,
            OracleType::PythEMA => 15000,
            OracleType::DeprecatedPlaceholder => {
                panic!("DeprecatedPlaceholder is not a valid oracle type")
            }
            OracleType::ScopeTwap => 10000,
        }
    }
}

/// Get the price for a given oracle type
///
/// The `base_account` should have been checked against the oracle mapping
/// If needed the `extra_accounts` will be extracted from the provided iterator and checked
/// with the data contained in the `base_account`
pub fn get_price<'a, 'b>(
    price_type: OracleType,
    base_account: &AccountInfo,
    _extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
    clock: &Clock,
) -> crate::Result<DatedPrice>
where
    'a: 'b,
{
    match price_type {
        OracleType::Pyth => pyth::get_price(base_account),
        OracleType::SwitchboardV1 => switchboard_v1::get_price(base_account),
        OracleType::SwitchboardV2 => switchboard_v2::get_price(base_account),
        OracleType::CToken => ctokens::get_price(base_account, clock),
        OracleType::SplStake => spl_stake::get_price(base_account, clock),
        #[cfg(not(feature = "yvaults"))]
        OracleType::KToken => {
            panic!("yvaults feature is not enabled, KToken oracle type is not available")
        }
        #[cfg(feature = "yvaults")]
        OracleType::KToken => ktokens::get_price(base_account, clock, _extra_accounts),
        OracleType::PythEMA => pyth_ema::get_price(base_account),
        OracleType::DeprecatedPlaceholder => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
        OracleType::ScopeTwap => {
            panic!("ScopeTwap is not a valid oracle type")
        }
    }
}

pub fn get_twap_from_observations(
    price_type: OracleType,
    oracle_twaps: &OracleTwaps,
    twap_buffer_source: usize,
    clock: &Clock,
) -> crate::Result<DatedPrice> {
    // Basically iterate through the observations of the [token] from OracleTwaps
    // and calculate twap up to a certain point in time, given how far back this current
    // OracleTwap twap duration is
    // TODO: add constraints about min num observations

    let twap_duration_seconds = price_type.twap_duration_seconds();
    let min_twap_observations = price_type.min_twap_observations();
    let oldest_ts = clock.unix_timestamp as u64 - twap_duration_seconds;

    let twap_buffer = oracle_twaps.twap_buffers[twap_buffer_source];

    let (mut running_index, mut twap, mut num_obs) = (twap_buffer.curr_index as usize, 0, 0);

    loop {
        let obs = twap_buffer.observations[running_index as usize];
        let ts = twap_buffer.unix_timestamps[running_index as usize];

        if ts < oldest_ts || ts == 0 {
            break;
        }

        twap += obs.value * 10u64.pow(obs.exp as u32);
        num_obs += 1;

        running_index = (running_index + TWAP_NUM_OBS - 1) % TWAP_NUM_OBS;
    }

    if min_twap_observations > num_obs {
        return err!(ScopeError::NotEnoughTwapObservations);
    }

    Ok(DatedPrice {
        price: crate::Price { value: 0, exp: 0 },
        last_updated_slot: twap_buffer.slots[twap_buffer.curr_index as usize],
        unix_timestamp: twap_buffer.unix_timestamps[twap_buffer.curr_index as usize],
        _reserved: [0; 2],
        _reserved2: [0; 3],
        index: 0,
    })
}

/// Validate the given account as being an appropriate price account for the
/// given oracle type.
///
/// This function shall be called before update of oracle mappings
pub fn validate_oracle_account(
    price_type: OracleType,
    price_account: &AccountInfo,
) -> crate::Result<()> {
    match price_type {
        OracleType::Pyth => pyth::validate_pyth_price_info(price_account),
        OracleType::SwitchboardV1 => Ok(()), // TODO at least check account ownership?
        OracleType::SwitchboardV2 => Ok(()), // TODO at least check account ownership?
        OracleType::CToken => Ok(()),        // TODO how shall we validate ctoken account?
        OracleType::SplStake => Ok(()),
        OracleType::KToken => Ok(()),
        OracleType::PythEMA => pyth::validate_pyth_price_info(price_account),
        OracleType::DeprecatedPlaceholder => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
        OracleType::ScopeTwap => Ok(()),
    }
}
