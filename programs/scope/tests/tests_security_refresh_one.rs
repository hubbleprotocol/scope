mod common;

use anchor_lang::{
    prelude::{Clock, Pubkey},
    InstructionData, ToAccountMetas,
};
use common::*;
use scope::{oracles::OracleType, OraclePrices, Price, ScopeError};
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;
use solana_program::{instruction::Instruction, sysvar::SysvarId};
use solana_program_test::tokio;
use solana_sdk::pubkey;
use solana_sdk::signer::Signer;
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

// - [x] Wrong oracle mapping
// - [x] Wrong oracle account (copy)
// - [x] Wrong oracle account (mixing indexes)
// - [x] Wrong sysvar instruction account
// - [x] Instruction executed in CPI
// - [x] Instruction preceded by non ComputeBudget instruction

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
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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

// - [ ] Wrong sysvar instruction account
#[tokio::test]
async fn test_wrong_sysvar_instructions() {
    let (mut ctx, feed) =
        fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Create the fake sysvar
    let wrong_sysvar_account = Pubkey::new_unique();

    ctx.set_account(&wrong_sysvar_account, vec![0; 100], &Pubkey::new_unique());

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: wrong_sysvar_account,
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

    let res = ctx.send_transaction_with_bot(&[ix]).await;
    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintAddress);
}

// - [ ] Instruction executed in CPI
#[tokio::test]
async fn test_refresh_through_cpi() {
    let (mut ctx, feed) =
        fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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

    let res = ctx.send_transaction_through_cpi(&[ix]).await;
    assert_eq!(map_scope_error(res), ScopeError::RefreshInCPI);
}

// - [ ] Instruction preceded by non ComputeBudget instruction
#[tokio::test]
async fn test_refresh_with_unexpected_ix() {
    let (mut ctx, feed) =
        fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE]).await;

    // Change price
    mock_oracles::set_price(&mut ctx, &TEST_PYTH_ORACLE, &Price { value: 1, exp: 6 }).await;

    // Random update mapping as extra ix
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

    let extra_ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    // Refresh
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
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

    let res = ctx.send_transaction(&[extra_ix, ix]).await;
    assert_eq!(map_scope_error(res), ScopeError::RefreshWithUnexpectedIxs);
}
