use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

pub fn find_data_address(pid: &Pubkey) -> Pubkey {
    let bpf_loader_addr: Pubkey =
        Pubkey::from_str("BPFLoaderUpgradeab1e11111111111111111111111").unwrap();

    let (program_data_address, _) =
        Pubkey::find_program_address(&[&pid.to_bytes()], &bpf_loader_addr);

    program_data_address
}
