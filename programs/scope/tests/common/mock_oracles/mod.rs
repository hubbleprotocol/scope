use async_recursion::async_recursion;
use scope::Price;
use solana_program::pubkey::Pubkey;

use super::types::{OracleConf, TestContext};
use crate::common::types::{ScopeFeedDefinition, TestOracleType};

#[cfg(feature = "yvaults")]
mod ktoken;
pub mod pyth;
pub mod switchboard_v2;

#[async_recursion] // kTokens recursively create underlying token mappings
pub async fn set_price(
    ctx: &mut TestContext,
    _feed: &ScopeFeedDefinition,
    conf: &OracleConf,
    price: &Price,
) {
    let clock = ctx.get_clock().await;
    let (oracle_data, owner, additional_accs): (Vec<u8>, Pubkey, Vec<(Pubkey, Pubkey, Vec<u8>)>) =
        match conf.price_type {
            TestOracleType::Pyth => (
                pyth::get_account_data_for_price(price, &clock),
                pyth::id(),
                vec![],
            ),
            TestOracleType::SwitchboardV2 => (
                switchboard_v2::get_account_data_for_price(price, &clock),
                switchboard_v2::id(),
                vec![],
            ),
            #[cfg(feature = "yvaults")]
            TestOracleType::KToken(dex) => {
                use crate::common::mock_oracles::ktoken;
                ktoken::get_ktoken_price_accounts(ctx, _feed, dex, price, &clock).await
            }
            _ => todo!("Implement other oracle types"),
        };
    additional_accs
        .iter()
        .for_each(|(address, owner, data)| ctx.set_account(address, data.clone(), &owner));
    ctx.set_account(&conf.pubkey, oracle_data, &owner)
}
