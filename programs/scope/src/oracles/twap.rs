use anchor_lang::{err, require};
use decimal_wad::decimal::Decimal;

use crate::{OracleTwaps, Price};

pub fn update_twap(
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    current_ts: u64,
    current_slot: u64,
) -> crate::Result<Price> {
    // todo: impl this to calculate and update the new twap value
    Ok(Price::default())
}
