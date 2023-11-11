use anchor_lang::prelude::Pubkey;
use async_recursion::async_recursion;
use scope::Price;

use super::types::{OracleConf, TestContext};
use crate::common::types::{ScopeFeedDefinition, TestOracleType};

mod jupiter_lp;
#[cfg(feature = "yvaults")]
mod ktoken;
mod pyth;
mod switchboard_v2;

#[async_recursion] // kTokens recursively create underlying token mappings
pub async fn set_price(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    conf: &OracleConf,
    price: &Price,
) {
    let clock = ctx.get_clock().await;
    let PriceSourceAccounts {
        oracle_data,
        owner,
        additional_accs,
    } = match conf.price_type {
        TestOracleType::Pyth => sp(pyth::get_account_data_for_price(price, &clock), pyth::id()),
        TestOracleType::SwitchboardV1 => {
            panic!("SwitchboardV1 oracle type is not available in tests")
        }
        TestOracleType::SwitchboardV2 => sp(
            switchboard_v2::get_account_data_for_price(price, &clock),
            switchboard_v2::id(),
        ),
        #[cfg(feature = "yvaults")]
        TestOracleType::KToken(dex) => {
            use crate::common::mock_oracles::ktoken;
            ktoken::get_ktoken_price_accounts(ctx, feed, dex, price, &clock).await
        }
        #[cfg(not(feature = "yvaults"))]
        TestOracleType::KToken(_) => {
            panic!("yvaults feature is not enabled, KToken oracle type is not available")
        }
        TestOracleType::PythEMA => sp(pyth::get_account_data_for_price(price, &clock), pyth::id()),
        TestOracleType::CToken => {
            panic!("CToken oracle type is not available in tests")
        }
        TestOracleType::SplStake => {
            panic!("SplStake oracle type is not available in tests")
        }
        TestOracleType::JupiterLP => jupiter_lp::get_jlp_price_accounts(&conf.pubkey, price),
        TestOracleType::ScopeTwap(_) => {
            // This is a derived oracle, we don't override it
            None
        }
        TestOracleType::DeprecatedPlaceholder => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
    };
    additional_accs.into_iter().for_each(|a| {
        let AdditionalAccount {
            address,
            owner,
            data,
        } = a;
        ctx.set_account(&address, data, &owner)
    });
    ctx.set_account(&conf.pubkey, oracle_data, &owner)
}

struct PriceSourceAccounts {
    oracle_data: Vec<u8>,
    owner: Pubkey,
    additional_accs: Vec<AdditionalAccount>,
}

struct AdditionalAccount {
    address: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
}

/// Helper to create a simple oracle account
fn sp(data: Vec<u8>, owner: Pubkey) -> PriceSourceAccounts {
    PriceSourceAccounts {
        oracle_data: data,
        owner,
        additional_accs: vec![],
    }
}

/// Helper to create an additional account
fn add_acc(address: Pubkey, owner: Pubkey, data: Vec<u8>) -> AdditionalAccount {
    AdditionalAccount {
        address,
        owner,
        data,
    }
}
