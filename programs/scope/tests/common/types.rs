use anchor_lang::prelude::{Pubkey, Rent};
use scope::oracles::OracleType;
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::signature::Keypair;
use thiserror::Error;
use yvaults::utils::types::DEX;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct OracleConf {
    pub token: usize,
    pub price_type: TestOracleType,
    pub pubkey: Pubkey,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TestOracleType {
    Pyth,
    SwitchboardV1,
    SwitchboardV2,
    /// Deprecated (formerly YiToken)
    // Do not remove - breaks the typescript idl codegen
    DeprecatedPlaceholder,
    /// Solend tokens
    CToken,
    /// SPL Stake Pool token (like scnSol)
    SplStake,
    /// KTokens from Kamino
    KToken(DEX),
    /// Pyth Exponentially-Weighted Moving Average
    PythEMA,
}

impl TestOracleType {
    pub fn to_u8(self) -> u8 {
        let oracle_type: OracleType = self.into();
        oracle_type.into()
    }
}

impl Into<OracleType> for TestOracleType {
    fn into(self) -> OracleType {
        match self {
            TestOracleType::Pyth => OracleType::Pyth,
            TestOracleType::SwitchboardV1 => OracleType::SwitchboardV1,
            TestOracleType::SwitchboardV2 => OracleType::SwitchboardV2,
            TestOracleType::CToken => OracleType::CToken,
            TestOracleType::SplStake => OracleType::SplStake,
            TestOracleType::KToken(_) => OracleType::KToken,
            TestOracleType::PythEMA => OracleType::PythEMA,
            TestOracleType::DeprecatedPlaceholder => {
                panic!("DeprecatedPlaceholder is not a valid oracle type")
            }
        }
    }
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
