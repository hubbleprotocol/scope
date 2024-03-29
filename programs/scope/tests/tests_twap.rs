mod common;

use crate::common::client::refresh_simple_oracle_ix;
use crate::common::utils::map_scope_error;
use crate::{client::reset_twap, common::fixtures::setup_mapping_for_token_with_twap};
use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use decimal_wad::decimal::Decimal;
use scope::{
    assert_fuzzy_eq, EmaTwap, OracleMappings, OraclePrices, OracleTwaps, Price, ScopeError,
};
use solana_program::instruction::Instruction;
use solana_program_test::tokio;
use solana_sdk::{pubkey, signer::Signer};
use types::*;

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
        oracle_mappings.price_info_accounts[TEST_PYTH_ORACLE.token],
        TEST_PYTH_ORACLE.pubkey
    );
    assert_eq!(
        oracle_mappings.price_info_accounts[TEST_TWAP.token],
        TEST_TWAP.pubkey
    );
    assert_eq!(
        oracle_mappings.price_types[TEST_PYTH_ORACLE.token],
        TEST_PYTH_ORACLE.price_type.to_u8()
    );
    assert_eq!(
        oracle_mappings.price_types[TEST_TWAP.token],
        TEST_TWAP.price_type.to_u8()
    );
    assert_eq!(
        oracle_mappings.twap_enabled[TEST_PYTH_ORACLE.token],
        u8::from(TEST_PYTH_ORACLE.twap_enabled)
    );
    assert_eq!(
        oracle_mappings.twap_enabled[TEST_TWAP.token],
        u8::from(TEST_TWAP.twap_enabled)
    );
    assert_eq!(
        oracle_mappings.twap_source[TEST_PYTH_ORACLE.token],
        TEST_PYTH_ORACLE.twap_source.unwrap_or(u16::MAX)
    );
    assert_eq!(
        oracle_mappings.twap_source[TEST_TWAP.token],
        TEST_TWAP.twap_source.unwrap_or(u16::MAX)
    );
}

#[tokio::test]
async fn test_set_price_sets_initial_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account and set price
    let token_price = Price { value: 1, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    {
        let oracle_twaps: OracleTwaps = ctx.get_zero_copy_account(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[TEST_TWAP.token], EmaTwap::default());
        assert_eq!(
            oracle_twaps.twaps[TEST_PYTH_ORACLE.token].last_update_slot,
            1
        );
        assert_eq!(
            oracle_twaps.twaps[TEST_PYTH_ORACLE.token].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );
    }

    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        let res = ctx.send_transaction_with_bot(&[refresh_ix]).await;

        assert_eq!(
            map_scope_error(res),
            ScopeError::TwapNotEnoughSamplesInPeriod
        );
    }
}

#[tokio::test]
async fn test_2_prices_with_same_value_no_twap_change() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price { value: 100, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let last_update_unix_timestamp = {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );

        oracle_twaps.twaps[0].last_update_unix_timestamp
    };

    // Fast forward not enough time and refresh price with the same value
    ctx.fast_forward_seconds(10).await;
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // verify that the twap value and ts didn't change (too frequent updates)
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            1
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp
        );
    }

    // Fast forward time and refresh price with the same value
    ctx.fast_forward_seconds(90).await;
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // verify that the twap value didn't change but the twap date increased (update after right amount of time)
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            1
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp + 100
        );
    }
}

#[tokio::test]
async fn test_2_prices_with_same_value_no_twap_change_1h() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price { value: 100, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let last_update_unix_timestamp = {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );

        oracle_twaps.twaps[0].last_update_unix_timestamp
    };

    // Fast forward time and refresh price with the same value
    ctx.fast_forward_seconds(60 * 60 + 120).await; // 1h
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // verify that the twap value didn't change but the twap date increased
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            1
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp + 60 * 60 + 120
        );
    }

    // verify that this is not enough samples to use this twap
    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        let res = ctx.send_transaction_with_bot(&[refresh_ix]).await;

        assert_eq!(
            map_scope_error(res),
            ScopeError::TwapNotEnoughSamplesInPeriod
        );
    }
}

#[tokio::test]
async fn test_9_is_not_enough_twap_samples() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price { value: 100, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let last_update_unix_timestamp = {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );

        oracle_twaps.twaps[0].last_update_unix_timestamp
    };

    // Fast forward time and refresh price with the same value 9 times over 1h
    for _ in 0..9 {
        ctx.fast_forward_seconds(60 * 60 / 9).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    // verify that the twap value didn't change but the twap date increased
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            1
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp + 60 * 60
        );
    }

    // verify that this is not enough samples to use this twap
    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        let res = ctx.send_transaction_with_bot(&[refresh_ix]).await;

        assert_eq!(
            map_scope_error(res),
            ScopeError::TwapNotEnoughSamplesInPeriod
        );
    }
}

#[tokio::test]
async fn test_not_enough_twap_samples_at_period_start() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price { value: 100, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let last_update_unix_timestamp = {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );

        oracle_twaps.twaps[0].last_update_unix_timestamp
    };

    // Fast forward 20 minutes (begining of the period)
    ctx.fast_forward_seconds(60 * 20).await;

    // Fast forward time and refresh price with the same value 20 times over 40 minutes
    for _ in 0..20 {
        ctx.fast_forward_seconds(60 * 40 / 20).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    // verify that the twap value didn't change but the twap date increased
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            token_price_u128 / 1000000
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp + 60 * 60
        );
    }

    // verify that this is not enough samples to use this twap
    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        let res = ctx.send_transaction_with_bot(&[refresh_ix]).await;

        assert_eq!(
            map_scope_error(res),
            ScopeError::TwapNotEnoughSamplesInPeriod
        );
    }
}

#[tokio::test]
async fn test_not_enough_twap_samples_at_period_end() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price { value: 100, exp: 6 };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let last_update_unix_timestamp = {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );

        oracle_twaps.twaps[0].last_update_unix_timestamp
    };

    // Fast forward time and refresh price with the same value 20 times over 40 minutes
    for _ in 0..20 {
        ctx.fast_forward_seconds(60 * 39 / 20).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    // Fast forward 20 minutes (end of the period)
    ctx.fast_forward_seconds(60 * 21).await;

    // verify that the twap value didn't change but the twap date increased
    {
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            token_price_u128 / 1000000
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            last_update_unix_timestamp + 39 * 60
        );
    }

    // verify that this is not enough samples to use this twap
    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        let res = ctx.send_transaction_with_bot(&[refresh_ix]).await;

        assert_eq!(
            map_scope_error(res),
            ScopeError::TwapNotEnoughSamplesInPeriod
        );
    }
}

#[tokio::test]
async fn test_multiple_prices_with_same_value_no_twap_change() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let token_price = Price {
        value: 5100,
        exp: 9,
    };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let oracle_twaps: Box<OracleTwaps> =
        ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
    assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
    assert_eq!(
        oracle_twaps.twaps[0].current_ema_1h,
        Decimal::from(token_price).to_scaled_val().unwrap()
    );

    let token_price_u128: u128 = Decimal::from(token_price).to_scaled_val().unwrap();
    for index in 1..100 {
        // Fast forward time and refresh price with the same value
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        // verify that the twap value didn't change but the twap date increased
        let oracle_twaps_updated: OracleTwaps =
            ctx.get_zero_copy_account(&feed.twaps).await.unwrap();

        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            token_price_u128 / 1000000
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            oracle_twaps.twaps[0].last_update_unix_timestamp + 60 * index
        );
    }

    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        let price: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let expected: Decimal = token_price.into();
        let price_scaled = price.to_scaled_val::<u128>().unwrap();
        let expected_scaled = expected.to_scaled_val::<u128>().unwrap();
        assert_fuzzy_eq!(price_scaled, expected_scaled, expected_scaled / 100000000);
    }
}

#[tokio::test]
async fn test_multiple_prices_with_same_increasing_value_twap_increases() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let mut price_value = 543;
    let mut token_price = Price {
        value: price_value,
        exp: 8,
    };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let mut prev_oracle_twaps: Box<OracleTwaps> =
        ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
    assert_eq!(prev_oracle_twaps.twaps[0].last_update_slot, 1);
    assert_eq!(
        prev_oracle_twaps.twaps[0].current_ema_1h,
        Decimal::from(token_price).to_scaled_val().unwrap()
    );

    for _ in 1..10 {
        price_value += 5;
        token_price.value = price_value;

        // Fast forward time and refresh price with the new value
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        // verify that the twap value didn't change but the twap date increased
        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();

        assert!(
            oracle_twaps_updated.twaps[0].current_ema_1h
                > prev_oracle_twaps.twaps[0].current_ema_1h
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            prev_oracle_twaps.twaps[0].last_update_unix_timestamp + 60
        );

        prev_oracle_twaps = oracle_twaps_updated;
    }

    let twap_after_price_move = Decimal::from_scaled_val(prev_oracle_twaps.twaps[0].current_ema_1h);
    let price_after_price_move = Decimal::from(token_price);
    let diff_price_twap_after_price_move = price_after_price_move - twap_after_price_move;

    // Check that the twap stabilize after 1hour without price change
    for _ in 1..60 {
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();

        let price: Decimal = oracle_prices.prices[TEST_PYTH_ORACLE.token].price.into();
        let twap: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let diff_price_twap_after_1h = price - twap;

        // Assert that the diff has decreased by 85% after 1h
        assert!(
            diff_price_twap_after_1h < diff_price_twap_after_price_move * 15/100,
            "diff_price_twap_after_1h: {diff_price_twap_after_1h}, diff_price_twap_after_price_move: {diff_price_twap_after_price_move}"
        )
    }
}

#[tokio::test]
async fn test_multiple_prices_with_decreasing_value_twap_decreases() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    let mut price_value = 500;
    let mut token_price = Price {
        value: price_value,
        exp: 6,
    };
    // Change price
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

    setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;

    // Refresh price
    let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
    ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

    // Verify that TWAP is the same as the price as it is the first sample
    let mut prev_oracle_twaps: Box<OracleTwaps> =
        ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
    assert_eq!(prev_oracle_twaps.twaps[0].last_update_slot, 1);
    assert_eq!(
        prev_oracle_twaps.twaps[0].current_ema_1h,
        Decimal::from(token_price).to_scaled_val().unwrap()
    );

    for _ in 1..10 {
        price_value -= 10;
        token_price.value = price_value;

        // Fast forward time and refresh price with the new value
        ctx.fast_forward_seconds(70).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_twaps_updated: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();

        assert!(
            oracle_twaps_updated.twaps[0].current_ema_1h
                < prev_oracle_twaps.twaps[0].current_ema_1h
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            prev_oracle_twaps.twaps[0].last_update_unix_timestamp + 70
        );

        prev_oracle_twaps = oracle_twaps_updated;
    }

    let twap_after_price_move = Decimal::from_scaled_val(prev_oracle_twaps.twaps[0].current_ema_1h);
    let price_after_price_move = Decimal::from(token_price);
    let diff_price_twap_after_price_move = twap_after_price_move - price_after_price_move;

    // Check that the twap stabilize after 1hour without price change
    for _ in 1..60 {
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();

        let price: Decimal = oracle_prices.prices[TEST_PYTH_ORACLE.token].price.into();
        let twap: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let diff_price_twap_after_1h = twap - price;

        // Assert that the diff has decreased by 85% after 1h
        assert!(
            diff_price_twap_after_1h < diff_price_twap_after_price_move * 15/100,
            "diff_price_twap_after_1h: {diff_price_twap_after_1h}, diff_price_twap_after_price_move: {diff_price_twap_after_price_move}"
        );
    }
}

// todo: remove the need for extra stack
#[tokio::test]
async fn test_reset_twap() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, Vec::new()).await;

    // Initialize oracle account and set price
    let mut token_price = Price { value: 100, exp: 6 };
    {
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        setup_mapping_for_token_with_twap(&mut ctx, &feed, TEST_PYTH_ORACLE, TEST_TWAP).await;
    }

    {
        // Refresh
        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(oracle_twaps.twaps[0].last_update_slot, 1);
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );
    }

    {
        // update the price of the token and reset the TWAP to the current value of the token
        ctx.fast_forward_seconds(10).await;
        token_price = Price { value: 200, exp: 6 };
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;

        let refresh_ix = refresh_simple_oracle_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        // reset the TWAP
        let reset_twap_ix = reset_twap(&ctx.admin.pubkey(), &feed, TEST_PYTH_ORACLE);
        ctx.send_transaction(&[reset_twap_ix]).await.unwrap();
    }

    {
        ctx.fast_forward_seconds(10).await;
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );
    }

    {
        // verify that reset is idempotent
        let reset_twap_ix = reset_twap(&ctx.admin.pubkey(), &feed, TEST_PYTH_ORACLE);
        ctx.send_transaction(&[reset_twap_ix]).await.unwrap();
    }

    {
        let oracle_twaps: Box<OracleTwaps> =
            ctx.get_zero_copy_account_boxed(&feed.twaps).await.unwrap();
        assert_eq!(
            oracle_twaps.twaps[0].current_ema_1h,
            Decimal::from(token_price).to_scaled_val().unwrap()
        );
    }
}
