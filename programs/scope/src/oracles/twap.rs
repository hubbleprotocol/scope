use decimal_wad::decimal::Decimal;

use crate::{DatedPrice, OracleMappings, OracleTwaps, Price};

use self::utils::{reset_ema_twap, update_ema_twap};

const EMA_1H_SAMPLES_NUMBER: u64 = 30;
const EMA_1H_SAMPLING_RATE_SECONDS: u64 = 60 * 2;

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

    let mut twap = oracle_twaps.twaps[source_index];
    // if there is no previous twap, store the existent
    update_ema_twap(&mut twap, price, current_ts, current_slot);
    return Ok(get_price(oracle_mappings, &oracle_twaps, token));
}

pub fn reset_twap(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    current_ts: u64,
    current_slot: u64,
) {
    let source_index = usize::from(oracle_mappings.twap_source[token]);

    let mut twap = oracle_twaps.twaps[source_index];
    reset_ema_twap(&mut twap, price, current_ts, current_slot)
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

mod utils {
    use decimal_wad::decimal::Decimal;

    use crate::{EmaTwap, Price};

    use super::{EMA_1H_SAMPLES_NUMBER, EMA_1H_SAMPLING_RATE_SECONDS};

    pub(crate) fn update_ema_twap(
        twap: &mut EmaTwap,
        price: Price,
        current_ts: u64,
        current_slot: u64,
    ) {
        twap.last_update_slot = current_slot;
        twap.last_update_unix_timestamp = current_ts;

        if twap.last_update_slot == 0 {
            twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        } else {
            let ema_decimal = Decimal::from(twap.current_ema_1h);
            let price_decimal = Decimal::from(price);
            let smoothing_factor = Decimal::from(2) / Decimal::from(EMA_1H_SAMPLES_NUMBER);
            let weighted_smoothing_factor = (smoothing_factor)
                * Decimal::from(current_ts - twap.last_update_unix_timestamp)
                / Decimal::from(EMA_1H_SAMPLING_RATE_SECONDS);
            let new_ema = price_decimal * weighted_smoothing_factor
                + (Decimal::from(1) - weighted_smoothing_factor) * ema_decimal;

            twap.current_ema_1h = new_ema.to_scaled_val().unwrap();
        }
    }

    pub(crate) fn reset_ema_twap(
        twap: &mut EmaTwap,
        price: Price,
        current_ts: u64,
        current_slot: u64,
    ) {
        twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        twap.last_update_slot = current_slot;
        twap.last_update_unix_timestamp = current_ts;
    }
}
