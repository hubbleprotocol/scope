use std::num::NonZeroU64;

use scope::{anchor_lang::prelude::Pubkey, oracles::OracleType};
use serde::{Deserialize, Serialize};

use super::utils::serde_string;

/// Configuration of the tokens
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenConfig {
    /// Name of the pair (used for display)
    /// eg. "SOL/USD"
    pub label: String,
    /// Type of oracle providing the price.
    pub oracle_type: OracleType,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional specific token max age (in number of slot).
    pub max_age: Option<NonZeroU64>,
    /// Onchain account used as source for the exchange rate.
    #[serde(
        with = "serde_string",
        skip_serializing_if = "pubkey_is_default",
        default
    )] // Use bs58 for serialization
    pub oracle_mapping: Pubkey,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub twap_enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub twap_source: Option<u16>,
}

pub fn pubkey_is_default(pk: &Pubkey) -> bool {
    *pk == Pubkey::default()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::config::utils::remove_whitespace;

    #[test]
    fn conf_de_ser() {
        let token_conf = TokenConfig {
            label: "SOL/USD".to_string(),
            max_age: None,
            oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                .unwrap(),
            oracle_type: OracleType::Pyth,
            twap_enabled: false,
            twap_source: None,
        };

        let json = r#"{
              "label": "SOL/USD",
              "oracle_type": "Pyth",
              "oracle_mapping": "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
            }
            "#;

        let serialized: TokenConfig = serde_json::from_str(json).unwrap();
        assert_eq!(token_conf, serialized);

        let deserialized = serde_json::to_string(&token_conf).unwrap();
        assert_eq!(remove_whitespace(&deserialized), remove_whitespace(json));
    }
}
