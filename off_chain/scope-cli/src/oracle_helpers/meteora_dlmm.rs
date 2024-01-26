//! Implementation of helper for Jupiter's LP Tokens

use std::fmt::{Debug, Display};

use anchor_client::anchor_lang::__private::bytemuck;
use anchor_client::solana_sdk::clock;
use anyhow::{Context, Result};
use orbit_link::async_client::AsyncClient;
use scope::oracles::meteora_dlmm::lb_clmm::LbPair;
use scope::{anchor_lang::prelude::Pubkey, oracles::OracleType, DatedPrice};

use super::{OracleHelper, TokenEntry};
use crate::config::TokenConfig;

#[derive(Debug)]
pub struct MeteoraDlmmOracle {
    oracle_type: OracleType,
    label: String,
    /// Pubkey to the Pool account account
    mapping: Pubkey,

    /// Token A mint
    token_a_mint: Pubkey,
    /// Token B mint
    token_b_mint: Pubkey,

    /// Configured max age
    max_age: clock::Slot,

    twap_enabled: bool,
}

impl MeteoraDlmmOracle {
    pub async fn new(
        conf: &TokenConfig,
        default_max_age: clock::Slot,
        rpc: &dyn AsyncClient,
    ) -> Result<Self> {
        if !matches!(
            conf.oracle_type,
            OracleType::MeteoraDlmmAtoB | OracleType::MeteoraDlmmBtoA
        ) {
            anyhow::bail!("Wrong oracle type for MeteoraDlmmOracle");
        }

        let mapping = conf.oracle_mapping;
        let lb_pair_raw = rpc
            .get_account(&mapping)
            .await
            .context("Retrieving Meteora's lb pair account")?;
        let lb_pair: &LbPair = bytemuck::from_bytes(&lb_pair_raw.data[8..]);
        let token_a_mint = lb_pair.token_x_mint;
        let token_b_mint = lb_pair.token_y_mint;
        Ok(Self {
            oracle_type: conf.oracle_type,
            label: conf.label.clone(),
            mapping,
            token_a_mint,
            token_b_mint,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
            twap_enabled: conf.twap_enabled,
        })
    }
}

#[async_trait::async_trait]
impl OracleHelper for MeteoraDlmmOracle {
    fn get_type(&self) -> OracleType {
        self.oracle_type
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        2
    }

    fn get_mapping_account(&self) -> Option<Pubkey> {
        Some(self.mapping)
    }

    async fn get_extra_accounts(&self, _rpc: Option<&dyn AsyncClient>) -> Result<Vec<Pubkey>> {
        Ok(vec![self.token_a_mint, self.token_b_mint])
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

impl Display for MeteoraDlmmOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for MeteoraDlmmOracle {}
