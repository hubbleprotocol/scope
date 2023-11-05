mod common;

use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use scope::{
    OraclePrices, OracleTwaps, Price, UpdateTokenMetadataMode, TWAP_INTERVAL_SECONDS, TWAP_NUM_OBS,
};
use solana_program::{
    instruction::Instruction, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use solana_program_test::tokio;
use solana_sdk::{pubkey, signature::Keypair, signer::Signer};
use types::*;

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: TestOracleType::Pyth,
};

#[tokio::test]
async fn test_refresh_one_no_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    let idx = TEST_PYTH_ORACLE.token;
    let zero = Price::default();

    // Change price
    mock_oracles::set_price(
        &mut ctx,
        &feed,
        &TEST_PYTH_ORACLE,
        &Price { value: 1, exp: 6 },
    )
    .await;

    // Refresh
    let ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    // Check price
    let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    assert_eq!(data.prices[idx].price.value, 1);
    assert_eq!(data.prices[idx].price.exp, 6);

    let twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
    assert_eq!(twaps.twap_buffers[idx].curr_index, 0);
    assert_eq!(twaps.twap_buffers[idx].unix_timestamps, [0; TWAP_NUM_OBS]);
    assert_eq!(twaps.twap_buffers[idx].values, [zero; TWAP_NUM_OBS]);
}

#[tokio::test]
async fn test_refresh_one_with_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    let (idx, admin, oracle) = (
        TEST_PYTH_ORACLE.token,
        Keypair::from_bytes(&ctx.admin.to_bytes()).unwrap(),
        TEST_PYTH_ORACLE,
    );

    let px = Price { value: 1, exp: 6 };
    mock_oracles::set_price(&mut ctx, &feed, &oracle, &px).await;

    let ix = enable_twap_token_metadata(&mut ctx, &feed, oracle);
    ctx.send_transaction_with_payer(&[ix], &admin)
        .await
        .unwrap();

    let ts = ctx.get_clock().await.unix_timestamp;
    let ix = refresh_one_ix(&feed, oracle);
    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    // Check price
    let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    assert_eq!(data.prices[idx].price.value, 1);
    assert_eq!(data.prices[idx].price.exp, 6);

    let twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
    assert_eq!(twaps.twap_buffers[idx].unix_timestamps[0], ts);
    assert_eq!(twaps.twap_buffers[idx].values[0], px);
    assert_eq!(twaps.twap_buffers[idx].curr_index, 0);
}

#[tokio::test]
async fn test_refresh_one_with_twap_cranking_big_interval() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    let (idx, admin, oracle) = (
        TEST_PYTH_ORACLE.token,
        Keypair::from_bytes(&ctx.admin.to_bytes()).unwrap(),
        TEST_PYTH_ORACLE,
    );

    let mut px = Price { value: 100, exp: 8 };
    mock_oracles::set_price(&mut ctx, &feed, &oracle, &px).await;

    let ix = enable_twap_token_metadata(&mut ctx, &feed, oracle);
    ctx.send_transaction_with_payer(&[ix], &admin)
        .await
        .unwrap();

    let seconds_step_size = TWAP_INTERVAL_SECONDS as u64; // seconds
    let price_step_size = 5;
    let mut curr_twpidx = TWAP_NUM_OBS - 1;

    for _ in 0..100 {
        ctx.fast_forward_seconds(seconds_step_size).await;
        let ts = ctx.get_clock().await.unix_timestamp;

        curr_twpidx = (curr_twpidx + 1) % TWAP_NUM_OBS;

        // Update price
        px.value += price_step_size;
        mock_oracles::set_price(&mut ctx, &feed, &oracle, &px).await;

        let ix = refresh_one_ix(&feed, oracle);
        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(data.prices[idx].price.value, px.value);
        assert_eq!(data.prices[idx].price.exp, px.exp);

        let twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
        assert_eq!(twaps.twap_buffers[idx].unix_timestamps[curr_twpidx], ts);
        assert_eq!(twaps.twap_buffers[idx].values[curr_twpidx], px);
        assert_eq!(twaps.twap_buffers[idx].curr_index, curr_twpidx as u64);
    }
}

#[tokio::test]
async fn test_refresh_one_with_twap_cranking_small_interval() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    let (idx, admin, oracle) = (
        TEST_PYTH_ORACLE.token,
        Keypair::from_bytes(&ctx.admin.to_bytes()).unwrap(),
        TEST_PYTH_ORACLE,
    );

    let mut px = Price { value: 100, exp: 8 };
    mock_oracles::set_price(&mut ctx, &feed, &oracle, &px).await;

    let ix = enable_twap_token_metadata(&mut ctx, &feed, oracle);
    ctx.send_transaction_with_payer(&[ix], &admin)
        .await
        .unwrap();

    let seconds_step_size = (TWAP_INTERVAL_SECONDS / 2) as u64; // seconds
    let price_step_size = 5;
    let mut curr_twpidx = TWAP_NUM_OBS - 1;

    for i in 0..100 {
        ctx.fast_forward_seconds(seconds_step_size).await;
        let ts = ctx.get_clock().await.unix_timestamp;

        // Update price
        px.value += price_step_size;
        mock_oracles::set_price(&mut ctx, &feed, &oracle, &px).await;

        let ix = refresh_one_ix(&feed, oracle);
        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(data.prices[idx].price.value, px.value);
        assert_eq!(data.prices[idx].price.exp, px.exp);

        if i % 2 == 0 {
            curr_twpidx = (curr_twpidx + 1) % TWAP_NUM_OBS;
            let twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
            assert_eq!(twaps.twap_buffers[idx].unix_timestamps[curr_twpidx], ts);
            assert_eq!(twaps.twap_buffers[idx].values[curr_twpidx], px);
            assert_eq!(twaps.twap_buffers[idx].curr_index, curr_twpidx as u64);
        }
    }
}

fn refresh_one_ix(feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        price_info: oracle.pubkey,
        oracle_twaps: feed.twaps,
        tokens_metadata: feed.metadatas,
    };

    let args = scope::instruction::RefreshOnePrice {
        token: oracle.token.try_into().unwrap(),
    };

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}

fn enable_twap_token_metadata(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    oracle: OracleConf,
) -> Instruction {
    update_token_metadata(
        ctx,
        feed,
        oracle,
        UpdateTokenMetadataMode::TwapEnabled,
        vec![1],
    )
}

fn update_token_metadata(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    oracle: OracleConf,
    mode: UpdateTokenMetadataMode,
    value: Vec<u8>,
) -> Instruction {
    let accounts = scope::accounts::UpdateTokensMetadata {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        tokens_metadata: feed.metadatas,
    };

    let args = scope::instruction::UpdateTokenMetadata {
        index: oracle.token.try_into().unwrap(),
        mode: mode.to_u64(),
        feed_name: feed.feed_name.clone(),
        value,
    };

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}
