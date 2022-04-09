use std::num::NonZeroU64;
use std::{fs::File, io::BufReader, path::Path};

use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use nohash_hasher::IntMap;
use serde::{Deserialize, Serialize};

use scope::utils::OracleType;

use super::utils::{serde_int_map, serde_string};

/// Format of storage of Scope configuration
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TokenConfList {
    /// Default mage age in number of slot
    pub default_max_age: u64,
    #[serde(flatten, deserialize_with = "serde_int_map::deserialize")]
    /// List of token (index in the accounts and configuration)
    pub tokens: IntMap<u64, TokenConf>,
}

/// Configuration of the tokens
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TokenConf {
    /// Name of the pair (used for display)
    /// eg. "SOL/USD"
    pub token_pair: String,
    /// Type of oracle providing the price.
    pub oracle_type: OracleType,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional specific token max age (in number of slot).
    pub max_age: Option<NonZeroU64>,
    /// Onchain account used as source for the exchange rate.
    #[serde(with = "serde_string")] // Use bs58 for serialization
    pub oracle_mapping: Pubkey,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn remove_whitespace(s: &str) -> String {
        s.split_whitespace().collect()
    }

    #[test]
    fn conf_de_ser() {
        let token_conf = TokenConf {
            token_pair: "SOL/USD".to_string(),
            max_age: None,
            oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                .unwrap(),
            oracle_type: OracleType::Pyth,
        };

        let json = r#"{
              "token_pair": "SOL/USD",
              "oracle_type": "Pyth",
              "oracle_mapping": "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
            }
            "#;

        let serialized: TokenConf = serde_json::from_str(json).unwrap();
        assert_eq!(token_conf, serialized);

        let deserialized = serde_json::to_string(&token_conf).unwrap();
        assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }

    #[test]
    fn conf_list_de_ser() {
        let mut token_conf_list = TokenConfList {
            default_max_age: 30,
            tokens: IntMap::default(),
        };
        token_conf_list.tokens.insert(
            0,
            TokenConf {
                token_pair: "SOL/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                    .unwrap(),
                oracle_type: OracleType::Pyth,
            },
        );
        token_conf_list.tokens.insert(
            1,
            TokenConf {
                token_pair: "ETH/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw")
                    .unwrap(),
                oracle_type: OracleType::Switchboard,
            },
        );
        token_conf_list.tokens.insert(
            4, // 4 to test actual holes
            TokenConf {
                token_pair: "UST/stSolUST".to_string(),
                max_age: NonZeroU64::new(800),
                oracle_mapping: Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J")
                    .unwrap(),
                oracle_type: OracleType::YiToken,
            },
        );

        let json = r#"{
            "default_max_age": 30,
            "0": {
              "token_pair": "SOL/USD",
              "oracle_type": "Pyth",
              "oracle_mapping": "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
            },
            "1": {
              "token_pair": "ETH/USD",
              "oracle_type": "Switchboard",
              "oracle_mapping": "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw"
            },
            "4": {
              "token_pair": "UST/stSolUST",
              "oracle_type": "YiToken",
              "max_age": 800,
              "oracle_mapping": "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J"
            }
          }
          "#;

        let serialized: TokenConfList = dbg!(serde_json::from_str(json)).unwrap();
        assert_eq!(token_conf_list, serialized);

        let deserialized = serde_json::to_string(&token_conf_list).unwrap();
        assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }
}
