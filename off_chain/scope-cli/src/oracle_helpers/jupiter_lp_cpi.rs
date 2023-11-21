//! Implementation of helper for Jupiter's LP Tokens

use std::fmt::{Debug, Display};

use anchor_client::solana_sdk::clock;
use anchor_client::solana_sdk::signer::Signer;
use anyhow::{anyhow, Result};
use orbit_link::async_client::AsyncClient;
use orbit_link::OrbitLink;
use scope::oracles::jupiter_lp::get_mint_pk;
use scope::oracles::jupiter_lp::perpetuals;
use scope::{anchor_lang::prelude::Pubkey, oracles::OracleType, DatedPrice};

use super::{OracleHelper, TokenEntry};
use crate::config::TokenConfig;

#[derive(Debug)]
pub struct JupiterLPOracleCpi {
    label: String,
    /// Pubkey to the Pool account account
    mapping: Pubkey,

    /// Extra accounts:
    /// - Perpetuals program (PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu)
    /// - Perpetuals account (H4ND9aYttUVLFmNypZqLjZ52FYiGvdEB45GmwNoKEjTj)
    /// - Mint of the JLP token
    /// - All custodies of the pool
    /// - All oracles of the pool (from the custodies)
    extra_accounts: Vec<Pubkey>,

    /// Configured max age
    max_age: clock::Slot,

    twap_enabled: bool,
}

impl JupiterLPOracleCpi {
    pub async fn new<T: AsyncClient, S: Signer>(
        conf: &TokenConfig,
        default_max_age: clock::Slot,
        rpc: &OrbitLink<T, S>,
    ) -> Result<Self> {
        let mapping = conf.oracle_mapping;
        let perp_program = perpetuals::id();
        let perp_acc = perpetuals::PERPETUAL_ACC;
        let (lp_mint, _) = get_mint_pk(&mapping);

        let jup_pool: perpetuals::Pool = rpc.get_anchor_account(&conf.oracle_mapping).await?;

        let mut extra_accounts = Vec::with_capacity(3 + jup_pool.custodies.len() * 2);
        let custodies: Vec<perpetuals::Custody> = rpc
            .get_anchor_accounts::<perpetuals::Custody>(&jup_pool.custodies)
            .await?
            .into_iter()
            .map(|c| c.ok_or(anyhow!("Error while fetching custodies")))
            .collect::<Result<Vec<_>>>()?;

        extra_accounts.push(perp_program);
        extra_accounts.push(perp_acc);
        extra_accounts.push(lp_mint);
        extra_accounts.extend(jup_pool.custodies.iter().cloned());
        extra_accounts.extend(custodies.iter().map(|c| c.oracle.oracle_account));

        Ok(Self {
            label: conf.label.clone(),
            mapping,
            extra_accounts,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
            twap_enabled: conf.twap_enabled,
        })
    }
}

#[async_trait::async_trait]
impl OracleHelper for JupiterLPOracleCpi {
    fn get_type(&self) -> OracleType {
        OracleType::JupiterLP
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        self.extra_accounts.len()
    }

    fn get_mapping_account(&self) -> Option<Pubkey> {
        Some(self.mapping)
    }

    async fn get_extra_accounts(&self, _rpc: Option<&dyn AsyncClient>) -> Result<Vec<Pubkey>> {
        Ok(self.extra_accounts.clone())
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

impl Display for JupiterLPOracleCpi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for JupiterLPOracleCpi {}
