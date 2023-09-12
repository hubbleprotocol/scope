use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use scope::Price;
use solana_program::instruction::Instruction;
use solana_sdk::signer::Signer;
use types::TestContext;

use super::{types::OracleConf, *};

pub async fn setup_scope(
    feed_name: &str,
    mapping: Vec<OracleConf>,
) -> (TestContext, types::ScopeFeedDefinition) {
    let mut test_program = runner::program();
    let admin = setup::funded_kp(&mut test_program, 100000000);
    let bot = setup::funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, admin, bot).await;
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", feed_name.as_bytes()], &scope::id());
    let accounts = scope::accounts::Initialize {
        admin: ctx.admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: feed_name.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();

    let feed = types::ScopeFeedDefinition {
        feed_name: feed_name.to_string(),
        conf: configuration_acc,
        mapping: zero_copy_accounts.mapping.pubkey(),
        prices: zero_copy_accounts.prices.pubkey(),
    };

    // Set up the mapping and oracles
    for conf in mapping {
        // Initialize oracle account
        mock_oracles::set_price(&mut ctx, &feed, &conf, &Price::default()).await;
        // Set the mapping
        operations::update_oracle_mapping(&mut ctx, &feed, &conf).await;
    }

    (ctx, feed)
}
