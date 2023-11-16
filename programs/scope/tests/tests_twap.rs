mod common;

use crate::client::reset_twap;
use crate::{common::client::refresh_one_ix, utils::setup_mapping_for_token_with_twap};
use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use decimal_wad::decimal::Decimal;
use scope::{assert_fuzzy_eq, EmaTwap, OracleMappings, OraclePrices, OracleTwaps, Price};
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
        let refresh_ix = refresh_one_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(oracle_prices.prices[TEST_TWAP.token].price, token_price);
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
    ctx.fast_forward_seconds(10).await;
    mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
            last_update_unix_timestamp + 10
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        // verify that the twap value didn't change but the twap date increased
        let oracle_twaps_updated: OracleTwaps =
            ctx.get_zero_copy_account(&feed.twaps).await.unwrap();

        assert_fuzzy_eq!(
            oracle_twaps_updated.twaps[0].current_ema_1h,
            token_price_u128,
            2
        );

        assert_eq!(
            oracle_twaps_updated.twaps[0].last_update_unix_timestamp,
            oracle_twaps.twaps[0].last_update_unix_timestamp + 60 * index
        );
    }

    {
        let refresh_ix = refresh_one_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        let price: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let expected: Decimal = token_price.into();
        assert_fuzzy_eq!(
            price.to_scaled_val::<u128>().unwrap(),
            expected.to_scaled_val::<u128>().unwrap(),
            2
        );
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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

    // Check that the twap stabilize after 1hour without price change
    for _ in 1..60 {
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        let refresh_ix = refresh_one_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        let price: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let expected: Decimal = token_price.into();
        assert_fuzzy_eq!(
            price.to_scaled_val::<u128>().unwrap(),
            expected.to_scaled_val::<u128>().unwrap(),
            3
        );
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
    let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        // verify that the twap value didn't change but the twap date increased
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

    // Check that the twap stabilize after 1hour without price change
    for _ in 1..60 {
        ctx.fast_forward_seconds(60).await;
        mock_oracles::set_price(&mut ctx, &feed, &TEST_PYTH_ORACLE, &token_price).await;
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();
    }

    {
        let refresh_ix = refresh_one_ix(&feed, TEST_TWAP);
        ctx.send_transaction_with_bot(&[refresh_ix]).await.unwrap();

        let oracle_prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        let price: Decimal = oracle_prices.prices[TEST_TWAP.token].price.into();
        let expected: Decimal = token_price.into();
        assert_fuzzy_eq!(
            price.to_scaled_val::<u128>().unwrap(),
            expected.to_scaled_val::<u128>().unwrap(),
            2
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
        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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

        let refresh_ix = refresh_one_ix(&feed, TEST_PYTH_ORACLE);
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

pub mod utils {
    use anchor_lang::{InstructionData, ToAccountMetas};
    use solana_program::instruction::Instruction;
    use solana_sdk::signer::Signer;

    use crate::{
        common::types::{OracleConf, ScopeFeedDefinition},
        TestContext,
    };

    pub async fn setup_mapping_for_token_with_twap(
        ctx: &mut TestContext,
        feed: &ScopeFeedDefinition,
        token_oracle: OracleConf,
        twap_oracle: OracleConf,
    ) {
        // Set the mapping for first price account
        let accounts = scope::accounts::UpdateOracleMapping {
            admin: ctx.admin.pubkey(),
            configuration: feed.conf,
            oracle_mappings: feed.mapping,
            price_info: Some(token_oracle.pubkey),
        };
        let args = scope::instruction::UpdateMapping {
            feed_name: feed.feed_name.clone(),
            token: token_oracle.token.try_into().unwrap(),
            price_type: token_oracle.price_type.to_u8(),
            twap_enabled: token_oracle.twap_enabled,
            twap_source: token_oracle.twap_source.unwrap_or(u16::MAX),
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
            price_info: Some(twap_oracle.pubkey),
        };
        let args = scope::instruction::UpdateMapping {
            feed_name: feed.feed_name.clone(),
            token: twap_oracle.token.try_into().unwrap(),
            price_type: twap_oracle.price_type.to_u8(),
            twap_enabled: twap_oracle.twap_enabled,
            twap_source: twap_oracle.twap_source.unwrap_or(u16::MAX),
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        ctx.send_transaction(&[ix]).await.unwrap();
    }
}
