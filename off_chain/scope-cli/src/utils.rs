use std::str::FromStr;

use scope::Price;
use solana_sdk::pubkey::Pubkey;

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
