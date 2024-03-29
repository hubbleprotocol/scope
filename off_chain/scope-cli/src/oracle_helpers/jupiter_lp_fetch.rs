//! Implementation of helper for Jupiter's LP Tokens

use std::fmt::{Debug, Display};

use anchor_client::solana_sdk::clock;
use anyhow::Result;
use orbit_link::async_client::AsyncClient;
use scope::oracles::jupiter_lp::get_mint_pk;
use scope::{anchor_lang::prelude::Pubkey, oracles::OracleType, DatedPrice};

use super::{OracleHelper, TokenEntry};
use crate::config::TokenConfig;

#[derive(Debug)]
pub struct JupiterLPOracleFetch {
    label: String,
    /// Pubkey to the Pool account account
    mapping: Pubkey,

    /// Mint of the LP token
    /// (PDA derived from the pool account)
    lp_mint: Pubkey,

    /// Configured max age
    max_age: clock::Slot,

    twap_enabled: bool,
}

impl JupiterLPOracleFetch {
    pub fn new(conf: &TokenConfig, default_max_age: clock::Slot) -> Result<Self> {
        let mapping = conf.oracle_mapping;
        let (lp_mint, _) = get_mint_pk(&mapping);

        Ok(Self {
            label: conf.label.clone(),
            mapping,
            lp_mint,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
            twap_enabled: conf.twap_enabled,
        })
    }
}

#[async_trait::async_trait]
impl OracleHelper for JupiterLPOracleFetch {
    fn get_type(&self) -> OracleType {
        OracleType::JupiterLpFetch
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        1
    }

    fn get_mapping_account(&self) -> Option<Pubkey> {
        Some(self.mapping)
    }

    async fn get_extra_accounts(&self, _rpc: Option<&dyn AsyncClient>) -> Result<Vec<Pubkey>> {
        Ok(vec![self.lp_mint])
    }

    fn get_max_age(&self) -> clock::Slot {
        self.max_age
    }

    fn get_label(&self) -> &str {
        &self.label
    }

    async fn need_refresh(
        &self,
        _scope_price: &DatedPrice,
        _rpc: &dyn AsyncClient,
    ) -> Result<bool> {
        Ok(false)
    }

    fn is_twap_enabled(&self) -> bool {
        self.twap_enabled
    }
}

impl Display for JupiterLPOracleFetch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for JupiterLPOracleFetch {}
