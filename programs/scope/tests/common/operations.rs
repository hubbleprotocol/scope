use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program::{
    instruction::Instruction, pubkey::Pubkey, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use solana_program_test::BanksClientError;
use solana_sdk::signature::{Keypair, Signer};

use crate::common::{
    types,
    types::{OracleConf, TestContext},
    utils,
};

pub async fn update_oracle_mapping(
    ctx: &mut TestContext,
    feed: &types::ScopeFeedDefinition,
    conf: &OracleConf,
) {
    let accounts = scope::accounts::UpdateOracleMapping {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        price_info: Some(conf.pubkey),
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: conf.token.try_into().unwrap(),
        price_type: conf.price_type.to_u8(),
        twap_enabled: conf.twap_enabled,
        twap_source: conf.twap_source.unwrap_or(u16::MAX),
    };
    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };
    ctx.send_transaction(&[ix]).await.unwrap();
}

pub async fn refresh_price(
    ctx: &mut TestContext,
    feed: &types::ScopeFeedDefinition,
    conf: &OracleConf,
) {
    let mut accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        price_info: conf.pubkey,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    }
    .to_account_metas(None);
    let mut refresh_accounts = utils::get_remaining_accounts(ctx, conf).await;
    accounts.append(&mut refresh_accounts);

    let args = scope::instruction::RefreshOnePrice {
        token: conf.token.try_into().unwrap(),
    };
    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };
    ctx.send_transaction(&[ix]).await.unwrap();
}

pub async fn set_admin_cached(
    ctx: &mut TestContext,
    feed: &types::ScopeFeedDefinition,
    admin_cached: &Pubkey,
) -> Result<(), BanksClientError> {
    let accounts = scope::accounts::SetAdminCached {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
    }
    .to_account_metas(None);

    let args = scope::instruction::SetAdminCached {
        new_admin: admin_cached.clone(),
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };
    ctx.send_transaction(&[ix]).await
}

pub async fn approve_admin_cached(
    ctx: &mut TestContext,
    feed: &types::ScopeFeedDefinition,
    admin_cached: &Keypair,
) -> Result<(), BanksClientError> {
    let accounts = scope::accounts::ApproveAdminCached {
        admin_cached: admin_cached.pubkey(),
        configuration: feed.conf,
    }
    .to_account_metas(None);

    let args = scope::instruction::ApproveAdminCached {
        feed_name: feed.feed_name.clone(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };
    ctx.send_transaction_with_payer(&[ix], admin_cached).await
}
