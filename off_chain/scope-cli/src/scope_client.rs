use std::mem::size_of;

use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::{Client, Program};

use anchor_client::solana_sdk::{
    clock::{self, Clock},
    compute_budget::ComputeBudgetInstruction,
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction, system_program,
    sysvar::SysvarId,
};

use anyhow::{anyhow, bail, Context, Result};

use nohash_hasher::IntMap;
use scope::{accounts, instruction, Configuration, OracleMappings, OraclePrices};
use tracing::{debug, error, event, info, trace, warn, Level};

use crate::config::{ScopeConfig, TokenConfig, TokenList};
use crate::oracle_helpers::{entry_from_config, TokenEntry};
use crate::utils::{get_clock, price_to_f64};

/// Max number of refresh per tx
const MAX_REFRESH_CHUNK_SIZE: usize = 24;

/// Max compute units to request
// TODO: optimize this so the refresh lists costs less.
const MAX_COMPUTE_UNITS: u32 = 1_400_000;

const EXTRA_LAMPORT_PER_TX: u64 = 10;

const EXTRA_MICRO_LAMPORT_PER_CU: u64 =
    EXTRA_LAMPORT_PER_TX * 1_000_000 / (MAX_COMPUTE_UNITS as u64);

type TokenEntryList = IntMap<u16, Box<dyn TokenEntry>>;

#[derive(Debug)]
pub struct ScopeClient {
    program: Program,
    feed_name: String,
    configuration_acc: Pubkey,
    oracle_prices_acc: Pubkey,
    oracle_mappings_acc: Pubkey,
    tokens: TokenEntryList,
}

impl ScopeClient {
    #[tracing::instrument(skip(client))] //Skip client that does not impl Debug
    pub fn new(client: Client, program_id: Pubkey, price_feed: &str) -> Result<Self> {
        let program = client.program(program_id);

        // Retrieve accounts in configuration PDA
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], &program_id);

        let Configuration { oracle_mappings, oracle_prices, .. } = program
            .account::<Configuration>(configuration_acc)
            .context("Error while retrieving program configuration account, the program might be uninitialized")?;

        debug!(%oracle_prices, %oracle_mappings, %configuration_acc, %price_feed);

        Ok(Self {
            program,
            feed_name: price_feed.to_string(),
            configuration_acc,
            oracle_prices_acc: oracle_prices,
            oracle_mappings_acc: oracle_mappings,
            tokens: IntMap::default(),
        })
    }

    /// Create a new client instance after initializing the program accounts
    #[tracing::instrument(skip(client))]
    pub fn new_init_program(
        client: &Client,
        program_id: &Pubkey,
        price_feed: &str,
    ) -> Result<Self> {
        let program = client.program(*program_id);

        // Generate accounts keypairs.
        let oracle_prices_acc = Keypair::new();
        let oracle_mappings_acc = Keypair::new();

        // Compute configuration PDA pbk
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], program_id);

        Self::ix_initialize(
            &program,
            &configuration_acc,
            &oracle_prices_acc,
            &oracle_mappings_acc,
            price_feed,
        )?;

        debug!(?oracle_prices_acc, "oracle_prices_pbk" = %oracle_prices_acc.pubkey(), ?oracle_mappings_acc, "oracle_mappings_pbk" = %oracle_prices_acc.pubkey(), %configuration_acc);

        Ok(Self {
            program,
            feed_name: price_feed.to_string(),
            configuration_acc,
            oracle_prices_acc: oracle_prices_acc.pubkey(),
            oracle_mappings_acc: oracle_mappings_acc.pubkey(),
            tokens: IntMap::default(),
        })
    }

    /// Set the locally known oracle mapping according to the provided configuration list.
    pub fn set_local_mapping(&mut self, token_list: &ScopeConfig) -> Result<()> {
        let default_max_age = token_list.default_max_age;
        let rpc = self.program.rpc();
        // Transform the configuration entries in appropriate local token entries
        // Local implies to get a copy of needed onchain data (as a cache)
        let tokens_res: Result<TokenEntryList> = token_list
            .tokens
            .iter()
            .map(|(id, token_conf)| {
                let token_entry: Box<dyn TokenEntry> =
                    entry_from_config(token_conf, default_max_age, &rpc)?;
                Ok((*id, token_entry))
            })
            .collect();
        self.tokens = tokens_res?;
        Ok(())
    }

    /// Update the remote oracle mapping from the local
    pub fn upload_oracle_mapping(&self) -> Result<()> {
        let program_mapping = self.get_program_mapping()?;
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
                self.ix_update_mapping(local_mapping_pk, token_idx.into(), loc_price_type_u8)?;
            }
        }
        Ok(())
    }

    /// Update the local oracle mapping from the on-chain version
    pub fn download_oracle_mapping(&mut self, default_max_age: clock::Slot) -> Result<()> {
        let onchain_oracle_mapping = self.get_program_mapping()?;
        let onchain_mapping = onchain_oracle_mapping.price_info_accounts;
        let onchain_types = onchain_oracle_mapping.price_types;

        let zero_pk = Pubkey::default();
        let rpc = self.program.rpc();

        let tokens_res: Result<TokenEntryList> = onchain_mapping
            .iter()
            .enumerate()
            .zip(onchain_types)
            .filter(|((_, &oracle_mapping), _)| oracle_mapping != zero_pk)
            .map(|((idx, oracle_mapping), oracle_type)| {
                let id: u16 = idx.try_into()?;
                let oracle_conf = TokenConfig {
                    label: "".to_string(),
                    oracle_type: oracle_type.try_into()?,
                    max_age: None,
                    oracle_mapping: *oracle_mapping,
                };
                let entry = entry_from_config(&oracle_conf, default_max_age, &rpc)?;
                Ok((id, entry))
            })
            .collect();
        self.tokens = tokens_res?;
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
    pub fn refresh_all_prices(&self) -> Result<()> {
        info!("Refresh all prices");
        // Create chunk of tokens of max `MAX_REFRESH_CHUNK_SIZE` accounts
        let mut acc_account_num = 0_usize;
        let mut acc_token_id: Vec<u16> = Vec::with_capacity(MAX_REFRESH_CHUNK_SIZE);
        let refresh_acc = |token_ids: &[u16]| {
            if let Err(e) = self.ix_refresh_price_list(token_ids) {
                event!(Level::WARN, "err" = ?e, "Refresh of some prices failed");
            }
        };

        for (id, entry) in &self.tokens {
            // if current entry would overflow the token count > send and reset
            if entry.get_number_of_extra_accounts() + 1 + acc_account_num > MAX_REFRESH_CHUNK_SIZE {
                refresh_acc(&acc_token_id);
                acc_account_num = 0;
                acc_token_id.clear()
            }
            // accumulate
            acc_account_num += entry.get_number_of_extra_accounts() + 1;
            acc_token_id.push(*id);
        }

        // last tokens refresh
        if !acc_token_id.is_empty() {
            refresh_acc(&acc_token_id);
        }

        Ok(())
    }

    /// Refresh all prices that has reach 0 ttl
    ///
    /// As an optimization for number of tx, we complete tx with not 0 ttl
    /// if some room is left.
    #[tracing::instrument(skip(self))]
    pub fn refresh_expired_prices(&self) -> Result<()> {
        let mut prices_ttl: Vec<(u16, clock::Slot)> = self.get_prices_ttl()?.collect();

        // Sort the prices ttl from the smallest to biggest.
        prices_ttl.sort_by(|(_, a), (_, b)| a.cmp(b));

        trace!(?prices_ttl);

        // Create chunk of tokens of max `MAX_REFRESH_CHUNK_SIZE` accounts
        let mut acc_account_num = 0_usize;
        let mut acc_token_id: Vec<u16> = Vec::with_capacity(MAX_REFRESH_CHUNK_SIZE);
        let refresh_acc = |token_ids: &[u16]| {
            if let Err(e) = self.ix_refresh_price_list(token_ids) {
                event!(Level::WARN, "err" = ?e, "Refresh of some prices failed");
            }
        };

        for (id, ttl) in &prices_ttl {
            let entry = self
                .tokens
                .get(id)
                .ok_or_else(|| anyhow!("Unknown price at index {id}"))?;
            // if current entry would overflow the token count > send and reset
            if entry.get_number_of_extra_accounts() + 1 + acc_account_num > MAX_REFRESH_CHUNK_SIZE {
                refresh_acc(&acc_token_id);
                acc_account_num = 0;
                acc_token_id.clear();

                if *ttl > 0 {
                    // Current entry is not old enough yet: stop refresh procedure
                    break;
                }
            }
            // accumulate
            acc_account_num += entry.get_number_of_extra_accounts() + 1;
            acc_token_id.push(*id);
        }

        // last tokens refresh
        if !acc_token_id.is_empty() {
            refresh_acc(&acc_token_id);
        }

        Ok(())
    }

    /// Get an iterator over (id, prices_ttl)
    ///
    /// i.e. the number of slot until at the price currently known by scope has reached its `max_age`
    /// Note: If a price `need_refresh` then ttl is forced to 0.
    pub fn get_prices_ttl(&self) -> Result<impl Iterator<Item = (u16, clock::Slot)> + '_> {
        let oracle_prices = self.get_prices()?;

        let current_slot = get_clock(&self.get_rpc())?.slot;

        let it = self.tokens.iter().map(move |(id, entry)| {
            let price = &oracle_prices.prices[usize::from(*id)];
            let price_slot = price.last_updated_slot;
            // default to no remaning slot (ttl=0)
            let age = current_slot.saturating_sub(price_slot);
            let mut remaining_slots = entry.get_max_age().saturating_sub(age);
            match entry.need_refresh(price, &self.get_rpc()) {
                Ok(true) => remaining_slots = 0,
                Err(e) => error!(
                    ?e,
                    "token" = id,
                    "Error while checking if price need refresh"
                ),
                Ok(false) => (), // Nothing to do
            }
            (*id, remaining_slots)
        });
        Ok(it)
    }

    /// Get the minimum remaining time to live of all prices.
    ///
    /// i.e. the number of slot until at least one price has reached its `max_age`
    pub fn get_prices_shortest_ttl(&self) -> Result<clock::Slot> {
        let shortest_ttl = self
            .get_prices_ttl()?
            .map(|(_, ttl)| ttl)
            .min()
            .unwrap_or(0);

        Ok(shortest_ttl)
    }

    /// Log current prices
    /// Note: this uses local mapping
    pub fn log_prices(&self, current_slot: u64) -> Result<()> {
        let prices = self.get_prices()?.prices;

        for (&id, entry) in &self.tokens {
            let dated_price = prices[usize::from(id)];
            let price = price_to_f64(&dated_price.price);
            let exponent = (dated_price.price.exp + 1) as usize;
            let price = format!("{price:.exponent$}");
            let price_type = entry.get_type();
            let age_in_slots: i64 = current_slot as i64 - dated_price.last_updated_slot as i64;
            let max_age = entry.get_max_age() as i64;
            let age_string = if age_in_slots > max_age {
                format!("\x1b[1m\x1b[31m{}\x1b[0m", age_in_slots)
            } else {
                format!("\x1b[32m{}\x1b[0m", age_in_slots)
            };
            trace!(id, %entry, price = ?dated_price.price);
            info!(id, %entry, %price, ?price_type, "age" = %age_string, max_age);
        }
        Ok(())
    }

    /// Return a list (label if available) of expired prices
    pub fn get_expired_prices(&self) -> Result<Vec<String>> {
        Ok(self
            .get_prices_ttl()?
            .filter_map(|(index, ttl)| {
                if ttl == 0 {
                    self.tokens.get(&index).map(|t| t.to_string())
                } else {
                    None
                }
            })
            .collect())
    }

    /// Print a list of all pubkeys that are needed for price refreshed.
    pub fn print_pubkeys(&self) -> Result<()> {
        for entry in self.tokens.values() {
            let main_mapping = entry.get_mapping_account();
            print!("{main_mapping} ");
            let extra_accounts = entry.get_extra_accounts(None)?;
            for account in extra_accounts {
                print!("{account} ");
            }
        }
        println!();
        Ok(())
    }

    /// Get an the rpc instance used by the ScopeClient
    pub fn get_rpc(&self) -> RpcClient {
        self.program.rpc()
    }

    /// Get all prices
    fn get_prices(&self) -> Result<OraclePrices> {
        let prices: OraclePrices = self.program.account(self.oracle_prices_acc)?;
        Ok(prices)
    }

    /// Get program oracle mapping
    fn get_program_mapping(&self) -> Result<OracleMappings> {
        let mapping: OracleMappings = self.program.account(self.oracle_mappings_acc)?;
        Ok(mapping)
    }

    #[tracing::instrument(skip(program))]
    fn ix_initialize(
        program: &Program,
        configuration_acc: &Pubkey,
        oracle_prices_acc: &Keypair,
        oracle_mappings_acc: &Keypair,
        price_feed: &str,
    ) -> Result<()> {
        debug!("Entering initialize ix");

        // Prepare init instruction accounts
        let init_account = accounts::Initialize {
            admin: program.payer(),
            system_program: system_program::ID,
            configuration: *configuration_acc,
            oracle_prices: oracle_prices_acc.pubkey(),
            oracle_mappings: oracle_mappings_acc.pubkey(),
        };

        let rpc = program.rpc();

        let init_res = program
            .request()
            // Create the price account
            .instruction(system_instruction::create_account(
                &program.payer(),
                &oracle_prices_acc.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(8_usize + size_of::<OraclePrices>())?,
                8_u64 + u64::try_from(size_of::<OraclePrices>()).unwrap(), //constant, it cannot fail
                &program.id(),
            ))
            // Create the oracle mapping account
            .instruction(system_instruction::create_account(
                &program.payer(),
                &oracle_mappings_acc.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(8_usize + size_of::<OracleMappings>())?,
                8_u64 + u64::try_from(size_of::<OracleMappings>()).unwrap(), //constant, it cannot fail
                &program.id(),
            ))
            .signer(oracle_prices_acc)
            .signer(oracle_mappings_acc)
            .accounts(init_account)
            .args(instruction::Initialize {
                feed_name: price_feed.to_string(),
            })
            .send();

        debug!("Init ix result: {:#?}", init_res);
        init_res.context("Failed to initialize the account")?;

        info!("Accounts initialized successfully");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn ix_update_mapping(&self, oracle_account: &Pubkey, token: u64, price_type: u8) -> Result<()> {
        let update_account = accounts::UpdateOracleMapping {
            admin: self.program.payer(),
            configuration: self.configuration_acc,
            oracle_mappings: self.oracle_mappings_acc,
            price_info: *oracle_account,
        };

        let request = self.program.request();

        let res = request
            .accounts(update_account)
            .args(instruction::UpdateMapping {
                token,
                price_type,
                feed_name: self.feed_name.clone(),
            })
            .send();

        match res {
            Ok(sig) => info!(signature = %sig, "Accounts updated successfully"),
            Err(err) => {
                error!(err = ?err, "Mapping update failed");
                bail!(err);
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn ix_refresh_one_price(&self, token: u16) -> Result<()> {
        let entry = self
            .tokens
            .get(&token)
            .ok_or_else(|| anyhow!("Unexpected token id {token}"))?;
        let refresh_account = accounts::RefreshOne {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            price_info: *entry.get_mapping_account(),
            clock: Clock::id(),
        };

        let mut request = self.program.request();

        request = request
            .accounts(refresh_account)
            .args(instruction::RefreshOnePrice {
                token: token.into(),
            });

        for extra in entry.get_extra_accounts(None)? {
            request = request.accounts(AccountMeta::new_readonly(extra, false));
        }

        let tx = request.send()?;

        info!(%tx, "Price refreshed successfully");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn ix_refresh_price_list(&self, tokens: &[u16]) -> Result<()> {
        let refresh_account = accounts::RefreshList {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            clock: Clock::id(),
        };

        let rpc = self.program.rpc();

        let request = self.program.request();

        let mut request = request.accounts(refresh_account);

        for token_idx in tokens {
            let entry = self
                .tokens
                .get(token_idx)
                .ok_or_else(|| anyhow!("Unexpected token {token_idx}"))?;
            // Note: no control at this point, all token accounts will be sent in on tx
            request = request.accounts(AccountMeta::new_readonly(
                *entry.get_mapping_account(),
                false,
            ));
            for extra in entry.get_extra_accounts(Some(&rpc))? {
                request = request.accounts(AccountMeta::new_readonly(extra, false));
            }
        }

        let tokens = tokens.to_vec();

        let tx_res = request
            .instruction(ComputeBudgetInstruction::set_compute_unit_limit(
                MAX_COMPUTE_UNITS,
            ))
            // 10 extra lamports per tx
            .instruction(ComputeBudgetInstruction::set_compute_unit_price(
                EXTRA_MICRO_LAMPORT_PER_CU,
            ))
            .args(instruction::RefreshPriceList { tokens })
            .send();

        match tx_res {
            Ok(sig) => info!(signature = %sig, "Prices list refreshed successfully"),
            Err(err) => match err {
                anchor_client::ClientError::SolanaClientError(e) => {
                    info!("Prices list refresh failed: RPC Client error: {e:#?}");
                    // We could `bail!` here but we want to avoid double logs,
                }
                _ => {
                    warn!("Price list refresh failed: {:#?}", err);
                    // We could `bail!` here but we want to avoid double logs,
                }
            },
        }

        Ok(())
    }
}
