use async_recursion::async_recursion;
use scope::{oracles::OracleType, Price};
use solana_program::pubkey::Pubkey;

use super::types::{OracleConf, TestContext};
use crate::common::{mock_oracles::ktoken::get_ktoken_price_accounts, types::ScopeFeedDefinition};

mod ktoken;
pub mod pyth;
pub mod switchboard_v2;

#[async_recursion] // kTokens recursively create underlying token mappings
pub async fn set_price(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    conf: &OracleConf,
    price: &Price,
) {
    let clock = ctx.get_clock().await;
    let (oracle_data, owner, additional_accs): (Vec<u8>, Pubkey, Vec<(Pubkey, Pubkey, Vec<u8>)>) =
        match conf.price_type {
            OracleType::Pyth => (
                pyth::get_account_data_for_price(price, &clock),
                pyth::id(),
                vec![],
            ),
            OracleType::SwitchboardV2 => (
                switchboard_v2::get_account_data_for_price(price, &clock),
                switchboard_v2::id(),
                vec![],
            ),
            OracleType::KToken => get_ktoken_price_accounts(ctx, feed, price, &clock).await,
            _ => todo!("Implement other oracle types"),
        };
    additional_accs
        .iter()
        .for_each(|(address, owner, data)| ctx.set_account(address, data.clone(), &owner));
    ctx.set_account(&conf.pubkey, oracle_data, &owner)
}
