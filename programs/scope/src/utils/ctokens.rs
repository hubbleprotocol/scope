use crate::{DatedPrice, Price, Result};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;

use solend_program::state::Reserve;

pub fn get_price(solend_reserve_account: &AccountInfo) -> Result<DatedPrice> {
    let reserve = Reserve::unpack(&solend_reserve_account.data.borrow())?;
    let rate = reserve.collateral_exchange_rate()?;

    const DECIMALS: u32 = 15u32;
    const FACTOR: u64 = 10u64.pow(DECIMALS);
    let value = rate.liquidity_to_collateral(FACTOR)?;

    let price = Price {
        value,
        exp: DECIMALS.into(),
    };
    let dated_price = DatedPrice {
        price,
        last_updated_slot: reserve.last_update.slot,
        _reserved: Default::default(),
    };

    Ok(dated_price)
}
