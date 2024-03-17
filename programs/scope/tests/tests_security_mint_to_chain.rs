#![allow(clippy::items_after_test_module)]
use anchor_lang::prelude::Pubkey;
use common::*;
use scope::Price;
use solana_program_test::tokio;
use solana_sdk::{pubkey, signer::Signer};
use types::*;

use crate::{common::utils::AnchorErrorCode, utils::map_anchor_error};

mod common;

const TEST_PYTH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePythPriceAccount11111111111111111111111"),
    token: 0,
    price_type: TestOracleType::Pyth,
    twap_enabled: false,
    twap_source: None,
};

const TEST_PYTH2_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomePyth2PriceAccount1111111111111111111111"),
    token: 1,
    price_type: TestOracleType::Pyth,
    twap_enabled: false,
    twap_source: None,
};

const TEST_JLP_FETCH_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomeJLPPriceAccount111111111111111111111111"),
    token: 5,
    price_type: TestOracleType::JupiterLPFetch,
    twap_enabled: false,
    twap_source: None,
};

const TEST_JLP_COMPUTE_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomeJLP2PriceAccount11111111111111111111111"),
    token: 6,
    price_type: TestOracleType::JupiterLpCompute,
    twap_enabled: false,
    twap_source: None,
};

const TEST_ORCA_ATOB: OracleConf = OracleConf {
    pubkey: pubkey!("SomeorcaPriceAccount11111111111111111111111"),
    token: 7,
    price_type: TestOracleType::OrcaWhirlpool(true),
    twap_enabled: false,
    twap_source: None,
};

const TEST_ORCA_BTOA: OracleConf = OracleConf {
    pubkey: pubkey!("SomeorcaPriceAccount21111111111111111111111"),
    token: 8,
    price_type: TestOracleType::OrcaWhirlpool(false),
    twap_enabled: false,
    twap_source: None,
};

const TEST_RAYDIUM_ATOB: OracleConf = OracleConf {
    pubkey: pubkey!("SomeRaydiumPriceAccount11111111111111111111"),
    token: 9,
    price_type: TestOracleType::RaydiumAmmV3(true),
    twap_enabled: false,
    twap_source: None,
};

const TEST_ORACLE_CONF: [OracleConf; 7] = [
    TEST_PYTH_ORACLE,
    TEST_PYTH2_ORACLE,
    TEST_JLP_FETCH_ORACLE,
    TEST_JLP_COMPUTE_ORACLE,
    TEST_ORCA_ATOB,
    TEST_ORCA_BTOA,
    TEST_RAYDIUM_ATOB,
];

#[tokio::test]
async fn test_working_create_close() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Change prices
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            conf,
            &Price {
                value: (i as u64) + 1,
                exp: 6,
            },
        )
        .await;
    }

    // Create some mints for each oracle entry
    let mints = TEST_ORACLE_CONF.map(|_| setup::create_mint(&mut ctx));

    let scope_chains = TEST_ORACLE_CONF
        .iter()
        .map(|conf| [conf.token as u16, 0, 0, 0])
        .collect::<Vec<_>>();

    // Create the mint map
    let (create_mint_map_ix, mint_map_pk, _) = client::create_mint_map(
        &ctx.admin.pubkey(),
        &feed,
        &Pubkey::new_unique(),
        0,
        &mints,
        scope_chains,
    );
    ctx.send_transaction(&[create_mint_map_ix]).await.unwrap();

    // Close the mint map
    let close_mint_map_ix = client::close_mint_map(&ctx.admin.pubkey(), &feed, &mint_map_pk);
    ctx.send_transaction(&[close_mint_map_ix]).await.unwrap();
}

#[tokio::test]
async fn test_security_open_mint_to_chain() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Change prices
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            conf,
            &Price {
                value: (i as u64) + 1,
                exp: 6,
            },
        )
        .await;
    }

    // Create some mints for each oracle entry
    let mints = TEST_ORACLE_CONF.map(|_| setup::create_mint(&mut ctx));

    let scope_chains = TEST_ORACLE_CONF
        .iter()
        .map(|conf| [conf.token as u16, 0, 0, 0])
        .collect::<Vec<_>>();

    // Wrong admin
    {
        let (create_mint_map_ix, _, _) = client::create_mint_map(
            &ctx.bot.pubkey(),
            &feed,
            &Pubkey::new_unique(),
            0,
            &mints,
            scope_chains.clone(),
        );
        let res = ctx.send_transaction_with_bot(&[create_mint_map_ix]).await;
        assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
    }

    // Wrong signer
    {
        let (mut create_mint_map_ix, _, _) = client::create_mint_map(
            &ctx.admin.pubkey(),
            &feed,
            &Pubkey::new_unique(),
            0,
            &mints,
            scope_chains,
        );
        // Allow to compile the tx
        create_mint_map_ix.accounts[0].is_signer = false;
        let res = ctx.send_transaction_with_bot(&[create_mint_map_ix]).await;
        assert_eq!(map_anchor_error(res), AnchorErrorCode::AccountNotSigner);
    }
}

#[tokio::test]
async fn test_security_close_mint_to_chain() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Change prices
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            conf,
            &Price {
                value: (i as u64) + 1,
                exp: 6,
            },
        )
        .await;
    }

    // Create some mints for each oracle entry
    let mints = TEST_ORACLE_CONF.map(|_| setup::create_mint(&mut ctx));

    let scope_chains = TEST_ORACLE_CONF
        .iter()
        .map(|conf| [conf.token as u16, 0, 0, 0])
        .collect::<Vec<_>>();

    // Create the mint map
    let (create_mint_map_ix, mint_map_pk, _) = client::create_mint_map(
        &ctx.admin.pubkey(),
        &feed,
        &Pubkey::new_unique(),
        0,
        &mints,
        scope_chains,
    );
    ctx.send_transaction(&[create_mint_map_ix]).await.unwrap();

    // Wrong admin
    {
        let close_mint_map_ix = client::close_mint_map(&ctx.bot.pubkey(), &feed, &mint_map_pk);
        let res = ctx.send_transaction_with_bot(&[close_mint_map_ix]).await;
        assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
    }

    // Wrong signer
    {
        let mut close_mint_map_ix =
            client::close_mint_map(&ctx.admin.pubkey(), &feed, &mint_map_pk);
        // Allow to compile the tx
        close_mint_map_ix.accounts[0].is_signer = false;
        let res = ctx.send_transaction_with_bot(&[close_mint_map_ix]).await;
        assert_eq!(map_anchor_error(res), AnchorErrorCode::AccountNotSigner);
    }

    // Configuration account from another feed
    {
        let config_clone_pk = Pubkey::new_unique();
        let mut config_clone: scope::Configuration =
            ctx.get_zero_copy_account(&feed.conf).await.unwrap();

        config_clone.oracle_prices = Pubkey::new_unique();
        ctx.set_zero_copy_account(&config_clone_pk, &config_clone);

        let mut close_mint_map_ix =
            client::close_mint_map(&ctx.admin.pubkey(), &feed, &mint_map_pk);

        close_mint_map_ix.accounts[1].pubkey = config_clone_pk;
        let res = ctx.send_transaction(&[close_mint_map_ix]).await;
        assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintRaw);
    }
}
