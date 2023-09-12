use std::mem::size_of;

use anchor_client::{
    anchor_lang::ToAccountMetas,
    solana_sdk::{
        clock::{self, Clock},
        instruction::AccountMeta,
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        signer::Signer,
        system_program,
        sysvar::{instructions::ID as SYSVAR_INSTRUCTIONS_ID, SysvarId},
    },
};
use anyhow::{anyhow, bail, Context, Result};
use futures::future::join_all;
use nohash_hasher::IntMap;
use orbit_link::{async_client::AsyncClient, OrbitLink};
use scope::{accounts, instruction, Configuration, OracleMappings, OraclePrices};
use tracing::{debug, error, info, trace, warn};

use crate::{
    config::{ScopeConfig, TokenConfig, TokenList},
    oracle_helpers::{entry_from_config, TokenEntry},
    utils::{get_clock, price_to_f64},
};

/// Max number of refresh per tx
const MAX_REFRESH_CHUNK_SIZE: usize = 24;
/// Token gap to max age that still trigger refresh (in slots)
const REMAINING_AGE_TO_REFRESH: i64 = 10;

type TokenEntryList = IntMap<u16, Box<dyn TokenEntry>>;

pub struct ScopeClient<T: AsyncClient, S: Signer> {
    client: OrbitLink<T, S>,
    program_id: Pubkey,
    feed_name: String,
    configuration_acc: Pubkey,
    oracle_prices_acc: Pubkey,
    oracle_mappings_acc: Pubkey,
    tokens: TokenEntryList,
}

impl<T, S> ScopeClient<T, S>
where
    T: AsyncClient,
    S: Signer,
{
    #[tracing::instrument(skip(client))] //Skip client that does not impl Debug
    pub async fn new(
        client: OrbitLink<T, S>,
        program_id: Pubkey,
        price_feed: &str,
    ) -> Result<Self> {
        // Retrieve accounts in configuration PDA
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], &program_id);

        let Configuration { oracle_mappings, oracle_prices, .. } = client
            .get_anchor_account::<Configuration>(&configuration_acc).await
            .context("Error while retrieving program configuration account, the program might be uninitialized")?;

        debug!(%oracle_prices, %oracle_mappings, %configuration_acc, %price_feed);

        Ok(Self {
            client,
            program_id,
            feed_name: price_feed.to_string(),
            configuration_acc,
            oracle_prices_acc: oracle_prices,
            oracle_mappings_acc: oracle_mappings,
            tokens: IntMap::default(),
        })
    }

    /// Create a new client instance after initializing the program accounts
    #[tracing::instrument(skip(client))]
    pub async fn new_init_program(
        client: OrbitLink<T, S>,
        program_id: &Pubkey,
        price_feed: &str,
    ) -> Result<Self> {
        // Generate accounts keypairs.
        let oracle_prices_acc = Keypair::new();
        let oracle_mappings_acc = Keypair::new();

        // Compute configuration PDA pbk
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], program_id);

        Self::ix_initialize(
            &client,
            program_id,
            &configuration_acc,
            &oracle_prices_acc,
            &oracle_mappings_acc,
            price_feed,
        )
        .await?;

        debug!(?oracle_prices_acc, "oracle_prices_pbk" = %oracle_prices_acc.pubkey(), ?oracle_mappings_acc, "oracle_mappings_pbk" = %oracle_prices_acc.pubkey(), %configuration_acc);

        Ok(Self {
            client,
            program_id: *program_id,
            feed_name: price_feed.to_string(),
            configuration_acc,
            oracle_prices_acc: oracle_prices_acc.pubkey(),
            oracle_mappings_acc: oracle_mappings_acc.pubkey(),
            tokens: IntMap::default(),
        })
    }

    /// Set the locally known oracle mapping according to the provided configuration list.
    pub async fn set_local_mapping(&mut self, token_list: &ScopeConfig) -> Result<()> {
        let default_max_age = token_list.default_max_age;
        let rpc = self.get_rpc();
        // Transform the configuration entries in appropriate local token entries
        // Local implies to get a copy of needed onchain data (as a cache)
        let tokens_res: Result<TokenEntryList> =
            join_all(token_list.tokens.iter().map(|(id, token_conf)| async {
                let token_entry: Box<dyn TokenEntry> =
                    entry_from_config(token_conf, default_max_age, rpc).await?;
                Ok((*id, token_entry))
            }))
            .await
            .into_iter()
            .collect();
        self.tokens = tokens_res?;
        Ok(())
    }

    /// Update the remote oracle mapping from the local
    pub async fn upload_oracle_mapping(&self) -> Result<()> {
        let program_mapping = self.get_program_mapping().await?;
        let onchain_accounts_mapping = program_mapping.price_info_accounts;
        let onchain_price_type_mapping = program_mapping.price_types;

        // For all "token" local and remote
        for (&token_idx, local_entry) in &self.tokens {
            let idx: usize = token_idx.try_into().unwrap();
            let rem_mapping = &onchain_accounts_mapping[idx];
            let rem_price_type = onchain_price_type_mapping[idx];
            // Update remote in case of difference
            let local_mapping_pk = local_entry.get_mapping_account();
            let loc_price_type_u8: u8 = local_entry.get_type().into();
            if rem_mapping != local_mapping_pk || rem_price_type != loc_price_type_u8 {
                self.ix_update_mapping(local_mapping_pk, token_idx.into(), loc_price_type_u8)
                    .await?;
            }
        }
        Ok(())
    }

    /// Update the local oracle mapping from the on-chain version
    pub async fn download_oracle_mapping(&mut self, default_max_age: clock::Slot) -> Result<()> {
        let onchain_oracle_mapping = self.get_program_mapping().await?;
        let onchain_mapping = onchain_oracle_mapping.price_info_accounts;
        let onchain_types = onchain_oracle_mapping.price_types;

        let zero_pk = Pubkey::default();
        let rpc = self.get_rpc();

        let entry_builders = onchain_mapping
            .iter()
            .enumerate()
            .zip(onchain_types)
            .filter(|((_, &oracle_mapping), _)| oracle_mapping != zero_pk)
            .map(|((idx, &oracle_mapping), oracle_type)| async move {
                let id: u16 = idx.try_into()?;
                let oracle_conf = TokenConfig {
                    label: "".to_string(),
                    oracle_type: oracle_type.try_into()?,
                    max_age: None,
                    oracle_mapping,
                };
                let entry = entry_from_config(&oracle_conf, default_max_age, rpc).await?;
                Result::<(u16, Box<dyn TokenEntry>)>::Ok((id, entry))
            });

        self.tokens = join_all(entry_builders)
            .await
            .into_iter()
            .collect::<Result<TokenEntryList>>()?;
        Ok(())
    }

    /// Extract the local oracle mapping to a token list configuration
    pub fn get_local_mapping(&self) -> Result<ScopeConfig> {
        let tokens: TokenList = self
            .tokens
            .iter()
            .map(|(id, entry)| {
                (
                    *id,
                    TokenConfig {
                        label: entry.to_string(),
                        oracle_mapping: *entry.get_mapping_account(),
                        oracle_type: entry.get_type(),
                        max_age: None,
                    },
                )
            })
            .collect();
        Ok(ScopeConfig {
            tokens,
            default_max_age: 0,
        })
    }

    /// Refresh all price referenced in oracle mapping
    ///
    /// We will use [`ScopeClient::ix_refresh_price_list`] for this method.
    /// The ix has a hard limit of [`MAX_REFRESH_CHUNK_SIZE`] accounts that needs
    /// to be carefully taken care of since the number of accounts varies from
    /// one token to another.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_all_prices(&self) -> Result<()> {
        info!("Refresh all prices");
        // Create chunk of tokens of max `MAX_REFRESH_CHUNK_SIZE` accounts
        let mut acc_account_num = 0_usize;
        let mut acc_token_id: Vec<u16> = Vec::with_capacity(MAX_REFRESH_CHUNK_SIZE);

        let mut refresh_futures = Vec::new();

        for (id, entry) in &self.tokens {
            // if current entry would overflow the token count > send and reset
            if entry.get_number_of_extra_accounts() + 1 + acc_account_num > MAX_REFRESH_CHUNK_SIZE {
                refresh_futures.push(self.refresh_price_list_print_res(acc_token_id.clone()));
                acc_account_num = 0;
                acc_token_id.clear()
            }
            // accumulate
            acc_account_num += entry.get_number_of_extra_accounts() + 1;
            acc_token_id.push(*id);
        }

        // last tokens refresh
        if !acc_token_id.is_empty() {
            refresh_futures.push(self.refresh_price_list_print_res(acc_token_id));
        }

        join_all(refresh_futures).await;

        Ok(())
    }

    /// Refresh all prices that has reach 0 ttl
    ///
    /// As an optimization for number of tx, we complete tx with not 0 ttl
    /// if some room is left.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_old_prices(&self) -> Result<()> {
        let mut prices_ttl: Vec<(u16, i64)> = self.get_prices_ttl().await?.collect();
        // TODO: filter prices that cannot be refreshed
        // Sort the prices ttl from the smallest to biggest.
        prices_ttl.sort_by(|(_, a), (_, b)| a.cmp(b));

        trace!(?prices_ttl);

        // Keep only the prices that are below REMAINING_AGE_TO_REFRESH
        prices_ttl.retain(|(_, ttl)| *ttl < REMAINING_AGE_TO_REFRESH);

        // Create chunk of tokens of max `MAX_REFRESH_CHUNK_SIZE` accounts
        let mut acc_account_num = 0_usize;
        let mut acc_token_id: Vec<u16> = Vec::with_capacity(MAX_REFRESH_CHUNK_SIZE);
        let mut refresh_futures = Vec::new();

        for (id, _ttl) in &prices_ttl {
            let entry = self
                .tokens
                .get(id)
                .ok_or_else(|| anyhow!("Unknown price at index {id}"))?;
            // if current entry would overflow the token count > send and reset
            if entry.get_number_of_extra_accounts() + 1 + acc_account_num > MAX_REFRESH_CHUNK_SIZE {
                refresh_futures.push(self.refresh_price_list_print_res(acc_token_id.clone()));
                acc_account_num = 0;
                acc_token_id.clear();
            }
            // accumulate
            acc_account_num += entry.get_number_of_extra_accounts() + 1;
            acc_token_id.push(*id);
        }

        // last tokens refresh
        if !acc_token_id.is_empty() {
            refresh_futures.push(self.refresh_price_list_print_res(acc_token_id));
        }

        join_all(refresh_futures).await;

        Ok(())
    }

    /// Get an iterator over `(id, price_ttl)`
    ///
    /// i.e. the number of slot until at the price currently known by scope has reached its `max_age`
    /// Note: negative `price_ttl` gives how much expired is the price
    pub async fn get_prices_ttl(&self) -> Result<impl Iterator<Item = (u16, i64)> + '_> {
        let oracle_prices = self.get_prices().await?;

        let rpc = self.get_rpc();

        let current_slot = get_clock(rpc).await?.slot;

        let it = self.tokens.iter().map(move |(id, entry)| {
            let price = &oracle_prices.prices[usize::from(*id)];
            let price_slot = price.last_updated_slot;
            // default to age == 0 if "updated in the future"
            let age = current_slot.saturating_sub(price_slot);

            let remaining_slots: i64 = if age > clock::DEFAULT_SLOTS_PER_EPOCH {
                // Age is more than one epoch, assume it is infinitely old.
                i64::MIN
            } else if entry.get_max_age() > i64::MAX as u64 {
                // Max age is too high default to "infinite" ttl
                i64::MAX
            } else {
                // No overflow possible thanks to the previous checks
                entry.get_max_age() as i64 - age as i64
            };
            (*id, remaining_slots)
        });
        Ok(it)
    }

    /// Get the minimum remaining time to live of all prices.
    ///
    /// i.e. the number of slot until at least one price has reached its `max_age`
    pub async fn get_prices_shortest_ttl(&self) -> Result<i64> {
        let shortest_ttl = self
            .get_prices_ttl()
            .await?
            .map(|(_, ttl)| ttl)
            .min()
            .unwrap_or(0);

        Ok(shortest_ttl)
    }

    /// Log current prices
    /// Note: this uses local mapping
    pub async fn log_prices(&self, current_slot: u64) -> Result<()> {
        let prices = self.get_prices().await?.prices;

        for (&id, entry) in &self.tokens {
            let dated_price = prices[usize::from(id)];
            let price = price_to_f64(&dated_price.price);
            let exponent = (dated_price.price.exp + 1) as usize;
            let price_type = entry.get_type();
            let age_in_slots: i64 = current_slot as i64 - dated_price.last_updated_slot as i64;
            let max_age = entry.get_max_age() as i64;
            let age_string = if age_in_slots > max_age {
                format!("\x1b[1m\x1b[31m{age_in_slots}\x1b[0m")
            } else {
                format!("\x1b[32m{age_in_slots}\x1b[0m")
            };
            // For easier parsing of these logs don't use tracing here.
            println!("id={id}, entry='{entry}', price='{price:.exponent$}', price_type='{price_type:?}', age={age_in_slots}, age_c={age_string}, max_age={max_age}");
        }
        Ok(())
    }

    /// Return a list (label if available) of expired prices
    pub async fn get_expired_prices(&self) -> Result<Vec<String>> {
        Ok(self
            .get_prices_ttl()
            .await?
            .filter_map(|(index, ttl)| {
                if ttl <= 0 {
                    self.tokens.get(&index).map(|t| t.to_string())
                } else {
                    None
                }
            })
            .collect())
    }

    /// Print a list of all pubkeys that are needed for price refreshed.
    pub async fn print_pubkeys(&self) -> Result<()> {
        for entry in self.tokens.values() {
            let main_mapping = entry.get_mapping_account();
            print!("{main_mapping} ");
            let extra_accounts = entry.get_extra_accounts(None).await?;
            for account in extra_accounts {
                print!("{account} ");
            }
        }
        println!();
        Ok(())
    }

    /// Get an the rpc instance used by the ScopeClient
    pub fn get_rpc(&self) -> &T {
        &self.client.client
    }

    /// Get all prices
    async fn get_prices(&self) -> Result<OraclePrices> {
        let prices: OraclePrices = self
            .client
            .get_anchor_account(&self.oracle_prices_acc)
            .await?;
        Ok(prices)
    }

    /// Get program oracle mapping
    async fn get_program_mapping(&self) -> Result<OracleMappings> {
        let mapping: OracleMappings = self
            .client
            .get_anchor_account(&self.oracle_mappings_acc)
            .await?;
        Ok(mapping)
    }

    #[tracing::instrument(skip(client))]
    async fn ix_initialize(
        client: &OrbitLink<T, S>,
        program_id: &Pubkey,
        configuration_acc: &Pubkey,
        oracle_prices_acc: &Keypair,
        oracle_mappings_acc: &Keypair,
        price_feed: &str,
    ) -> Result<()> {
        debug!("Entering initialize ix");

        // Prepare init instruction accounts
        let init_account = accounts::Initialize {
            admin: client.payer(),
            system_program: system_program::ID,
            configuration: *configuration_acc,
            oracle_prices: oracle_prices_acc.pubkey(),
            oracle_mappings: oracle_mappings_acc.pubkey(),
        };

        let init_tx = client
            .tx_builder()
            // Create the price account
            .add_ix_with_budget(
                client
                    .create_account_ix(
                        &oracle_prices_acc.pubkey(),
                        size_of::<OraclePrices>() + 8,
                        program_id,
                    )
                    .await?,
                50_000,
            )
            // Create the oracle mapping account
            .add_ix_with_budget(
                client
                    .create_account_ix(
                        &oracle_mappings_acc.pubkey(),
                        size_of::<OracleMappings>() + 8,
                        program_id,
                    )
                    .await?,
                50_000,
            )
            .add_anchor_ix(
                program_id,
                init_account,
                instruction::Initialize {
                    feed_name: price_feed.to_string(),
                },
            )
            .build_with_budget_and_fee(&[oracle_prices_acc, oracle_mappings_acc])
            .await?;

        let (signature, init_res) = client.send_retry_and_confirm_transaction(init_tx).await?;

        info!(%signature, "Init tx");
        match init_res {
            Some(r) => r.context(format!("Init transaction: {signature}")),
            None => bail!("Init transaction failed to confirm: {signature}"),
        }
    }

    #[tracing::instrument(skip(self))]
    async fn ix_update_mapping(
        &self,
        oracle_account: &Pubkey,
        token: u64,
        price_type: u8,
    ) -> Result<()> {
        let update_accounts = accounts::UpdateOracleMapping {
            admin: self.client.payer(),
            configuration: self.configuration_acc,
            oracle_mappings: self.oracle_mappings_acc,
            price_info: *oracle_account,
        };

        let request = self.client.tx_builder();

        let tx = request
            .add_anchor_ix(
                &self.program_id,
                update_accounts,
                instruction::UpdateMapping {
                    token,
                    price_type,
                    feed_name: self.feed_name.clone(),
                },
            )
            .build_with_budget_and_fee(&[])
            .await?;

        let (signature, res) = self.client.send_retry_and_confirm_transaction(tx).await?;

        match res {
            Some(Ok(())) => info!(%signature, "Accounts updated successfully"),
            Some(Err(err)) => {
                error!(%signature, err = ?err, "Mapping update failed");
                bail!(err);
            }
            None => {
                error!(%signature, "Could not confirm mapping update transaction");
                bail!("Could not confirm mapping update transaction");
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn ix_refresh_one_price(&self, token: u16) -> Result<()> {
        let entry = self
            .tokens
            .get(&token)
            .ok_or_else(|| anyhow!("Unexpected token id {token}"))?;
        let mut refresh_accounts = accounts::RefreshOne {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            price_info: *entry.get_mapping_account(),
            clock: Clock::id(),
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        // Add eventual extra accounts (depends on price type)
        refresh_accounts.extend(
            entry
                .get_extra_accounts(None)
                .await?
                .iter()
                .map(|acc| AccountMeta::new_readonly(*acc, false)),
        );

        let request = self.client.tx_builder();

        let tx = request
            .add_anchor_ix(
                &self.program_id,
                refresh_accounts,
                instruction::RefreshOnePrice {
                    token: token.into(),
                },
            )
            .build_with_budget_and_fee(&[])
            .await?;

        let (signature, res) = self.client.send_and_confirm_transaction(tx).await?;

        info!(%signature, "Price refresh tx sent");

        match res {
            Some(r) => r.context(format!("Price refresh transaction: {signature}")),
            None => bail!("Init transaction failed to confirm: {signature}"),
        }
    }

    async fn ix_refresh_price_list(&self, tokens: &[u16]) -> Result<Signature> {
        let mut refresh_accounts = accounts::RefreshList {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            clock: Clock::id(),
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        let rpc = self.get_rpc();
        let mut cu_budget = 15_000;

        for token_idx in tokens {
            let entry = self
                .tokens
                .get(token_idx)
                .ok_or_else(|| anyhow!("Unexpected token {token_idx}"))?;
            // Note: no control at this point, all token accounts will be sent in on tx
            refresh_accounts.push(AccountMeta::new_readonly(
                *entry.get_mapping_account(),
                false,
            ));
            for extra in entry.get_extra_accounts(Some(rpc)).await? {
                refresh_accounts.push(AccountMeta::new_readonly(extra, false));
            }
            cu_budget += entry.get_update_cu_budget();
        }

        let tokens = tokens.to_vec();

        let tx = self
            .client
            .tx_builder()
            .add_anchor_ix_with_budget(
                &self.program_id,
                refresh_accounts,
                instruction::RefreshPriceList { tokens },
                cu_budget,
            )
            .build_with_budget_and_fee(&[])
            .await?;

        let (signature, tx_res) = self.client.send_and_confirm_transaction(tx).await?;

        match tx_res {
            Some(Ok(())) => {
                info!(%signature, "Prices list refreshed successfully");
            }
            Some(Err(err)) => {
                error!(%signature, ?err, "Failed to refresh price list");
            }
            None => {
                info!(%signature, "Could not confirm refresh price list transaction");
            }
        }

        Ok(signature)
    }

    #[tracing::instrument(skip(self))]
    async fn refresh_price_list_print_res(&self, tokens: Vec<u16>) {
        if let Err(err) = self.ix_refresh_price_list(&tokens).await {
            warn!(?err, "Error while sending refresh price list transaction");
            // Ok case already printed
        }
    }
}
