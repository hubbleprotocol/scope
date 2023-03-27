#[cfg(not(feature = "no-entrypoint"))]
use solana_program::entrypoint;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::invoke,
    pubkey::Pubkey,
};

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

/// Instruction processor
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let program_id = *accounts[0].key;
    // Replicate accounts metas
    let accounts_metas: Vec<AccountMeta> = accounts
        .iter()
        .skip(1)
        .map(|info| AccountMeta {
            pubkey: *info.key,
            is_signer: info.is_signer,
            is_writable: info.is_writable,
        })
        .collect();

    msg!("Calling {} with accounts: {accounts_metas:#?}", program_id);

    // Just pass the ix to yvaults
    invoke(
        &Instruction {
            program_id,
            accounts: accounts_metas,
            data: data.to_vec(),
        },
        accounts,
    )?;

    Ok(())
}
