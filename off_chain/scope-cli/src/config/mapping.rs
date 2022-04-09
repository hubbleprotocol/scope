use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use scope::utils::PriceType;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// Format of storage of Scope configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TokenConfList {
    /// List of token (index in the accounts and configuration)
    pub tokens: Vec<(u64, TokenConf)>,
}

// Configuration of the tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenConf {
    /// Name of the pair (used for display)
    /// eg. "SOL/USD"
    pub token_pair: String,
    /// Onchain account used as source for the exchange rate.
    #[serde(with = "serde_string")] // Use bs58 for serialization
    pub oracle_mapping: Pubkey,
    pub price_type: PriceType,
}

impl TokenConfList {
    pub fn save_to_file(&self, file_path: impl AsRef<Path>) -> Result<()> {
        let file = File::create(file_path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    pub fn read_from_file(file_path: &impl AsRef<Path>) -> Result<Self> {
        let file = File::open(file_path)?;
        let buf_reader = BufReader::new(file);
        Ok(serde_json::from_reader(buf_reader)?)
    }
}

mod serde_string {
    use std::fmt::Display;
    use std::str::FromStr;

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}
