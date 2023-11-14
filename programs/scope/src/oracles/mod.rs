pub mod ctokens;
#[cfg(feature = "yvaults")]
pub mod ktokens;
#[cfg(feature = "yvaults")]
pub mod ktokens_token_x;

pub mod jupiter_lp;
pub mod msol_stake;
pub mod orca_whirlpool;
pub mod pyth;
pub mod pyth_ema;
pub mod raydium_ammv3;
pub mod spl_stake;
pub mod switchboard_v1;
pub mod switchboard_v2;
pub mod twap;

use anchor_lang::prelude::{err, AccountInfo, Clock, Context, Result};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

use crate::{DatedPrice, OracleMappings, OracleTwaps, ScopeError};

use self::ktokens_token_x::TokenTypes;

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
    /// MSOL Stake Pool token
    MsolStake = 8,
    /// Number of lamports of token A for 1 lamport of kToken
    KTokenToTokenA = 9,
    /// Number of lamports of token B for 1 lamport of kToken
    KTokenToTokenB = 10,
    /// Jupiter's perpetual LP tokens
    JupiterLP = 11,
    /// Scope twap
    ScopeTwap = 12,
    /// Orca's whirlpool price (CLMM)
    OrcaWhirlpool = 13,
    /// Raydium's AMM v3 price (CLMM)
    RaydiumAmmV3 = 14,
}

impl OracleType {
    pub fn is_twap(&self) -> bool {
        matches!(self, OracleType::ScopeTwap)
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
            OracleType::KTokenToTokenA => 100000,
            OracleType::KTokenToTokenB => 100000,
            OracleType::MsolStake => 20000,
            OracleType::JupiterLP => 40000,
            OracleType::ScopeTwap => 10000,
            OracleType::OrcaWhirlpool => 10000,
            OracleType::RaydiumAmmV3 => 10000,
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
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
    clock: &Clock,
    oracle_twaps: &OracleTwaps,
    oracle_mappings: &OracleMappings,
    index: usize,
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
        OracleType::PythEMA => pyth_ema::get_price(base_account),
        #[cfg(feature = "yvaults")]
        OracleType::KToken => ktokens::get_price(base_account, clock, extra_accounts),
        #[cfg(feature = "yvaults")]
        OracleType::KTokenToTokenA => ktokens_token_x::get_token_x_per_share(
            base_account,
            clock,
            extra_accounts,
            TokenTypes::TokenA,
        ),
        #[cfg(feature = "yvaults")]
        OracleType::KTokenToTokenB => ktokens_token_x::get_token_x_per_share(
            base_account,
            clock,
            extra_accounts,
            TokenTypes::TokenB,
        ),
        #[cfg(not(feature = "yvaults"))]
        OracleType::KTokenToTokenA => {
            panic!("yvaults feature is not enabled, KToken oracle type is not available")
        }
        #[cfg(not(feature = "yvaults"))]
        OracleType::KTokenToTokenB => {
            panic!("yvaults feature is not enabled, KToken oracle type is not available")
        }
        OracleType::MsolStake => msol_stake::get_price(base_account, clock),
        OracleType::JupiterLP => jupiter_lp::get_price(base_account, clock, extra_accounts),
        OracleType::ScopeTwap => twap::get_price(oracle_mappings, oracle_twaps, index),
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
        OracleType::SplStake => Ok(()),      // TODO, should validate ownership of the account
        OracleType::KToken => Ok(()), // TODO, should validate ownership of the ktoken account
        OracleType::KTokenToTokenA => Ok(()), // TODO, should validate ownership of the ktoken account
        OracleType::KTokenToTokenB => Ok(()), // TODO, should validate ownership of the ktoken account
        OracleType::PythEMA => pyth::validate_pyth_price_info(price_account),
        OracleType::MsolStake => Ok(()),
        OracleType::JupiterLP => jupiter_lp::validate_jlp_pool(price_account),
        OracleType::ScopeTwap => twap::validate_price_account(price_account),
        OracleType::DeprecatedPlaceholder => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
    }
}
