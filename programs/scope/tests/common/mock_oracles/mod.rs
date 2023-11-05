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
    let res: Option<(Vec<u8>, Pubkey, Vec<(Pubkey, Pubkey, Vec<u8>)>)> = match conf.price_type {
        TestOracleType::Pyth => Some((
            pyth::get_account_data_for_price(price, &clock),
            pyth::id(),
            vec![],
        )),
        TestOracleType::SwitchboardV2 => Some((
            switchboard_v2::get_account_data_for_price(price, &clock),
            switchboard_v2::id(),
            vec![],
        )),
        #[cfg(feature = "yvaults")]
        TestOracleType::KToken(dex) => {
            use crate::common::mock_oracles::ktoken;
            Some(ktoken::get_ktoken_price_accounts(ctx, _feed, dex, price, &clock).await)
        }
        TestOracleType::ScopeTwap(_) => {
            // This is a derived oracle, we don't override it
            None
        }
        _ => todo!("Implement other oracle types"),
    };

    if let Some((oracle_data, owner, additional_accs)) = res {
        additional_accs
            .iter()
            .for_each(|(address, owner, data)| ctx.set_account(address, data.clone(), &owner));
        ctx.set_account(&conf.pubkey, oracle_data, &owner)
    }
}
