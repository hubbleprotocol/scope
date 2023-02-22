use anchor_lang::prelude::{Clock, Pubkey};
use scope::Price;
use solana_sdk::pubkey;

pub const fn id() -> Pubkey {
    // It does not matter what the pubkey is
    pubkey!("Switchv211111111111111111111111111111111111")
}

pub fn get_account_data_for_price(_price: &Price, _clock: &Clock) -> Vec<u8> {
    todo!("Implement switchboard prices");
}
