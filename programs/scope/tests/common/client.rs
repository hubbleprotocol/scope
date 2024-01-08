use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program::instruction::AccountMeta;
use solana_program::{instruction::Instruction, pubkey::Pubkey};

use super::types::{OracleConf, ScopeFeedDefinition};
use solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;

pub fn refresh_simple_oracle_ix(feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let mut accounts = scope::accounts::RefreshList {
        oracle_prices: feed.prices,
        oracle_mappings: feed.mapping,
        instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        oracle_twaps: feed.twaps,
    }
    .to_account_metas(None);

    accounts.push(AccountMeta::new_readonly(oracle.pubkey, false));

    let args = scope::instruction::RefreshPriceList {
        tokens: vec![oracle.token.try_into().unwrap()],
    };

    Instruction {
        program_id: scope::id(),
        accounts,
        data: args.data(),
    }
}

pub fn reset_twap(admin: &Pubkey, feed: &ScopeFeedDefinition, oracle: OracleConf) -> Instruction {
    let accounts = scope::accounts::ResetTwap {
        admin: *admin,
        oracle_prices: feed.prices,
        configuration: feed.conf,
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
