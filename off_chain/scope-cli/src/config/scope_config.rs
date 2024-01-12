use std::{fs::File, io::BufReader, path::Path};

use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use nohash_hasher::IntMap;
use scope::oracles::OracleType;
use serde::{Deserialize, Serialize};

use super::{token_config::TokenConfig, utils::serde_int_map};

/// Format of storage of Scope configuration
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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
        let config: ScopeConfig = serde_json::from_reader(buf_reader)?;
        for (id, token) in config.tokens.iter() {
            if token.oracle_type == OracleType::ScopeTwap1h {
                if token.twap_source.is_none() {
                    return Err(anyhow::anyhow!(
                        "Twap source not set for token {id}: {}",
                        token.label
                    ));
                }
                if token.oracle_mapping != Pubkey::default() {
                    return Err(anyhow::anyhow!(
                        "Token {id}: {} is of type Twap but oracle mapping is provided",
                        token.label
                    ));
                }
            } else {
                if token.twap_source.is_some() {
                    return Err(anyhow::anyhow!(
                        "Twap source set for token {id}: {} but token is not of type Twap",
                        token.label
                    ));
                }
                if token.oracle_mapping == Pubkey::default() {
                    return Err(anyhow::anyhow!(
                        "Token {id}: {} invalid oracle mapping provided",
                        token.label
                    ));
                }
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    // use crate::config::utils::remove_whitespace;
    use scope::anchor_lang::prelude::Pubkey;
    use scope::oracles::OracleType;

    use super::*;

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
                twap_enabled: false,
                twap_source: None,
            },
        );
        token_conf_list.tokens.insert(
            1,
            TokenConfig {
                label: "ETH/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw")
                    .unwrap(),
                oracle_type: OracleType::RaydiumAmmV3AtoB,
                twap_enabled: false,
                twap_source: None,
            },
        );
        token_conf_list.tokens.insert(
            13, // 13 to test actual holes
            TokenConfig {
                label: "STSOL/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("9LNYQZLJG5DAyeACCTzBFG6H3sDhehP5xtYLdhrZtQkA")
                    .unwrap(),
                oracle_type: OracleType::SwitchboardV2,
                twap_enabled: false,
                twap_source: None,
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
                twap_enabled: false,
                twap_source: None,
            },
        );
        token_conf_list.tokens.insert(
            41,
            TokenConfig {
                label: "kUSDHUSDCOrca/USD".to_string(),
                max_age: None,
                oracle_mapping: Pubkey::from_str("VF45TSF5WPAay9qy2zr1hPYgieBv7r17vYLRK6v1RmB")
                    .unwrap(),
                oracle_type: OracleType::KToken,
                twap_enabled: false,
                twap_source: None,
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
                "oracle_type": "RaydiumAmmV3AtoB",
                "oracle_mapping": "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw"
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
            },
            "41": {
                "label": "kUSDHUSDCOrca/USD",
                "oracle_type": "KToken",
                "oracle_mapping": "VF45TSF5WPAay9qy2zr1hPYgieBv7r17vYLRK6v1RmB"
            }
          }
          "#;

        let serialized: ScopeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(token_conf_list, serialized);

        //TODO: Nit does not work because order is not preserved but it does not cause any issue. To investigate.
        //let deserialized = serde_json::to_string(&token_conf_list).unwrap();
        //assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }
}
