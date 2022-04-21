use crate::{DatedPrice, Price, Result};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, program_pack::Pack, sysvar::Sysvar};

mod last_update;
mod math;
mod reserve;

use reserve::Reserve;

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
        last_updated_slot: Clock::get()?.slot,
        _reserved: Default::default(),
    };

    Ok(dated_price)
}

// Helpers
fn pack_decimal(decimal: math::Decimal, dst: &mut [u8; 16]) {
    *dst = decimal
        .to_scaled_val()
        .expect("Decimal cannot be packed")
        .to_le_bytes();
}

fn unpack_decimal(src: &[u8; 16]) -> math::Decimal {
    math::Decimal::from_scaled_val(u128::from_le_bytes(*src))
}

fn pack_bool(boolean: bool, dst: &mut [u8; 1]) {
    *dst = (boolean as u8).to_le_bytes()
}

fn unpack_bool(src: &[u8; 1]) -> std::result::Result<bool, ProgramError> {
    match u8::from_le_bytes(*src) {
        0 => Ok(false),
        1 => Ok(true),
        _ => {
            msg!("Boolean cannot be unpacked");
            Err(ProgramError::InvalidAccountData)
        }
    }
}
