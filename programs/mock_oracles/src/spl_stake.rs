use anchor_lang::{
    prelude::{AccountInfo, Clock, Result, SolanaSysvar},
    AnchorSerialize,
};
use spl_stake_pool::state::StakePool;

pub fn initialize(
    stake_pool_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> Result<()> {
    let pool = StakePool {
        last_update_epoch: Clock::get()?.epoch,
        total_lamports: total_liquidity,
        pool_token_supply: mint_total_supply,
        ..Default::default()
    };
    let mut data = stake_pool_account.data.borrow_mut();
    pool.serialize(&mut data.as_mut())?;
    Ok(())
}

pub fn update(
    stake_pool_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> Result<()> {
    let pool = StakePool {
        last_update_epoch: Clock::get()?.epoch,
        total_lamports: total_liquidity,
        pool_token_supply: mint_total_supply,
        ..Default::default()
    };
    let mut data = stake_pool_account.data.borrow_mut();
    pool.serialize(&mut data.as_mut())?;
    Ok(())
}
