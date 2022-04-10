use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use nohash_hasher::IntMap;
use serde::{Deserialize, Serialize};

use super::token_config::TokenConfig;
use super::utils::serde_int_map;

/// Format of storage of Scope configuration
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TokensConfig {
    /// Default mage age in number of slot
    pub default_max_age: u64,
    #[serde(flatten, deserialize_with = "serde_int_map::deserialize")]
    /// List of token (index in the accounts and configuration)
    pub tokens: TokenList,
}

pub type TokenList = IntMap<u16, TokenConfig>;

impl TokensConfig {
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
    use crate::config::utils::remove_whitespace;
    use scope::utils::OracleType;
    use scope::Pubkey;
    use std::num::NonZeroU64;
    use std::str::FromStr;

    #[test]
    fn conf_list_de_ser() {
        let mut token_conf_list = TokensConfig {
            default_max_age: 30,
            tokens: IntMap::default(),
        };
        token_conf_list.tokens.insert(
            0,
            TokenConfig {
                token_pair: "SOL/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                    .unwrap(),
                oracle_type: OracleType::Pyth,
            },
        );
        token_conf_list.tokens.insert(
            1,
            TokenConfig {
                token_pair: "ETH/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw")
                    .unwrap(),
                oracle_type: OracleType::Switchboard,
            },
        );
        token_conf_list.tokens.insert(
            4, // 4 to test actual holes
            TokenConfig {
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

        let serialized: TokensConfig = dbg!(serde_json::from_str(json)).unwrap();
        assert_eq!(token_conf_list, serialized);

        let deserialized = serde_json::to_string(&token_conf_list).unwrap();
        assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }
}
