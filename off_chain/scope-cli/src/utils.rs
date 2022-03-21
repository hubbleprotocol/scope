use std::str::FromStr;

use anchor_client::solana_client::rpc_client::RpcClient;
use scope::Price;
use solana_sdk::pubkey::Pubkey;

use anyhow::Result;

use solana_sdk::clock::Clock;
use solana_sdk::sysvar::SysvarId;

/// Get the program data address of the given program id
pub fn find_data_address(pid: &Pubkey) -> Pubkey {
    let bpf_loader_addr: Pubkey =
        Pubkey::from_str("BPFLoaderUpgradeab1e11111111111111111111111").unwrap();

    let (program_data_address, _) =
        Pubkey::find_program_address(&[&pid.to_bytes()], &bpf_loader_addr);

    program_data_address
}

/// Convert a price to f64
///
/// Used for display only
pub fn price_to_f64(price: &Price) -> f64 {
    // allow potential precision loss here as used for display only
    (price.value as f64) * 10_f64.powi(-(price.exp as i32))
}

/// Get current clock
pub fn get_clock(rpc: &RpcClient) -> Result<Clock> {
    let clock = rpc.get_account(&Clock::id())?.deserialize_data()?;

    Ok(clock)
}
