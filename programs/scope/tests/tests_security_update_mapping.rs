mod common;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use common::*;
use scope::{oracles::OracleType, Price};
use solana_program::instruction::Instruction;
use solana_program_test::tokio;
use solana_sdk::{pubkey, signature::Keypair, signer::Signer};
use types::*;

use crate::{common::utils::AnchorErrorCode, utils::map_anchor_error};

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: OracleType::Pyth,
};

// - [x] Wrong feed name
// - [x] Wrong config account
// - [x] Wrong mapping account
// - [x] Wrong admin

// Working update mapping
#[tokio::test]
async fn test_working_update_mapping() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;
    // Set the mapping
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: TEST_PYTH_ORACLE.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.into(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
}

// - [ ] Wrong feed name
#[tokio::test]
async fn test_wrong_feed_name() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;
    // Set the mapping
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: TEST_PYTH_ORACLE.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: "randomFeed".to_string(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.into(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction(&[ix]).await),
        AnchorErrorCode::ConstraintSeeds,
    );
}

// - [ ] Wrong config account
#[tokio::test]
async fn test_wrong_config_account() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;

    // Create a fake config account
    let fake_config_pk = Pubkey::new_unique();
    ctx.clone_account(&feed.conf, &fake_config_pk).await;

    // Set the mapping
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: fake_config_pk,
        oracle_mappings: feed.mapping,
        price_info: TEST_PYTH_ORACLE.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.into(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction(&[ix]).await),
        AnchorErrorCode::ConstraintSeeds,
    );
}

// - [ ] Wrong mapping account
#[tokio::test]
async fn test_wrong_mapping_account() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;

    // Create a fake mapping account
    let fake_mapping_pk = Pubkey::new_unique();
    ctx.clone_account(&feed.mapping, &fake_mapping_pk).await;

    // Set the mapping
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: fake_mapping_pk,
        price_info: TEST_PYTH_ORACLE.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.into(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction(&[ix]).await),
        AnchorErrorCode::ConstraintHasOne,
    );
}

// - [ ] Wrong admin
#[tokio::test]
async fn test_wrong_admin() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &Price::default()).await;

    // New (bad) admin
    let fake_admin = Keypair::new();
    ctx.clone_account(&ctx.admin.pubkey(), &fake_admin.pubkey())
        .await;

    // Set the mapping
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: fake_admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: TEST_PYTH_ORACLE.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        price_type: TEST_PYTH_ORACLE.price_type.into(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction_with_payer(&[ix], &fake_admin).await),
        AnchorErrorCode::ConstraintHasOne,
    );
}
