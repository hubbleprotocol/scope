//! Provides a generic implementation for all oracle prices that only requires
//! one oracle account to perform a price refresh (such as pyth and switchboard)

use std::fmt::{Debug, Display};

use anchor_client::solana_client::rpc_client::RpcClient;
use anyhow::Result;
use scope::utils::OracleType;
use scope::{DatedPrice, Pubkey};

use crate::config::TokenConfig;

use super::{OracleHelper, TokenEntry};
use anchor_client::solana_sdk::clock;

pub struct SingleAccountOracle {
    pub label: String,
    pub oracle_account: Pubkey,
    pub oracle_type: OracleType,
    pub max_age: clock::Slot,
}

impl SingleAccountOracle {
    pub fn new(conf: &TokenConfig, default_max_age: clock::Slot) -> Self {
        Self {
            label: conf.label.clone(),
            oracle_account: conf.oracle_mapping,
            oracle_type: conf.oracle_type,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
        }
    }
}

impl OracleHelper for SingleAccountOracle {
    fn get_type(&self) -> OracleType {
        self.oracle_type
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        0_usize
    }

    fn get_mapping_account(&self) -> &Pubkey {
        &self.oracle_account
    }

    fn get_extra_accounts(&self) -> &[Pubkey] {
        &[]
    }

    fn get_max_age(&self) -> clock::Slot {
        self.max_age
    }

    fn need_refresh(&self, _scope_price: &DatedPrice, _rpc: &RpcClient) -> Result<bool> {
        Ok(false)
    }
}

impl Display for SingleAccountOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl Debug for SingleAccountOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleAccountOracle")
            .field("label", &self.label)
            .field("oracle_account", &self.oracle_account)
            .field("oracle_type", &self.oracle_type)
            .finish()
    }
}

impl TokenEntry for SingleAccountOracle {}
