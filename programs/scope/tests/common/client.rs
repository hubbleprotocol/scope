use anchor_lang::{InstructionData, ToAccountMetas};
use scope::UpdateTokenMetadataMode;
use solana_program::instruction::Instruction;
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
