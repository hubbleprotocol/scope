use scope::{oracles::OracleType, Price};

use super::types::{OracleConf, TestContext};

pub mod pyth;
pub mod switchboard_v2;

pub async fn set_price(test_context: &mut TestContext, conf: &OracleConf, price: &Price) {
    let clock = test_context.get_clock().await;
    let (oracle_data, owner) = match conf.price_type {
        OracleType::Pyth => (pyth::get_account_data_for_price(price, &clock), pyth::id()),
        OracleType::SwitchboardV2 => (
            switchboard_v2::get_account_data_for_price(price, &clock),
            switchboard_v2::id(),
        ),
        _ => todo!("Implement other oracle types"),
    };
    test_context.set_account(&conf.pubkey, oracle_data, &owner)
}
