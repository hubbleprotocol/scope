use std::mem::size_of;

use anchor_client::{Client, Program};

use solana_sdk::clock;
use solana_sdk::{
    clock::Clock, instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction, system_program, sysvar::SysvarId,
};

use anyhow::{anyhow, bail, Context, Result};

use scope::{accounts, instruction, Configuration, OracleMappings, OraclePrices};
use tracing::{debug, error, event, info, span, trace, warn, Level};

use crate::config::{TokenConf, TokenConfList};
use crate::utils::find_data_address;

/// Max number of refresh per tx
const MAX_REFRESH_CHUNK_SIZE: usize = 27;

/// Default value for token_pairs
const EMPTY_STRING: String = String::new();

#[derive(Debug)]
pub struct ScopeClient {
    program: Program,
    program_data_acc: Pubkey,
    oracle_prices_acc: Pubkey,
    oracle_mappings_acc: Pubkey,
    oracle_mappings: [Option<Pubkey>; scope::MAX_ENTRIES],
    token_pairs: [String; scope::MAX_ENTRIES],
}

impl ScopeClient {
    #[tracing::instrument(skip(client))] //Skip client that does not impl Debug
    pub fn new(client: Client, program_id: Pubkey, price_feed: &str) -> Result<Self> {
        let program = client.program(program_id);
        let program_data_acc = find_data_address(&program_id);

        // Retrieve accounts in configuration PDA
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], &program_id);

        let Configuration { oracle_mappings_pbk, oracle_prices_pbk, .. } = program
            .account::<Configuration>(configuration_acc)
            .context("Error while retrieving program configuration account, the program might be uninitialized")?;

        Ok(Self {
            program,
            program_data_acc,
            oracle_prices_acc: oracle_prices_pbk,
            oracle_mappings_acc: oracle_mappings_pbk,
            oracle_mappings: [None; scope::MAX_ENTRIES],
            token_pairs: [EMPTY_STRING; scope::MAX_ENTRIES],
        })
    }

    /// Create a new client instance after initializing the program accounts
    pub fn new_init_program(
        client: &Client,
        program_id: &Pubkey,
        price_feed: &str,
    ) -> Result<Self> {
        let program = client.program(*program_id);

        let program_data_acc = find_data_address(program_id);

        // Generate accounts keypairs.
        let oracle_prices_acc = Keypair::new();
        let oracle_mappings_acc = Keypair::new();

        // Compute configuration PDA pbk
        let (configuration_acc, _) =
            Pubkey::find_program_address(&[b"conf", price_feed.as_bytes()], program_id);

        Self::ix_initialize(
            &program,
            &program_data_acc,
            &configuration_acc,
            &oracle_prices_acc,
            &oracle_mappings_acc,
            price_feed,
        )?;

        Ok(Self {
            program,
            program_data_acc,
            oracle_prices_acc: oracle_prices_acc.pubkey(),
            oracle_mappings_acc: oracle_mappings_acc.pubkey(),
            oracle_mappings: [None; scope::MAX_ENTRIES],
            token_pairs: [EMPTY_STRING; scope::MAX_ENTRIES],
        })
    }

    /// Set the locally known oracle mapping according to the provided configuration list.
    pub fn set_local_mapping(&mut self, token_list: &TokenConfList) -> Result<()> {
        for (idx, token) in &token_list.tokens {
            let idx = usize::try_from(*idx)?;
            if idx >= scope::MAX_ENTRIES {
                bail!("Out of range token index provided in token list configuration");
            }
            self.oracle_mappings[idx] = Some(token.oracle_mapping);
            self.token_pairs[idx] = token.token_pair.clone();
        }
        Ok(())
    }

    /// Update the remote oracle mapping from the local
    pub fn upload_oracle_mapping(&self) -> Result<()> {
        let onchain_mapping = self.get_program_mapping()?.price_info_accounts;

        // For all "token" local and remote
        for (token, (loc_mapping, rem_mapping)) in
            self.oracle_mappings.iter().zip(onchain_mapping).enumerate()
        {
            // Update remote in case of difference
            let loc_pk = loc_mapping.unwrap_or_default();
            if rem_mapping != loc_pk {
                self.ix_update_mapping(&loc_pk, token.try_into()?)?;
            }
        }
        Ok(())
    }

    /// Update the local oracle mapping from the on-chain version
    pub fn download_oracle_mapping(&mut self) -> Result<()> {
        let onchain_mapping = self.get_program_mapping()?.price_info_accounts;
        let zero_pk = Pubkey::default();
        for (loc_mapping, rem_mapping) in self.oracle_mappings.iter_mut().zip(onchain_mapping) {
            *loc_mapping = if rem_mapping == zero_pk {
                None
            } else {
                Some(rem_mapping)
            };
        }
        Ok(())
    }

    /// Extract the local oracle mapping to a token list configuration
    pub fn get_local_mapping(&self) -> Result<TokenConfList> {
        let tokens: Vec<_> = self
            .oracle_mappings
            .iter()
            .enumerate()
            .zip(self.token_pairs.iter())
            .filter_map(|((idx, mapping_op), pair)| {
                mapping_op.as_ref().map(|mapping| {
                    (
                        u64::try_from(idx).unwrap(),
                        TokenConf {
                            token_pair: pair.clone(),
                            oracle_mapping: *mapping,
                        },
                    )
                })
            })
            .collect();
        Ok(TokenConfList { tokens })
    }

    #[tracing::instrument(skip(self))]
    /// Refresh all price referenced in oracle mapping
    pub fn refresh_all_prices(&self) -> Result<()> {
        info!("Refresh all prices");
        let to_refresh_idx: Vec<u16> = self
            .oracle_mappings
            .iter()
            .enumerate()
            .filter_map(|(idx, e)| {
                if e.is_some() {
                    Some(u16::try_from(idx).unwrap())
                } else {
                    None
                }
            })
            .collect();

        for (nb, chunk) in to_refresh_idx.chunks(MAX_REFRESH_CHUNK_SIZE).enumerate() {
            let _span = span!(Level::TRACE, "refresh_chunk", "chunk.nb" = %nb, ?chunk).entered();
            if let Err(e) = self.ix_refresh_price_list(chunk) {
                event!(Level::ERROR, "err" = ?e, "Refresh of some prices failed");
            }
        }

        Ok(())
    }

    /// Refresh all prices older than given number of slots
    ///
    /// As an optimization for number of tx. The prices are divided in chunk by age.
    /// If one token is too old at least `MAX_REFRESH_CHUNK_SIZE` tokens will be
    /// refreshed.
    #[tracing::instrument(skip(self))]
    pub fn refresh_prices_older_than(&self, max_age: clock::Slot) -> Result<()> {
        let oracle_prices = self.get_prices()?;

        let mut prices: Vec<_> = oracle_prices
            .prices
            .iter()
            .zip(self.oracle_mappings) // Iterate with mappings to ensure the price is usable
            .enumerate() // keep track of indexes, needed for refresh
            .filter_map(|(idx, (dp, mapping_op))| mapping_op.map(|_| (idx, dp.last_updated_slot)))
            .collect();

        // Sort the prices from the oldest to the youngest.
        prices.sort_by(|a, b| a.1.cmp(&b.1));

        let clock: Clock = self
            .program
            .rpc()
            .get_account(&Clock::id())?
            .deserialize_data()?;

        let current_slot = clock.slot;
        trace!(current_slot);

        for (nb, chunk) in prices.chunks(MAX_REFRESH_CHUNK_SIZE).enumerate() {
            let _span = span!(Level::TRACE, "evaluate_chunk", "chunk.nb" = %nb, ?chunk).entered();
            let price_slot = chunk[0].1;
            let age = current_slot
                .checked_sub(price_slot)
                .ok_or(anyhow!("Some prices have been updated in the future"))?;

            if age >= max_age {
                let price_ids: Vec<_> = chunk
                    .iter()
                    .map(|(idx, _)| u16::try_from(*idx).unwrap())
                    .collect();
                debug!("Refresh chunk: {:?}", price_ids);
                if let Err(e) = self.ix_refresh_price_list(&price_ids) {
                    event!(Level::ERROR, "err" = ?e, "Refresh of some prices failed");
                } else {
                    let new_prices = self.get_prices()?;
                    // if any price has the same date as previously in the chunk
                    if let Some((id, _)) = chunk.iter().find(|(idx, _)| {
                        new_prices.prices[*idx].last_updated_slot
                            == oracle_prices.prices[*idx].last_updated_slot
                    }) {
                        event!(
                            Level::WARN,
                            "chunk" = ?chunk,
                            "first_failed_id" = ?id,
                            "Refresh of some prices failed"
                        );
                    }
                }
            } else {
                trace!("Chunk is too recent, stop");
                break;
            }
        }

        Ok(())
    }

    /// Get age in slots of the oldest price
    pub fn get_oldest_price_age(&self) -> Result<clock::Slot> {
        let oracle_prices = self.get_prices()?;

        let oldest_price_slot = oracle_prices
            .prices
            .iter()
            .zip(self.oracle_mappings) // Iterate with mappings to ensure the price is usable
            .filter_map(|(dp, mapping_op)| mapping_op.map(|_| dp.last_updated_slot))
            .min()
            .unwrap_or(0);

        trace!(oldest_price_slot);

        let clock: Clock = self
            .program
            .rpc()
            .get_account(&Clock::id())?
            .deserialize_data()?;

        let age = clock
            .slot
            .checked_sub(oldest_price_slot)
            .ok_or(anyhow!("Some prices have been updated in the future"))?;

        Ok(age)
    }

    /// Get all prices
    pub fn get_prices(&self) -> Result<OraclePrices> {
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
        program_data_acc: &Pubkey,
        configuration_acc: &Pubkey,
        oracle_prices_acc: &Keypair,
        oracle_mappings_acc: &Keypair,
        price_feed: &str,
    ) -> Result<()> {
        debug!("Entering initialize ix");

        // Prepare init instruction accounts
        let init_account = accounts::Initialize {
            admin: program.payer(),
            program: program.id(),
            program_data: *program_data_acc,
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
    fn ix_update_mapping(&self, oracle_account: &Pubkey, token: u64) -> Result<()> {
        let update_account = accounts::UpdateOracleMapping {
            oracle_mappings: self.oracle_mappings_acc,
            pyth_price_info: *oracle_account,
            program: self.program.id(),
            program_data: self.program_data_acc,
            admin: self.program.payer(),
        };

        let request = self.program.request();

        let res = request
            .accounts(update_account)
            .args(instruction::UpdateMapping { token })
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
    pub fn ix_refresh_one_price(&self, token: u64) -> Result<()> {
        let oracle_account = self
            .oracle_mappings
            .get(usize::try_from(token)?)
            .ok_or(anyhow!("Out of range token {token}"))?
            .unwrap_or_default();
        let refresh_account = accounts::RefreshOne {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            pyth_price_info: oracle_account,
            clock: Clock::id(),
        };

        let request = self.program.request();

        request
            .accounts(refresh_account)
            .args(instruction::RefreshOnePrice { token })
            .send()?;

        info!("Price refreshed successfully");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn ix_refresh_8_prices(&self, first_token: u64) -> Result<()> {
        let first_token_idx = usize::try_from(first_token)?;
        let oracle_accounts: Vec<Pubkey> = self.oracle_mappings
            [first_token_idx..first_token_idx.checked_add(8).unwrap()]
            .iter()
            .map(|op_pk| op_pk.unwrap_or_default())
            .collect();

        let refresh_account = accounts::RefreshBatch {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            pyth_price_info_0: oracle_accounts[0],
            pyth_price_info_1: oracle_accounts[1],
            pyth_price_info_2: oracle_accounts[2],
            pyth_price_info_3: oracle_accounts[3],
            pyth_price_info_4: oracle_accounts[4],
            pyth_price_info_5: oracle_accounts[5],
            pyth_price_info_6: oracle_accounts[6],
            pyth_price_info_7: oracle_accounts[7],
            clock: Clock::id(),
        };

        let request = self.program.request();

        request
            .accounts(refresh_account)
            .args(instruction::RefreshBatchPrices { first_token })
            .send()?;

        info!("Prices refreshed successfully");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn ix_refresh_price_list(&self, tokens: &[u16]) -> Result<()> {
        let refresh_account = accounts::RefreshList {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            clock: Clock::id(),
        };

        let request = self.program.request();

        let mut request = request.accounts(refresh_account);

        for token_idx in tokens.iter() {
            let oracle_pubkey_op = self
                .oracle_mappings
                .get(usize::from(*token_idx))
                .ok_or(anyhow!("Out of range token {token_idx}"))?;

            if let Some(oracle_pubkey) = oracle_pubkey_op {
                request = request.accounts(AccountMeta::new_readonly(*oracle_pubkey, false));
            } else {
                // TODO: Inefficient, we could remove the token from the list but this should not happen anyway in the program
                request = request.accounts(AccountMeta::new_readonly(Pubkey::default(), false));
                warn!(
                    token_idx,
                    "Refresh price of a token which has an undefined oracle mapping."
                )
            }
        }

        let tokens = tokens.to_vec();

        let tx = request
            .args(instruction::RefreshPriceList { tokens })
            .send()?;

        info!(signature = %tx, "Prices refreshed successfully");

        Ok(())
    }
}
