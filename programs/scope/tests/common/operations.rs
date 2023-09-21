use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program::{
    clock::Clock,
    instruction::Instruction,
    sysvar::{instructions::ID as SYSVAR_INSTRUCTIONS_ID, SysvarId},
};
use solana_sdk::signature::Signer;

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
        price_info: conf.pubkey,
    };
    let args = scope::instruction::UpdateMapping {
        feed_name: feed.feed_name.clone(),
        token: conf.token.try_into().unwrap(),
        price_type: conf.price_type.to_u8(),
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
        clock: Clock::id(),
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
