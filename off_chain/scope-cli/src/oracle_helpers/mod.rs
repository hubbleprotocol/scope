//! This mod provides an abstraction above the different implementations needed
//! to manage the refresh of a price on the bot side.
//!
//! Each supported oracle shall have a struct type that implement the trait [`TokenEntry`]:
//!
//! - [`OracleHelper`] to provide all required data to perform trigger the
//!   refresh ix.
//! - [`std::fmt::Display`] for basic logging of a reference to a token.
//! - [`std::fmt::Debug`] for detailled debug and error logs.

use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::solana_sdk::clock;
use anyhow::Result;
use scope::oracles::OracleType;
use scope::{anchor_lang::prelude::Pubkey, DatedPrice};

pub mod ktokens;
pub mod single_account_oracle;
pub mod yi_token;

pub use ktokens::KTokenOracle;
pub use single_account_oracle::SingleAccountOracle;
pub use yi_token::YiOracle;

use crate::config::TokenConfig;

/// Traits combination that should be implemented for all token entries in the bot
pub trait TokenEntry: OracleHelper + std::fmt::Debug + std::fmt::Display {}

/// Trait that must be implemented by objects representing a token in scope
pub trait OracleHelper {
    /// Get the oracle type of the token
    fn get_type(&self) -> OracleType;

    /// Get the number of extra accounts needed to refresh the price of a token
    fn get_number_of_extra_accounts(&self) -> usize;

    /// Get the reference mapping account (placed in oracle mapping and config file)
    ///
    /// The referenced account should contain any information needed to refresh
    /// the price or at least reference the extra account needed to do so (indirect
    /// mapping).
    fn get_mapping_account(&self) -> &Pubkey;

    /// Get the extra accounts needed for the refresh price ix
    fn get_extra_accounts(&self, rpc: Option<&RpcClient>) -> Result<Vec<Pubkey>>;

    /// Get max age after which a refresh must be forced.
    ///
    /// The price will be refreshed after this age even if
    /// [`OracleHelper::need_refresh`] return false to avoid price being
    /// considered stalled. `max_age` here should provide enough margin to
    /// have the maximum of chances of a successful refresh before the price
    /// being considered stalled by the user of the scope feed.
    fn get_max_age(&self) -> clock::Slot;

    /// Tell if a price has changed and need to be refreshed.
    ///
    /// **Note:** For prices that constantly changes implementation
    /// should always return false so refresh only happen on max_age.
    fn need_refresh(&self, scope_price: &DatedPrice, rpc: &RpcClient) -> Result<bool>;
}

pub fn entry_from_config(
    token_conf: &TokenConfig,
    default_max_age: clock::Slot,
    rpc: &RpcClient,
) -> Result<Box<dyn TokenEntry>> {
    Ok(match token_conf.oracle_type {
        OracleType::Pyth
        | OracleType::SwitchboardV1
        | OracleType::SwitchboardV2
        | OracleType::CToken
        | OracleType::SplStake => Box::new(SingleAccountOracle::new(token_conf, default_max_age)),
        OracleType::YiToken => Box::new(YiOracle::new(token_conf, default_max_age, rpc)?),
        OracleType::KToken => Box::new(KTokenOracle::new(token_conf, default_max_age, rpc)?),
    })
}
