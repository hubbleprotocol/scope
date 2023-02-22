mod common;

use anchor_lang::{
    prelude::{Clock, Pubkey},
    InstructionData, ToAccountMetas,
};
use common::*;
use scope::{oracles::OracleType, OraclePrices, Price, ScopeError};
use solana_program::{instruction::Instruction, sysvar::SysvarId};
use solana_program_test::tokio;
use solana_sdk::pubkey;
use types::*;

use crate::{
    common::utils::AnchorErrorCode,
    utils::{map_anchor_error, map_scope_error},
};

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: OracleType::Pyth,
};

const TEST_PYTH2_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePyth2PriceAccount1111111111111111111111"),
    token: 1,
    price_type: OracleType::Pyth,
};

// - [ ] Wrong oracle mapping
// - [ ] Wrong oracle account (copy)
// - [ ] Wrong oracle account (mixing indexes)

#[tokio::test]
async fn test_working_refresh_one() {
    let (mut ctx, feed) =
        fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        price_info: TEST_PYTH_ORACLE.pubkey,
    };

    let args = scope::instruction::RefreshOnePrice {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    // Check price
    let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    assert_eq!(data.prices[0].price.value, 1);
    assert_eq!(data.prices[0].price.exp, 6);
}

// - [ ] Wrong oracle mapping
#[tokio::test]
async fn test_wrong_oracle_mapping() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Create a fake mapping account
    let fake_mapping_pk = Pubkey::new_unique();
    ctx.clone_account(&feed.mapping, &fake_mapping_pk).await;

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: fake_mapping_pk,
        clock: Clock::id(),
        price_info: TEST_PYTH_ORACLE.pubkey,
    };

    let args = scope::instruction::RefreshOnePrice {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction_with_bot(&[ix]).await),
        AnchorErrorCode::ConstraintHasOne,
    );
}

// - [ ] Wrong oracle account (copy)
#[tokio::test]
async fn test_wrong_oracle_account_with_copy() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Create a fake mapping account
    let fake_price_account = Pubkey::new_unique();
    ctx.clone_account(&TEST_PYTH_ORACLE.pubkey, &fake_price_account)
        .await;

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        price_info: fake_price_account,
    };

    let args = scope::instruction::RefreshOnePrice {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_scope_error(ctx.send_transaction_with_bot(&[ix]).await),
        ScopeError::UnexpectedAccount,
    );
}

// - [ ] Wrong oracle account (mixing indexes)
#[tokio::test]
async fn test_wrong_index_oracle_account() {
    let (mut ctx, feed) =
        fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        price_info: TEST_PYTH_ORACLE.pubkey,
    };

    let args = scope::instruction::RefreshOnePrice { token: 1 };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_scope_error(ctx.send_transaction_with_bot(&[ix]).await),
        ScopeError::UnexpectedAccount,
    );
}
