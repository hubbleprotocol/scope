use anchor_lang::{InstructionData, ToAccountMetas};
use scope::UpdateTokenMetadataMode;
use solana_program::instruction::Instruction;
use solana_sdk::signer::Signer;

use super::types::{OracleConf, ScopeFeedDefinition, TestContext};
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;

pub fn metadata_enable_store_observations(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    oracle: OracleConf,
) -> Instruction {
    update_token_metadata_ix(
        ctx,
        feed,
        oracle,
        UpdateTokenMetadataMode::StoreObservations,
        vec![1],
    )
}

pub fn metadata_set_twap_source(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    oracle: OracleConf,
    source: usize,
) -> Instruction {
    update_token_metadata_ix(
        ctx,
        feed,
        oracle,
        UpdateTokenMetadataMode::TwapSource,
        (source as u16).to_le_bytes().into(),
    )
}

pub fn update_token_metadata_ix(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    oracle: OracleConf,
    mode: UpdateTokenMetadataMode,
    value: Vec<u8>,
) -> Instruction {
    let accounts = scope::accounts::UpdateTokensMetadata {
        admin: ctx.admin.pubkey(),
        configuration: feed.conf,
        tokens_metadata: feed.metadatas,
    };

    let args = scope::instruction::UpdateTokenMetadata {
        index: oracle.token.try_into().unwrap(),
        mode: mode.to_u64(),
        feed_name: feed.feed_name.clone(),
        value,
    };

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}

pub fn refresh_one_ix(feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        price_info: oracle.pubkey,
        oracle_twaps: feed.twaps,
        tokens_metadata: feed.metadatas,
    };

    let args = scope::instruction::RefreshOnePrice {
        token: oracle.token.try_into().unwrap(),
    };

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}
