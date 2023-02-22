use anchor_lang::prelude::{Pubkey, Rent};
use scope::oracles::OracleType;
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::signature::Keypair;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestError {
    #[error("Insufficient collateral to cover debt")]
    CannotDeserialize,
    #[error("Wrong discriminator")]
    BadDiscriminator,
    #[error("Account not found")]
    AccountNotFound,
    #[error("Unknown Error")]
    UnknownError,
    #[error("Banks client error: {0:?}")]
    BanksClientError(#[from] BanksClientError),
}

#[derive(Debug, Clone, Copy)]
pub struct OracleConf {
    pub token: usize,
    pub price_type: OracleType,
    pub pubkey: Pubkey,
}

pub struct ScopeFeedDefinition {
    pub feed_name: String,
    pub conf: Pubkey,
    pub mapping: Pubkey,
    pub prices: Pubkey,
}

pub struct TestContext {
    pub admin: Keypair,
    pub bot: Keypair,
    pub context: ProgramTestContext,
    pub rent: Rent,
    pub token_confs: Vec<OracleConf>,
}

pub struct ScopeZeroCopyAccounts {
    pub mapping: Keypair,
    pub prices: Keypair,
}
