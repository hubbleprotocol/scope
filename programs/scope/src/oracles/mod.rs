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

use crate::{DatedPrice, ScopeError};

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
}

impl OracleType {
    /// Get the number of compute unit needed to refresh the price of a token
    pub fn get_update_cu_budget(&self) -> u32 {
        match self {
            OracleType::Pyth => 15000,
            OracleType::SwitchboardV1 => 15000,
            OracleType::SwitchboardV2 => 30000,
            OracleType::CToken => 130000,
            OracleType::SplStake => 20000,
            OracleType::KToken => 90000,
            OracleType::PythEMA => 15000,
            OracleType::DeprecatedPlaceholder => {
                panic!("DeprecatedPlaceholder is not a valid oracle type")
            }
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
    }
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
    }
}
