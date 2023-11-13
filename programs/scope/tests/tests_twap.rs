mod common;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use common::*;
use scope::{OracleMappings, OraclePrices, Price, ScopeError};
use solana_program::{
    instruction::Instruction, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use solana_program_test::tokio;
use solana_sdk::{pubkey, signer::Signer};
use types::*;

use crate::{
    common::utils::AnchorErrorCode,
    utils::{map_anchor_error, map_scope_error},
};

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: TestOracleType::Pyth,
    twap_enabled: true,
    twap_source: None,
};

const TEST_TWAP: OracleConf = OracleConf {
    pubkey: pubkey!("HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ"),
    token: 1,
    price_type: TestOracleType::ScopeTwap(1),
    twap_enabled: false,
    twap_source: Some(0),
};

#[tokio::test]
async fn test_update_mapping() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;

    // Set the mapping for first price account
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: Some(TEST_PYTH_ORACLE.pubkey),
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.to_u8(),
        twap_enabled: TEST_PYTH_ORACLE.twap_enabled,
        twap_source: TEST_PYTH_ORACLE.twap_source.unwrap_or(u16::MAX),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();

    // Set the mapping for the TWAP
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: Some(TEST_TWAP.pubkey),
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_TWAP.token.try_into().unwrap(),
        price_type: TEST_TWAP.price_type.to_u8(),
        twap_enabled: TEST_TWAP.twap_enabled,
        twap_source: TEST_TWAP.twap_source.unwrap_or(u16::MAX),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();

    let oracle_mappings: OracleMappings = ctx.get_zero_copy_account(&feed.mapping).await.unwrap();

    assert_eq!(
        oracle_mappings.price_info_accounts[TEST_PYTH_ORACLE.token as usize],
        TEST_PYTH_ORACLE.pubkey
    );
    assert_eq!(
        oracle_mappings.price_info_accounts[TEST_TWAP.token as usize],
        Pubkey::default()
    );
    assert_eq!(
        oracle_mappings.price_types[TEST_PYTH_ORACLE.token as usize],
        TEST_PYTH_ORACLE.price_type.to_u8()
    );
    assert_eq!(
        oracle_mappings.price_types[TEST_TWAP.token as usize],
        TEST_TWAP.price_type.to_u8()
    );
    assert_eq!(
        oracle_mappings.twap_enabled[TEST_PYTH_ORACLE.token as usize],
        u8::from(TEST_PYTH_ORACLE.twap_enabled)
    );
    assert_eq!(
        oracle_mappings.twap_enabled[TEST_TWAP.token as usize],
        u8::from(TEST_TWAP.twap_enabled)
    );
    assert_eq!(
        oracle_mappings.twap_source[TEST_PYTH_ORACLE.token as usize],
        u16::MAX
    );
    assert_eq!(
        oracle_mappings.twap_source[TEST_TWAP.token as usize],
        TEST_TWAP.twap_source.unwrap_or(0)
    );
}
