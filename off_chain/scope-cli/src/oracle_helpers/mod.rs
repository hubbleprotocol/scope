//! This mod provides an abstraction above the different implementations needed
//! to manage the refresh of a price on the bot side.
//!
//! Each supported oracle shall have a struct type that implement the trait [`TokenEntry`]:
//!
//! - [`OracleHelper`] to provide all required data to perform trigger the
//!   refresh ix.
//! - [`std::fmt::Display`] for basic logging of a reference to a token.
//! - [`std::fmt::Debug`] for detailed debug and error logs.

use anchor_client::solana_sdk::clock;
use anchor_client::solana_sdk::signer::Signer;
use anyhow::Result;
use orbit_link::async_client::{self, AsyncClient};
use orbit_link::OrbitLink;
use scope::{anchor_lang::prelude::Pubkey, oracles::OracleType, DatedPrice};

pub mod jupiter_lp_compute;
pub mod jupiter_lp_fetch;
#[cfg(feature = "yvaults")]
pub mod ktokens;
pub mod meteora_dlmm;
pub mod orca_whirlpool;
pub mod single_account_oracle;
pub mod twap;

pub use single_account_oracle::SingleAccountOracle;

use crate::config::TokenConfig;

/// Traits combination that should be implemented for all token entries in the bot
pub trait TokenEntry: OracleHelper + std::fmt::Debug + std::fmt::Display {}

/// Trait that must be implemented by objects representing a token in scope
#[async_trait::async_trait]
pub trait OracleHelper: Sync {
    /// Get the oracle type of the token
    fn get_type(&self) -> OracleType;

    /// Get the number of extra accounts needed to refresh the price of a token
    fn get_number_of_extra_accounts(&self) -> usize;

    /// Get the reference mapping account (placed in oracle mapping and config file)
    ///
    /// The referenced account should contain any information needed to refresh
    /// the price or at least reference the extra account needed to do so (indirect
    /// mapping).
    fn get_mapping_account(&self) -> Option<Pubkey>;

    /// Get the extra accounts needed for the refresh price ix
    async fn get_extra_accounts(&self, rpc: Option<&dyn AsyncClient>) -> Result<Vec<Pubkey>>;

    /// Get max age after which a refresh must be forced.
    ///
    /// The price will be refreshed after this age even if
    /// [`OracleHelper::need_refresh`] return false to avoid price being
    /// considered stalled. `max_age` here should provide enough margin to
    /// have the maximum of chances of a successful refresh before the price
    /// being considered stalled by the user of the scope feed.
    fn get_max_age(&self) -> clock::Slot;

    fn get_label(&self) -> &str;

    /// Tell if a price has changed and need to be refreshed.
    ///
    /// **Note:** For prices that constantly changes implementation
    /// should always return false so refresh only happen on max_age.
    async fn need_refresh(&self, scope_price: &DatedPrice, rpc: &dyn AsyncClient) -> Result<bool>;

    /// Give the number of compute units needed to refresh the price of the token
    fn get_update_cu_budget(&self) -> u32 {
        self.get_type().get_update_cu_budget()
    }

    /// If the entry is a twap, give the id of the token used as source for this twap
    /// Else return None
    fn get_twap_source(&self) -> Option<u16> {
        None
    }

    /// Tell if this token should have a twap computed
    fn is_twap_enabled(&self) -> bool;
}

pub async fn entry_from_config<T, S>(
    token_conf: &TokenConfig,
    default_max_age: clock::Slot,
    rpc: &OrbitLink<T, S>,
) -> Result<Box<dyn TokenEntry>>
where
    T: async_client::AsyncClient,
    S: Signer,
{
    Ok(match token_conf.oracle_type {
        OracleType::Pyth
        | OracleType::SwitchboardV2
        | OracleType::CToken
        | OracleType::SplStake
        | OracleType::MsolStake
        | OracleType::PythEMA
        | OracleType::RaydiumAmmV3AtoB
        | OracleType::RaydiumAmmV3BtoA => {
            Box::new(SingleAccountOracle::new(token_conf, default_max_age))
        }
        #[cfg(feature = "yvaults")]
        OracleType::KToken | OracleType::KTokenToTokenA | OracleType::KTokenToTokenB => {
            Box::new(ktokens::KTokenOracle::new(token_conf, default_max_age, &rpc.client).await?)
        }
        OracleType::ScopeTwap => Box::new(twap::TwapOracle::new(token_conf, default_max_age)),
        #[cfg(not(feature = "yvaults"))]
        OracleType::KToken | OracleType::KTokenToTokenA | OracleType::KTokenToTokenB => {
            panic!("yvaults feature is not enabled, KTokenOracle is not available")
        }
        OracleType::JupiterLpFetch => Box::new(jupiter_lp_fetch::JupiterLPOracleFetch::new(
            token_conf,
            default_max_age,
        )?),
        OracleType::OrcaWhirlpoolAtoB | OracleType::OrcaWhirlpoolBtoA => Box::new(
            orca_whirlpool::OrcaWhirlpoolOracle::new(token_conf, default_max_age, &rpc.client)
                .await?,
        ),
        OracleType::JupiterLpCompute => Box::new(
            jupiter_lp_compute::JupiterLPOracleCompute::new(token_conf, default_max_age, rpc)
                .await?,
        ),
        OracleType::MeteoraDlmmAtoB | OracleType::MeteoraDlmmBtoA => Box::new(
            meteora_dlmm::MeteoraDlmmOracle::new(token_conf, default_max_age, &rpc.client).await?,
        ),
        OracleType::DeprecatedPlaceholder1 | OracleType::DeprecatedPlaceholder2 => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
    })
}
