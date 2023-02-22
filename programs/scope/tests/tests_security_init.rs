mod common;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use common::*;
use scope::accounts::Initialize;
use solana_program::instruction::Instruction;
use solana_program_test::tokio;
use solana_sdk::{signature::Keypair, signer::Signer};

use crate::{
    common::{setup::funded_kp, utils::AnchorErrorCode},
    utils::map_anchor_error,
};

// - [x] Non zeroed mapping account
// - [x] Non zeroed price account
// - [x] Not matching config PDA account

#[tokio::test]
async fn test_working_init() {
    let mut test_program = runner::program();
    let admin = funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, Keypair::new(), Keypair::new()).await;
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", DEFAULT_FEED_NAME.as_bytes()], &scope::id());
    let accounts = Initialize {
        admin: admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: DEFAULT_FEED_NAME.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction_with_payer(&[ix], &admin)
        .await
        .unwrap();
}

// - [ ] Non zeroed mapping account
#[tokio::test]
async fn test_non_zeroed_mapping_account() {
    let mut test_program = runner::program();
    let admin = funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, Keypair::new(), Keypair::new()).await;
    let garbage_data = vec![0xDEADBEEF_u32; (std::mem::size_of::<scope::OracleMappings>() / 4) + 2]
        .into_iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<u8>>();
    ctx.set_account(
        &zero_copy_accounts.mapping.pubkey(),
        garbage_data,
        &scope::id(),
    );
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", DEFAULT_FEED_NAME.as_bytes()], &scope::id());
    let accounts = Initialize {
        admin: admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: DEFAULT_FEED_NAME.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction_with_payer(&[ix], &admin).await),
        AnchorErrorCode::ConstraintZero,
    );
}

// - [ ] Non zeroed price account
#[tokio::test]
async fn test_non_zeroed_price_account() {
    let mut test_program = runner::program();
    let admin = funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, Keypair::new(), Keypair::new()).await;
    let garbage_data = vec![0xDEADBEEF_u32; (std::mem::size_of::<scope::OraclePrices>() / 4) + 2]
        .into_iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<u8>>();
    ctx.set_account(
        &zero_copy_accounts.prices.pubkey(),
        garbage_data,
        &scope::id(),
    );
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", DEFAULT_FEED_NAME.as_bytes()], &scope::id());
    let accounts = Initialize {
        admin: admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: DEFAULT_FEED_NAME.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction_with_payer(&[ix], &admin).await),
        AnchorErrorCode::ConstraintZero,
    );
}

// - [ ] Not matching config PDA account
#[tokio::test]
async fn test_non_matching_pda() {
    let mut test_program = runner::program();
    let admin = funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, Keypair::new(), Keypair::new()).await;
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", "DFH-1".as_bytes()], &scope::id());
    let accounts = Initialize {
        admin: admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: DEFAULT_FEED_NAME.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    assert_eq!(
        map_anchor_error(ctx.send_transaction_with_payer(&[ix], &admin).await),
        AnchorErrorCode::ConstraintSeeds,
    );
}
