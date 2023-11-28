use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use scope::Price;
use solana_program::instruction::Instruction;
use solana_sdk::signer::Signer;
use types::TestContext;

use super::{
    types::{OracleConf, ScopeFeedDefinition},
    *,
};

pub async fn setup_scope(
    feed_name: &str,
    mapping: Vec<OracleConf>,
) -> (TestContext, types::ScopeFeedDefinition) {
    let mut test_program = runner::program();
    let admin = setup::funded_kp(&mut test_program, 100000000);
    let bot = setup::funded_kp(&mut test_program, 10000000000);
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
        token_metadatas: zero_copy_accounts.token_metadatas.pubkey(),
        oracle_twaps: zero_copy_accounts.oracle_twaps.pubkey(),
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
        twaps: zero_copy_accounts.oracle_twaps.pubkey(),
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
