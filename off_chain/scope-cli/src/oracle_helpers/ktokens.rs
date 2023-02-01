//! Implementation of helper for Kamino's kTokens

use std::fmt::{Debug, Display};

use anchor_client::{anchor_lang::__private::bytemuck, solana_sdk::clock};
use anyhow::{Context, Result};
use orbit_link::async_client::AsyncClient;
use scope::{
    anchor_lang::prelude::Pubkey,
    oracles::{ktokens::WhirlpoolStrategy, OracleType},
    DatedPrice,
};

use super::{OracleHelper, TokenEntry};
use crate::config::TokenConfig;

const NB_EXTRA_ACCOUNT: usize = 4;

#[derive(Debug)]
pub struct KTokenOracle {
    label: String,
    /// Pubkey to Kamino's strategy account of type [`WhirlpoolStrategy`]
    mapping: Pubkey,

    /// Extra accounts are:
    /// 0. The [`whirlpool::state::Whirlpool`] used by the strategy.
    /// 1. The [`whirlpool::state::Position`] position taken by Kamino.
    /// 2. The [`scope::OraclePrices`] storing the prices of the underlying tokens.
    /// 3. The [`scope::utils::scope_chain::ScopeChainAccount`] allowing to find the right prices.
    extra_accounts: [Pubkey; NB_EXTRA_ACCOUNT],

    /// Configured max age
    max_age: clock::Slot,
}

impl KTokenOracle {
    pub async fn new(
        conf: &TokenConfig,
        default_max_age: clock::Slot,
        rpc: &dyn AsyncClient,
    ) -> Result<Self> {
        let mapping = conf.oracle_mapping;
        let strategy_account_raw = rpc
            .get_account(&mapping)
            .await
            .context("Retrieving Kamino strategy account")?;

        let strategy_account: &WhirlpoolStrategy =
            bytemuck::from_bytes(&strategy_account_raw.data[8..]);

        let whirlpool = strategy_account.whirlpool;
        let position = strategy_account.position;
        let prices = strategy_account.scope_prices;
        let (scope_chain_pk, _) = Pubkey::find_program_address(
            &[
                r"ScopeChain".as_bytes(),
                &strategy_account.scope_prices.to_bytes(),
            ],
            &strategy_account_raw.owner,
        );

        Ok(Self {
            label: conf.label.clone(),
            mapping,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
            extra_accounts: [whirlpool, position, prices, scope_chain_pk],
        })
    }
}

#[async_trait::async_trait]
impl OracleHelper for KTokenOracle {
    fn get_type(&self) -> OracleType {
        OracleType::KToken
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        NB_EXTRA_ACCOUNT
    }

    fn get_mapping_account(&self) -> &Pubkey {
        &self.mapping
    }

    async fn get_extra_accounts(&self, rpc: Option<&dyn AsyncClient>) -> Result<Vec<Pubkey>> {
        let mut res = self.extra_accounts.to_vec();
        if let Some(rpc) = rpc {
            let strategy_account_raw = rpc
                .get_account(&self.mapping)
                .await
                .context("Retrieving Kamino strategy account")?;

            let strategy_account: &WhirlpoolStrategy =
                bytemuck::from_bytes(&strategy_account_raw.data[8..]);
            res[1] = strategy_account.position;
        }
        Ok(res)
    }

    fn get_max_age(&self) -> clock::Slot {
        self.max_age
    }

    async fn need_refresh(
        &self,
        _scope_price: &DatedPrice,
        _rpc: &dyn AsyncClient,
    ) -> Result<bool> {
        Ok(false)
    }
}

impl Display for KTokenOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for KTokenOracle {}
