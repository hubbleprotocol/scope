use anchor_lang::{InstructionData, ToAccountMetas};
use scope::utils::pdas::mints_to_scope_chains_pubkey;
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

pub fn create_mint_map(
    admin: &Pubkey,
    feed: &ScopeFeedDefinition,
    seed_pk: &Pubkey,
    seed_id: u64,
    mints: &[Pubkey],
    scope_chains: Vec<[u16; 4]>,
) -> (Instruction, Pubkey, u8) {
    let (mapping_pk, bump) = mints_to_scope_chains_pubkey(&feed.prices, seed_pk, seed_id);

    let mut accounts = scope::accounts::CreateMintMap {
        admin: *admin,
        configuration: feed.conf,
        mappings: mapping_pk,
        system_program: solana_program::system_program::id(),
    }
    .to_account_metas(None);

    accounts.extend(mints.iter().map(|m| AccountMeta::new_readonly(*m, false)));

    let args = scope::instruction::CreateMintMap {
        seed_pk: *seed_pk,
        seed_id,
        bump,
        scope_chains,
    };

    (
        Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        },
        mapping_pk,
        bump,
    )
}

pub fn close_mint_map(
    admin: &Pubkey,
    feed: &ScopeFeedDefinition,
    mint_map: &Pubkey,
) -> Instruction {
    let accounts = scope::accounts::CloseMintMap {
        admin: *admin,
        configuration: feed.conf,
        mappings: *mint_map,
        system_program: solana_program::system_program::id(),
    };

    let args = scope::instruction::CloseMintMap {};

    Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    }
}

pub fn close_mint_map_from_seeds(
    admin: &Pubkey,
    feed: &ScopeFeedDefinition,
    seed_pk: &Pubkey,
    seed_id: u64,
) -> Instruction {
    let (mapping_pk, _) = mints_to_scope_chains_pubkey(&feed.prices, seed_pk, seed_id);

    close_mint_map(admin, feed, &mapping_pk)
}
