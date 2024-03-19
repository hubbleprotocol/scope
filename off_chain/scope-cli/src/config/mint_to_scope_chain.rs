use scope::{anchor_lang::prelude::Pubkey, MintsToScopeChains};
use serde::{Deserialize, Serialize};

use scope::MintToScopeChain;

use crate::scope_client::TokenEntryList;
use anyhow::{anyhow, Result};

#[derive(Debug, Serialize, Deserialize)]
pub struct MintToScopeChainConfig {
    pub label: String,
    pub user_entry_id: u16,
    pub mapping: Vec<MintToScopeChain>,
}

impl MintToScopeChainConfig {
    pub fn to_mints_to_scope_chains(
        &self,
        oracle_prices: Pubkey,
        scope_entries: &TokenEntryList,
    ) -> Result<(Pubkey, MintsToScopeChains)> {
        let mapping = self.mapping.clone();
        let seed_id = self.user_entry_id;
        let seed_pk = scope_entries
            .get(&seed_id)
            .and_then(|e| e.get_mapping_account())
            .ok_or_else(|| anyhow!("Seed entry not found"))?;
        let seed_id: u64 = seed_id.into();
        let (pk, bump) =
            scope::utils::pdas::mints_to_scope_chains_pubkey(&oracle_prices, &seed_pk, seed_id);
        Ok((
            pk,
            MintsToScopeChains {
                oracle_prices,
                seed_pk,
                seed_id,
                bump,
                mapping,
            },
        ))
    }
}
