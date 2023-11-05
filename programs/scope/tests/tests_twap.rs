mod common;

use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use scope::{OraclePrices, OracleTwaps, Price, UpdateTokenMetadataMode, TWAP_NUM_OBS};
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
    assert_eq!(twaps.twap_buffers[idx].next_index, 0);
    assert_eq!(twaps.twap_buffers[idx].unix_timestamps, [0; TWAP_NUM_OBS]);
    assert_eq!(twaps.twap_buffers[idx].values, [zero; TWAP_NUM_OBS]);
}

#[tokio::test]
async fn test_refresh_one_with_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_PYTH_ORACLE]).await;

    let (idx, zero, admin) = (
        TEST_PYTH_ORACLE.token,
        Price::default(),
        Keypair::from_bytes(&ctx.admin.to_bytes()).unwrap(),
    );

    // Change price
    let px = Price { value: 1, exp: 6 };
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &px).await;

    // Set Twap enabled
    let ix = update_token_metadata(
        &mut ctx,
        &feed,
        TEST_PYTH_ORACLE,
        UpdateTokenMetadataMode::TwapEnabled,
        vec![1],
    );
    ctx.send_transaction_with_payer(&[ix], &admin)
        .await
        .unwrap();

    // Refresh
    let ts = ctx.get_clock().await.unix_timestamp;
    let ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    // Check price
    let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    assert_eq!(data.prices[idx].price.value, 1);
    assert_eq!(data.prices[idx].price.exp, 6);

    let twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
    assert_eq!(twaps.twap_buffers[idx].unix_timestamps[0], ts);
    assert_eq!(twaps.twap_buffers[idx].values[0], px);
    assert_eq!(twaps.twap_buffers[idx].next_index, 1);
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
