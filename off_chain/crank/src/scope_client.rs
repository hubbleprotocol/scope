use anchor_client::{solana_sdk::pubkey::Pubkey, Client, Program};

use solana_sdk::{clock::Clock, instruction::AccountMeta, system_program, sysvar::SysvarId};

use anyhow::{anyhow, bail, Result};

use scope::{accounts, instruction, OracleMappings, OraclePrices};
use tracing::{debug, error, info, warn};

use crate::config::{TokenConf, TokenConfList};
use crate::utils::find_data_address;

/// Max number of refresh per tx
const MAX_REFRESH_CHUNK_SIZE: usize = 28;

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
    pub fn new(client: Client, program_id: Pubkey, price_feed: String) -> Result<Self> {
        let program = client.program(program_id);

        let program_data_acc = find_data_address(&program_id);
        let (oracle_prices_acc, _) =
            Pubkey::find_program_address(&[b"prices", price_feed.as_bytes()], &program_id);
        let (oracle_mappings_acc, _) =
            Pubkey::find_program_address(&[b"mappings", price_feed.as_bytes()], &program_id);

        const EMPTY_STRING: String = String::new();

        Ok(Self {
            program,
            program_data_acc,
            oracle_prices_acc,
            oracle_mappings_acc,
            oracle_mappings: [None; scope::MAX_ENTRIES],
            token_pairs: [EMPTY_STRING; scope::MAX_ENTRIES],
        })
    }

    /// Initialize the program accounts and set the oracle mappings to the local version
    pub fn init_program(&self) -> Result<()> {
        self.ix_initialize()?;

        for (token, op_mapping) in self.oracle_mappings.iter().enumerate() {
            if let Some(mapping) = op_mapping {
                self.ix_update_mapping(mapping, token.try_into()?)?;
            }
        }

        Ok(())
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
        let to_refresh_idx: Vec<u8> = self
            .oracle_mappings
            .iter()
            .enumerate()
            .filter_map(|(idx, e)| {
                if e.is_some() {
                    Some(u8::try_from(idx).unwrap())
                } else {
                    None
                }
            })
            .collect();

        for (nb, chunk) in to_refresh_idx.chunks(MAX_REFRESH_CHUNK_SIZE).enumerate() {
            debug!("Refresh chunk {}:{:?}", nb, chunk);
            if let Err(e) = self.ix_refresh_price_list(chunk.to_vec()) {
                error!("Refresh of some prices failed {:?}", e);
            }
        }

        Ok(())
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

    #[tracing::instrument(skip(self))]
    fn ix_initialize(&self) -> Result<()> {
        let init_account = accounts::Initialize {
            oracle_prices: self.oracle_prices_acc,
            oracle_mappings: self.oracle_mappings_acc,
            admin: self.program.payer(),
            program: self.program.id(),
            program_data: self.program_data_acc,
            system_program: system_program::ID,
        };
        let request = self.program.request();

        request
            .accounts(init_account)
            .args(instruction::Initialize {
                feed_name: "first".to_string(),
            })
            .send()?;

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

        request
            .accounts(update_account)
            .args(instruction::UpdateMapping { token })
            .send()?;

        info!("Accounts updated successfully");

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
    fn ix_refresh_price_list(&self, tokens: Vec<u8>) -> Result<()> {
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
                    "Refresh price of token {} which has an undefined oracle mapping.",
                    token_idx
                )
            }
        }

        request
            .args(instruction::RefreshPriceList { tokens })
            .send()?;

        info!("Prices refreshed successfully");

        Ok(())
    }
}
