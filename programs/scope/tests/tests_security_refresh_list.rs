mod common;

use anchor_lang::{
    prelude::{AccountMeta, Clock, Pubkey},
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

const TEST_ORACLE_CONF: [OracleConf; 2] = [TEST_PYTH_ORACLE, TEST_PYTH2_ORACLE];

// - [x] Wrong oracle mapping
// - [x] Wrong oracle account (copy)
// - [x] Wrong oracle account (mixing indexes)
// - [x] Wrong sysvar instruction account
// - [x] Instruction executed in CPI
// - [x] Instruction preceded by non ComputeBudget instruction

#[tokio::test]
async fn test_working_refresh_list() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Change prices
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        mock_oracles::set_price(
            &mut ctx,
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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));
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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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

// - [ ] Instruction executed in CPI
#[tokio::test]
async fn test_refresh_through_cpi() {
    let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, TEST_ORACLE_CONF.to_vec()).await;

    // Change prices
    for (i, conf) in TEST_ORACLE_CONF.iter().enumerate() {
        mock_oracles::set_price(
            &mut ctx,
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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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
    accounts.extend(TEST_ORACLE_CONF.map(|conf| AccountMeta {
        pubkey: conf.pubkey,
        is_signer: false,
        is_writable: false,
    }));

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
