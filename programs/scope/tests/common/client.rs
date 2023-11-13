use anchor_lang::{InstructionData, ToAccountMetas};
use scope::UpdateTokenMetadataMode;
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::signer::Signer;

use super::types::{OracleConf, ScopeFeedDefinition, TestContext};
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;

pub fn refresh_one_ix(feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let accounts = scope::accounts::RefreshOne {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        price_info: oracle.pubkey,
        oracle_twaps: feed.twaps,
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

pub fn reset_twap(admin: &Pubkey, feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let accounts = scope::accounts::ResetTwap {
        admin: *admin,
        oracle_prices: feed.prices,
        configuration: feed.conf,
        oracle_mappings: feed.mapping,
        oracle_twaps: feed.twaps,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
    };

    let args = scope::instruction::ResetTwap {
        token: oracle.token.try_into().unwrap(),
        feed_name: feed.feed_name.clone(),
    };

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}
