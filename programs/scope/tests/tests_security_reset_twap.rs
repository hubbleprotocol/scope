use crate::{client::reset_twap, common::fixtures::setup_mapping_for_token_with_twap};
use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use common::*;
use scope::{OracleMappings, OraclePrices, OracleTwaps, Price, TokenMetadatas};
use solana_program::{
    instruction::Instruction, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use solana_program_test::tokio;
use solana_sdk::{pubkey, signature::Keypair, signer::Signer};
use types::*;

use crate::{common::utils::AnchorErrorCode, utils::map_anchor_error};

mod common;

const TEST_TWAP: OracleConf = OracleConf {
    pubkey: pubkey!("HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ"),
    token: 1,
    price_type: TestOracleType::ScopeTwap(1),
    twap_enabled: false,
    twap_source: Some(0),
};

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: TestOracleType::Pyth,
    twap_enabled: true,
    twap_source: None,
};

#[cfg(feature = "yvaults")]
const TEST_ORACLE_CONF: [OracleConf; 1] = [TEST_PYTH_ORACLE];

// - [x] wrong admin
// - [x] wrong oracle_prices
// - [x] wrong configuration
// - [x] wrong oracle_twaps
// - [x] rong sysvar account
// - [x] second user injects different config

#[tokio::test]
async fn test_working_reset_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // reset the TWAP
    let reset_twap_ix = reset_twap(&ctx.admin.pubkey(), &feed, TEST_PYTH_ORACLE);
    ctx.send_transaction(&[reset_twap_ix]).await.unwrap();
}

// wrong admin
#[tokio::test]
async fn test_wrong_admin() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // New (bad) admin
    let fake_admin = Keypair::new();
    ctx.clone_account(&ctx.admin.pubkey(), &fake_admin.pubkey())
        .await;

    // reset the TWAP
    let reset_twap_ix = reset_twap(&fake_admin.pubkey(), &feed, TEST_PYTH_ORACLE);

    assert_eq!(
        map_anchor_error(
            ctx.send_transaction_with_payer(&[reset_twap_ix], &fake_admin)
                .await
        ),
        AnchorErrorCode::ConstraintHasOne,
    );
}

// wrong oracle_prices
#[tokio::test]
async fn test_wrong_oracle_prices() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // Create a fake mapping account
    let fake_price_account = Pubkey::new_unique();
    ctx.clone_account(&feed.prices, &fake_price_account).await;

    let accounts = scope::accounts::ResetTwap {
        admin: ctx.admin.pubkey(),
        oracle_prices: fake_price_account,
        configuration: feed.conf,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    };

    let args = scope::instruction::ResetTwap {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    let res = ctx.send_transaction(&[ix]).await;
    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
}

// wrong configuration
#[tokio::test]
async fn test_wrong_configuration() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // Create a fake mapping account
    let fake_configuration = Pubkey::new_unique();
    ctx.clone_account(&feed.conf, &fake_configuration).await;

    let accounts = scope::accounts::ResetTwap {
        admin: ctx.admin.pubkey(),
        oracle_prices: feed.prices,
        configuration: fake_configuration,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    };

    let args = scope::instruction::ResetTwap {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    let res = ctx.send_transaction(&[ix]).await;
    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintSeeds);
}

// wrong twaps account
#[tokio::test]
async fn test_wrong_twaps() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // Create a fake twap account
    let fake_twaps = Pubkey::new_unique();
    ctx.clone_account(&feed.twaps, &fake_twaps).await;

    let accounts = scope::accounts::ResetTwap {
        admin: ctx.admin.pubkey(),
        oracle_prices: feed.prices,
        configuration: feed.conf,
        oracle_twaps: fake_twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    };

    let args = scope::instruction::ResetTwap {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    let res = ctx.send_transaction(&[ix]).await;
    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
}

// wrong sysvar account
#[tokio::test]
async fn test_wrong_sysvar_account() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // Create the fake sysvar
    let wrong_sysvar_account = Pubkey::new_unique();

    let accounts = scope::accounts::ResetTwap {
        admin: ctx.admin.pubkey(),
        oracle_prices: feed.prices,
        configuration: feed.conf,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: wrong_sysvar_account,
    };

    let args = scope::instruction::ResetTwap {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction(&[ix]).await),
        AnchorErrorCode::ConstraintAddress,
    );
}

// second user injects different config
#[tokio::test]
async fn test_second_user_injects_config() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    // create a second config and feed
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    ctx.create_zeroed_account(
        &zero_copy_accounts.mapping.pubkey(),
        std::mem::size_of::<OracleMappings>() + 8,
        &scope::ID,
    )
    .await;
    ctx.create_zeroed_account(
        &zero_copy_accounts.prices.pubkey(),
        std::mem::size_of::<OraclePrices>() + 8,
        &scope::ID,
    )
    .await;
    ctx.create_zeroed_account(
        &zero_copy_accounts.token_metadatas.pubkey(),
        std::mem::size_of::<TokenMetadatas>() + 8,
        &scope::ID,
    )
    .await;
    ctx.create_zeroed_account(
        &zero_copy_accounts.oracle_twaps.pubkey(),
        std::mem::size_of::<OracleTwaps>() + 8,
        &scope::ID,
    )
    .await;

    let second_feed_name = "test_feed_name";
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", second_feed_name.as_bytes()], &scope::id());
    let accounts = scope::accounts::Initialize {
        admin: ctx.bot.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
        token_metadatas: zero_copy_accounts.token_metadatas.pubkey(),
        oracle_twaps: zero_copy_accounts.oracle_twaps.pubkey(),
    };

    let args = scope::instruction::Initialize {
        feed_name: second_feed_name.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    let second_feed: ScopeFeedDefinition = types::ScopeFeedDefinition {
        feed_name: second_feed_name.to_string(),
        conf: configuration_acc,
        mapping: zero_copy_accounts.mapping.pubkey(),
        prices: zero_copy_accounts.prices.pubkey(),
        twaps: zero_copy_accounts.oracle_twaps.pubkey(),
    };

    // verify that the second config can't reset the TWAP
    let accounts = scope::accounts::ResetTwap {
        admin: ctx.bot.pubkey(),
        oracle_prices: feed.prices,
        configuration: second_feed.conf,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    };

    let args = scope::instruction::ResetTwap {
        token: TEST_PYTH_ORACLE.token.try_into().unwrap(),
        feed_name: second_feed.feed_name.clone(),
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

    // verify the first config can still reset the TWAP
    let reset_twap_ix = reset_twap(&ctx.admin.pubkey(), &feed, TEST_PYTH_ORACLE);
    ctx.send_transaction(&[reset_twap_ix]).await.unwrap();
}
