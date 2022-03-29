use anchor_lang::prelude::Pubkey;

pub const LOCALNET_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    24, 182, 101, 20, 142, 227, 3, 67, 30, 101, 120, 181, 199, 162, 221, 168, 118, 163, 228, 210,
    0, 51, 111, 6, 30, 93, 175, 34, 94, 52, 16, 162,
]);
pub const DEVNET_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    37, 32, 214, 151, 217, 212, 227, 112, 61, 225, 76, 31, 72, 56, 19, 20, 138, 52, 176, 38, 197,
    25, 215, 56, 71, 155, 38, 199, 42, 39, 3, 158,
]);
pub const MAINNET_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    241, 132, 37, 68, 185, 143, 22, 221, 129, 231, 167, 12, 131, 77, 133, 134, 12, 88, 187, 200,
    219, 82, 194, 31, 226, 188, 76, 223, 202, 47, 94, 3,
]);

cfg_if::cfg_if! {
    if #[cfg(feature = "mainnet")] {
        pub const PROGRAM_ID:Pubkey = MAINNET_PROGRAM_ID;
    }
    else if #[cfg(feature = "localnet")] {
        pub const PROGRAM_ID:Pubkey = LOCALNET_PROGRAM_ID;
    } else if #[cfg(feature = "devnet")] {
        pub const PROGRAM_ID:Pubkey = DEVNET_PROGRAM_ID;
    } else {
        compile_error!("At least one of 'mainnet', 'localnet' or 'devnet' feature need to be set")
    }
}
