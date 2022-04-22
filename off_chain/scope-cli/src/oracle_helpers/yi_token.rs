//! Implementation of helper for the Yi token

use std::fmt::{Debug, Display};

use anchor_spl::token::{Mint, TokenAccount};
use anyhow::{anyhow, Context, Result};

use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::solana_sdk::clock;

use scope::utils::yitoken::{price_compute, YiToken};
use scope::utils::OracleType;
use scope::{AccountDeserialize, DatedPrice, Price, Pubkey};
use tracing::trace;

use super::{OracleHelper, TokenEntry};
use crate::config::TokenConfig;

#[derive(Debug)]
pub struct YiOracle {
    label: String,
    /// Yi token reference account of type [`YiToken`]
    mapping: Pubkey,

    /// Extra accounts are:
    /// 0. The [`anchor_spl::token::Mint`] backing the [`YiToken`].
    /// 1. [`anchor_spl::token::TokenAccount`] containing the staked tokens.
    extra_accounts: [Pubkey; 2],

    /// Configured max age
    max_age: clock::Slot,
}

impl YiOracle {
    pub fn new(conf: &TokenConfig, default_max_age: clock::Slot, rpc: &RpcClient) -> Result<Self> {
        let mapping = conf.oracle_mapping;
        let yi_account_raw = rpc
            .get_account_data(&mapping)
            .context("Retrieving yi token mapping account")?;

        //TODO: return error instead of unwrap
        let yi_account = YiToken::try_deserialize(&mut &yi_account_raw[..]).unwrap();

        Ok(Self {
            label: conf.label.clone(),
            mapping,
            max_age: conf.max_age.map(|nz| nz.into()).unwrap_or(default_max_age),
            extra_accounts: [yi_account.mint, yi_account.token_account],
        })
    }

    pub fn get_current_price(&self, rpc: &RpcClient) -> Result<Price> {
        // Retrieve the onchain accounts
        let token_account_raw = rpc
            .get_account_data(&self.extra_accounts[1])
            .context("retrieving yi token account")?;
        let token_account = TokenAccount::try_deserialize(&mut &token_account_raw[..]).unwrap();

        let token_mint_raw = rpc
            .get_account_data(&self.extra_accounts[0])
            .context("retrieving yi mint account")?;
        let token_mint = Mint::try_deserialize(&mut &token_mint_raw[..]).unwrap();

        // Compute the price
        let new_price = price_compute(token_account.amount, token_mint.supply)
            .ok_or_else(|| anyhow!("Overflow while computing yi price"))?;
        Ok(new_price)
    }
}

impl OracleHelper for YiOracle {
    fn get_type(&self) -> OracleType {
        OracleType::YiToken
    }

    fn get_number_of_extra_accounts(&self) -> usize {
        2
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

    fn need_refresh(&self, scope_price: &DatedPrice, rpc: &RpcClient) -> Result<bool> {
        let new_price = self.get_current_price(rpc)?;
        // Need refresh is current token price is different from scope price
        if new_price == scope_price.price {
            trace!("Price for Yi Token has not changed");
            Ok(false)
        } else {
            trace!("Price for Yi Token needs update");
            Ok(true)
        }
    }
}

impl Display for YiOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl TokenEntry for YiOracle {}
