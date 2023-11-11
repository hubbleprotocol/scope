use decimal_wad::decimal::Decimal;

use crate::{DatedPrice, OracleMappings, OracleTwaps, Price};

pub fn update_twap(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    current_ts: u64,
    current_slot: u64,
) -> crate::Result<DatedPrice> {
    // todo: impl this to calculate and update the new twap value

    let source_index = usize::from(oracle_mappings.twap_source[token]);

    let twap = oracle_twaps.twaps[source_index];
    // if there is no previous twap, store the existent
    if twap.last_update_slot == 0 {
        twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        twap.last_update_slot = current_slot;
        twap.last_update_unix_timestamp = current_ts;

        return Ok(get_price(oracle_mappings, &oracle_twaps, token));
    } else {
    }
}

pub fn get_price(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &OracleTwaps,
    token: usize,
) -> DatedPrice {
    let source_index = usize::from(oracle_mappings.twap_source[token]);

    let twap = oracle_twaps.twaps[source_index];
    return twap.to_dated_price();
}
