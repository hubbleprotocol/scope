use anchor_lang::prelude::*;
use scope::oracles::spl_stake::spl_stake_pool::StakePool;
use scope::Price;

pub use solana_program::stake::program::id;

pub fn get_account_data_for_price(price: &Price, clock: &Clock) -> Vec<u8> {
    let pool_token_supply = 10_u64.pow(price.exp.try_into().unwrap());
    let total_lamports = price.value;
    let last_update_epoch = clock.epoch;

    let stake_pool = StakePool {
        pool_token_supply,
        total_lamports,
        last_update_epoch,
        ..Default::default()
    };
    stake_pool.try_to_vec().unwrap()
}
