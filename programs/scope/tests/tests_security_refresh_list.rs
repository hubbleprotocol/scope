use anchor_lang::{
    prelude::{AccountMeta, Clock, Pubkey},
    InstructionData, ToAccountMetas,
};
use common::*;
use scope::{
    oracles::{
        ktokens::{GlobalConfig, WhirlpoolStrategy},
        OracleType,
    },
    OraclePrices, Price, ScopeError,
};
use solana_program::{
    instruction::Instruction,
    sysvar::{instructions::ID as SYSVAR_INSTRUCTIONS_ID, SysvarId},
};
use solana_program_test::tokio;
use solana_sdk::{pubkey, signer::Signer};
use types::*;

use crate::{
    common::utils::AnchorErrorCode,
    utils::{map_anchor_error, map_scope_error},
};

mod common;

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

const TEST_KTOKEN_ORACLE: OracleConf = OracleConf {
    pubkey: pubkey!("SomeKaminoStrategyAccount111111111111111111"),
    token: 2,
    price_type: OracleType::KToken,
};

const TEST_ORACLE_CONF: [OracleConf; 3] = [TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE, TEST_KTOKEN_ORACLE];

// - [x] Wrong oracle mapping
// - [x] Wrong oracle account (copy)
// - [x] Wrong oracle account (mixing indexes)
// - [x] Wrong sysvar instruction account
// - [x] Instruction executed in CPI
// - [x] Instruction preceded by non ComputeBudget instruction

// KTokens:
// - [x] Wrong kToken additional global config account
// - [x] Wrong kToken additional collateral infos account
// - [x] Wrong kToken additional orca whirlpool account
// - [x] Wrong kToken additional orca position account
// - [x] Wrong kToken additional scope prices account

#[tokio::test]
async fn test_working_refresh_list() {
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

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction_with_bot(&[ix]).await.unwrap();

    // Check prices
    let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        assert_eq!(data.prices[conf.token].price.value, (i as u64) + 1);
        assert_eq!(data.prices[conf.token].price.exp, 6);
    }
}

// - [ ] Wrong oracle mapping
#[tokio::test]
async fn test_wrong_oracle_mapping() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Create a fake mapping account
    let fake_mapping_pk = Pubkey::new_unique();
    ctx.clone_account(&feed.mapping, &fake_mapping_pk).await;

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

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: fake_mapping_pk,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
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
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Create a fake mapping account
    let fake_price_account = Pubkey::new_unique();
    ctx.clone_account(&TEST_PYTH_ORACLE.pubkey, &fake_price_account)
        .await;

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

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Replace fake account
    accounts[4] = AccountMeta {
        pubkey: fake_price_account,
        is_signer: false,
        is_writable: false,
    };

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
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

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);

    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    let mut tokens = TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec();

    // Swap the two first elements
    tokens.swap(0, 1);

    let args = scope::instruction::RefreshPriceList { tokens };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
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

    // Create the fake sysvar
    let wrong_sysvar_account = Pubkey::new_unique();

    ctx.set_account(&wrong_sysvar_account, vec![0; 100], &Pubkey::new_unique());

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: wrong_sysvar_account,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    let res = ctx.send_transaction_with_bot(&[ix]).await;
    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintAddress);
}

// - [ ] Wrong kToken additional global config account
#[tokio::test]
async fn test_wrong_ktoken_global_config() {
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

    let strategy: WhirlpoolStrategy = ctx
        .get_zero_copy_account(&TEST_KTOKEN_ORACLE.pubkey)
        .await
        .unwrap();

    // Create the fake global config
    let wrong_global_config = Pubkey::new_unique();
    ctx.clone_account(&strategy.global_config, &wrong_global_config)
        .await;

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Set the wrong global config
    accounts.iter_mut().for_each(|account| {
        if account.pubkey == strategy.global_config {
            account.pubkey = wrong_global_config;
        }
    });

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
    let prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    // kToken price not updated
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].last_updated_slot, 0);
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].price.value, 0);
}

// - [ ] Wrong kToken additional collateral infos account
#[tokio::test]
async fn test_wrong_ktoken_collateral_infos() {
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

    let strategy: WhirlpoolStrategy = ctx
        .get_zero_copy_account(&TEST_KTOKEN_ORACLE.pubkey)
        .await
        .unwrap();
    let global_config: GlobalConfig = ctx
        .get_zero_copy_account(&strategy.global_config)
        .await
        .unwrap();

    // Create the fake collateral infos
    let wrong_token_infos = Pubkey::new_unique();
    ctx.clone_account(&global_config.token_infos, &wrong_token_infos)
        .await;

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Set the wrong collateral infos
    accounts.iter_mut().for_each(|account| {
        if account.pubkey == global_config.token_infos {
            account.pubkey = wrong_token_infos;
        }
    });

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
    let prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    // kToken price not updated
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].last_updated_slot, 0);
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].price.value, 0);
}

// - [ ] Wrong kToken additional orca whirlpool account
#[tokio::test]
async fn test_wrong_ktoken_orca_whirlpool() {
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

    let strategy: WhirlpoolStrategy = ctx
        .get_zero_copy_account(&TEST_KTOKEN_ORACLE.pubkey)
        .await
        .unwrap();

    // Create the fake orca whirlpool
    let wrong_orca_whirlpool = Pubkey::new_unique();
    ctx.clone_account(&strategy.pool, &wrong_orca_whirlpool)
        .await;

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Set the wrong orca whirlpool
    accounts.iter_mut().for_each(|account| {
        if account.pubkey == strategy.pool {
            account.pubkey = wrong_orca_whirlpool;
        }
    });

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
    let prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    // kToken price not updated
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].last_updated_slot, 0);
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].price.value, 0);
}

// - [ ] Wrong kToken additional orca position account
#[tokio::test]
async fn test_wrong_ktoken_orca_position() {
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

    let strategy: WhirlpoolStrategy = ctx
        .get_zero_copy_account(&TEST_KTOKEN_ORACLE.pubkey)
        .await
        .unwrap();

    // Create the fake orca position
    let wrong_orca_position = Pubkey::new_unique();
    ctx.clone_account(&strategy.position, &wrong_orca_position)
        .await;

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Set the wrong orca position
    accounts.iter_mut().for_each(|account| {
        if account.pubkey == strategy.position {
            account.pubkey = wrong_orca_position;
        }
    });

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
    let prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    // kToken price not updated
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].last_updated_slot, 0);
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].price.value, 0);
}

// - [ ] Wrong kToken additional scope prices account
#[tokio::test]
async fn test_wrong_ktoken_scope_prices() {
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

    let strategy: WhirlpoolStrategy = ctx
        .get_zero_copy_account(&TEST_KTOKEN_ORACLE.pubkey)
        .await
        .unwrap();

    // Create the fake scope prices
    let wrong_scope_prices = Pubkey::new_unique();
    ctx.clone_account(&strategy.scope_prices, &wrong_scope_prices)
        .await;

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }
    // Set the wrong scope prices
    accounts.iter_mut().for_each(|account| {
        if account.pubkey == strategy.scope_prices {
            account.pubkey = wrong_scope_prices;
        }
    });

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();
    let prices: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
    // kToken price not updated
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].last_updated_slot, 0);
    assert_eq!(prices.prices[TEST_KTOKEN_ORACLE.token].price.value, 0);
}

// - [ ] Instruction executed in CPI
#[tokio::test]
async fn test_refresh_through_cpi() {
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

    // Refresh
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    let res = ctx.send_transaction_through_cpi(&[ix]).await;
    assert_eq!(map_scope_error(res), ScopeError::RefreshInCPI);
}

// Note: nominal cases with compute budget tested in crank tests.
// - [ ] Instruction preceded by non ComputeBudget instruction
#[tokio::test]
async fn test_refresh_with_unexpected_ix() {
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
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        clock: Clock::id(),
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    for conf in TEST_ORACLE_CONF.iter() {
        let mut refresh_accounts = utils::get_refresh_list_accounts(&mut ctx, conf).await;
        accounts.append(&mut refresh_accounts);
    }

    let args = scope::instruction::RefreshPriceList {
        tokens: TEST_ORACLE_CONF.map(|conf| conf.token as u16).to_vec(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    };

    let res = ctx.send_transaction(&[extra_ix, ix]).await;
    assert_eq!(map_scope_error(res), ScopeError::RefreshWithUnexpectedIxs);
}
