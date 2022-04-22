use std::num::NonZeroU64;

use super::utils::serde_string;
use scope::utils::OracleType;
use scope::Pubkey;
use serde::{Deserialize, Serialize};

/// Configuration of the tokens
#[derive(Debug, Serialize, Deserialize, PartialEq)]
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
    #[serde(with = "serde_string")] // Use bs58 for serialization
    pub oracle_mapping: Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::utils::remove_whitespace;
    use std::str::FromStr;

    #[test]
    fn conf_de_ser() {
        let token_conf = TokenConfig {
            label: "SOL/USD".to_string(),
            max_age: None,
            oracle_mapping: Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
                .unwrap(),
            oracle_type: OracleType::Pyth,
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
