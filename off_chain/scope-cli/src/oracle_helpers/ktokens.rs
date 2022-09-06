//! Implementation of helper for the Yi token

use std::fmt::{Debug, Display};

use anchor_client::anchor_lang::__private::bytemuck;
use anyhow::{Context, Result};

use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::solana_sdk::clock;

use scope::anchor_lang::prelude::Pubkey;
use scope::oracles::ktokens::WhirlpoolStrategy;
use scope::oracles::OracleType;
use scope::DatedPrice;

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
    pub fn new(conf: &TokenConfig, default_max_age: clock::Slot, rpc: &RpcClient) -> Result<Self> {
        let mapping = conf.oracle_mapping;
        let strategy_account_raw = rpc
            .get_account(&mapping)
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

    fn get_extra_accounts(&self) -> &[Pubkey] {
        &self.extra_accounts
    }

    fn get_max_age(&self) -> clock::Slot {
        self.max_age
    }

    fn need_refresh(&self, _scope_price: &DatedPrice, _rpc: &RpcClient) -> Result<bool> {
        Ok(false)
    }
}

impl Display for KTokenOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for KTokenOracle {}
