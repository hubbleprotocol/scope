use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use nohash_hasher::IntMap;
use serde::{Deserialize, Serialize};

use super::token_config::TokenConfig;
use super::utils::serde_int_map;

/// Format of storage of Scope configuration
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ScopeConfig {
    /// Default mage age in number of slot
    pub default_max_age: u64,
    #[serde(flatten, deserialize_with = "serde_int_map::deserialize")]
    /// List of token (index in the accounts and configuration)
    pub tokens: TokenList,
}

pub type TokenList = IntMap<u16, TokenConfig>;

impl ScopeConfig {
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
    use scope::anchor_lang::prelude::Pubkey;
    use scope::utils::OracleType;
    use std::num::NonZeroU64;
    use std::str::FromStr;

    #[test]
    fn conf_list_de_ser() {
        let mut token_conf_list = ScopeConfig {
            default_max_age: 30,
            tokens: IntMap::default(),
        };
        token_conf_list.tokens.insert(
            0,
            TokenConfig {
                label: "SOL/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                    .unwrap(),
                oracle_type: OracleType::Pyth,
            },
        );
        token_conf_list.tokens.insert(
            1,
            TokenConfig {
                label: "ETH/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw")
                    .unwrap(),
                oracle_type: OracleType::SwitchboardV1,
            },
        );
        token_conf_list.tokens.insert(
            4, // 4 to test actual holes
            TokenConfig {
                label: "UST/stSolUST".to_string(),
                max_age: NonZeroU64::new(800),
                oracle_mapping: Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J")
                    .unwrap(),
                oracle_type: OracleType::YiToken,
            },
        );
        token_conf_list.tokens.insert(
            13,
            TokenConfig {
                label: "STSOL/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("9LNYQZLJG5DAyeACCTzBFG6H3sDhehP5xtYLdhrZtQkA")
                    .unwrap(),
                oracle_type: OracleType::SwitchboardV2,
            },
        );
        token_conf_list.tokens.insert(
            14,
            TokenConfig {
                label: "cSOL/SOL".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("9LNYQZLJG5DAyeACCTzBFG6H3sDhehP5xtYLdhrZtQkA")
                    .unwrap(),
                oracle_type: OracleType::CToken,
            },
        );

        let json = r#"{
            "default_max_age": 30,
            "0": {
                "label": "SOL/USD",
                "oracle_type": "Pyth",
                "oracle_mapping": "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
            },
            "1": {
                "label": "ETH/USD",
                "oracle_type": "SwitchboardV1",
                "oracle_mapping": "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw"
            },
            "4": {
                "label": "UST/stSolUST",
                "oracle_type": "YiToken",
                "max_age": 800,
                "oracle_mapping": "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J"
            },
            "13": {
                "label": "STSOL/USD",
                "oracle_type": "SwitchboardV2",
                "oracle_mapping": "9LNYQZLJG5DAyeACCTzBFG6H3sDhehP5xtYLdhrZtQkA"
            },
            "14": {
                "label": "cSOL/SOL",
                "oracle_type": "CToken",
                "oracle_mapping": "9LNYQZLJG5DAyeACCTzBFG6H3sDhehP5xtYLdhrZtQkA"
            }
          }
          "#;

        let serialized: ScopeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(token_conf_list, serialized);

        let deserialized = serde_json::to_string(&token_conf_list).unwrap();
        assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }
}
