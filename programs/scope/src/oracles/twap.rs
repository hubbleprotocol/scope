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
        if twap.last_update_slot == 0 {
            twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        } else {
            let ema_decimal = Decimal::from_scaled_val(twap.current_ema_1h);
            let price_decimal = Decimal::from(price);
            println!("ema decimal {}", ema_decimal);
            println!("price_decimal {}", price_decimal);
            let smoothing_factor = Decimal::from(2) / Decimal::from(EMA_1H_SAMPLES_NUMBER);
            let weighted_smoothing_factor = (smoothing_factor)
                * Decimal::from(current_ts - twap.last_update_unix_timestamp)
                / Decimal::from(EMA_1H_SAMPLING_RATE_SECONDS);
            println!(
                "Decimal::from(1) - weighted_smoothing_factor {}",
                Decimal::from(1) - weighted_smoothing_factor
            );
            let new_ema = price_decimal * weighted_smoothing_factor
                + (Decimal::from(1) - weighted_smoothing_factor) * ema_decimal;

            twap.current_ema_1h = new_ema.to_scaled_val().unwrap();
        }

        twap.last_update_slot = current_slot;
        twap.last_update_unix_timestamp = current_ts;
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

#[cfg(test)]
mod tests_reset_twap {
    use decimal_wad::decimal::Decimal;

    use crate::{EmaTwap, Price};

    use super::utils::reset_ema_twap;

    #[test]
    fn test_reset_default_twap() {
        let mut twap = EmaTwap::default();
        let test_price = Price { value: 100, exp: 2 };
        let current_ts = 100;
        let current_slot = 1;

        reset_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_reset_default_twap_big_exp() {
        let mut twap = EmaTwap::default();
        let test_price = Price { value: 12, exp: 18 };
        let current_ts = 143;
        let current_slot = 10;

        reset_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_reset_twap_existing_value() {
        let mut twap = EmaTwap {
            current_ema_1h: 1234_u128,
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price = Price {
            value: 154965,
            exp: 6,
        };
        let current_ts = 143;
        let current_slot = 10;

        reset_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }
}

#[cfg(test)]
mod tests_update_ema_twap {
    use decimal_wad::decimal::Decimal;

    use crate::{EmaTwap, Price};

    use super::utils::update_ema_twap;

    #[test]
    fn test_set_initial_price() {
        let mut twap = EmaTwap::default();

        let test_price = Price { value: 100, exp: 6 };
        let current_ts = 160;
        let current_slot = 2;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_set_initial_price_big_value() {
        let mut twap = EmaTwap::default();

        let test_price = Price {
            value: 100_000_000,
            exp: 0,
        };
        let current_ts = 100;
        let current_slot = 20;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_same_value_as_ema_no_changes() {
        let mut twap = EmaTwap {
            current_ema_1h: Decimal::from(100_000).to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price = Price {
            value: 100_000_000,
            exp: 3,
        };
        let current_ts = 200;
        let current_slot = 18;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_same_value_as_ema_no_changeslow_value() {
        let mut twap = EmaTwap {
            current_ema_1h: Decimal::from(1).to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(1).into();
        let current_ts = 80;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_same_value_as_ema_no_changes_big_price() {
        let mut twap = EmaTwap {
            current_ema_1h: Decimal::from(125_000_000).to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(125_000_000).into();
        let current_ts = 80;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_smaller_value_as_ema_coming_earlier_than_sampling_rate() {
        let initial_ema = Decimal::from(125_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(120_000).into();

        let current_ts = 100;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) < initial_ema);
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_smaller_value_as_ema_coming_later_than_sampling_rate() {
        let initial_ema = Decimal::from(125_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(120_000).into();

        let current_ts = 400;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) < initial_ema);
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_smaller_value_as_ema_coming_later_than_sampling_rate_is_smaller_than_with_value_coming_earlier(
    ) {
        // vefiry that if there is a gap in time and a new sample comes later that sample has a bigger weight
        let initial_ema = Decimal::from(125_000);
        let mut twap_with_early_sample = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let mut twap_with_late_sample = twap_with_early_sample.clone();

        let test_price: Price = Decimal::from(120_000).into();
        let early_ts = 150;
        let early_slot = 8;

        let late_ts = 500;
        let late_slot = 12;

        update_ema_twap(
            &mut twap_with_early_sample,
            test_price,
            early_ts,
            early_slot,
        );
        update_ema_twap(&mut twap_with_late_sample, test_price, late_ts, late_slot);

        assert!(Decimal::from_scaled_val(twap_with_early_sample.current_ema_1h) < initial_ema);
        assert_eq!(twap_with_early_sample.last_update_slot, early_slot);
        assert_eq!(twap_with_early_sample.last_update_unix_timestamp, early_ts);

        assert!(Decimal::from_scaled_val(twap_with_late_sample.current_ema_1h) < initial_ema);
        assert_eq!(twap_with_late_sample.last_update_slot, late_slot);
        assert_eq!(twap_with_late_sample.last_update_unix_timestamp, late_ts);

        assert!(
            Decimal::from_scaled_val(twap_with_late_sample.current_ema_1h)
                < Decimal::from_scaled_val(twap_with_early_sample.current_ema_1h)
        );
    }

    #[test]
    fn test_decreasing_samples_keep_decreasing_twap() {
        // vefiry that if there is a gap in time and a new sample comes later that sample has a bigger weight
        let initial_ema = Decimal::from(5_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let mut price_value = 10_000;
        let mut current_ts = 150;
        let mut current_slot = 8;

        let mut previous_twap = twap.clone();
        for _ in 1..10 {
            price_value += 50;
            let test_price = Decimal::from(price_value).into();
            current_ts += 50;
            current_slot += 4;

            update_ema_twap(&mut twap, test_price, current_ts, current_slot);

            assert!(
                Decimal::from_scaled_val(twap.current_ema_1h)
                    > Decimal::from_scaled_val(previous_twap.current_ema_1h)
            );
            assert_eq!(twap.last_update_slot, current_slot);
            assert_eq!(twap.last_update_unix_timestamp, current_ts);

            previous_twap = twap;
        }
    }

    #[test]
    fn test_price_update_with_bigger_value_as_ema_coming_earlier_than_sampling_rate() {
        let initial_ema = Decimal::from(5_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(20_000).into();

        let current_ts = 100;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) > initial_ema);
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_price_update_with_bigger_value_as_ema_coming_later_than_sampling_rate() {
        let initial_ema = Decimal::from(12_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let test_price: Price = Decimal::from(120_000).into();

        let current_ts = 400;
        let current_slot = 8;

        update_ema_twap(&mut twap, test_price, current_ts, current_slot);

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) > initial_ema);
        assert_eq!(twap.last_update_slot, current_slot);
        assert_eq!(twap.last_update_unix_timestamp, current_ts);
    }

    #[test]
    fn test_increasing_samples_keep_increasing_twap() {
        // vefiry that if there is a gap in time and a new sample comes later that sample has a bigger weight
        let initial_ema = Decimal::from(1_000);
        let mut twap = EmaTwap {
            current_ema_1h: initial_ema.to_scaled_val().unwrap(),
            last_update_slot: 3,
            last_update_unix_timestamp: 50,
            padding: [0_u128; 40],
        };

        let mut price_value = 3_000;
        let mut current_ts = 150;
        let mut current_slot = 8;

        let mut previous_twap = twap.clone();
        for _ in 1..10 {
            price_value -= 5;
            let test_price = Decimal::from(price_value).into();
            current_ts += 10;
            current_slot += 2;

            update_ema_twap(&mut twap, test_price, current_ts, current_slot);

            assert!(
                Decimal::from_scaled_val(twap.current_ema_1h)
                    > Decimal::from_scaled_val(previous_twap.current_ema_1h)
            );
            assert_eq!(twap.last_update_slot, current_slot);
            assert_eq!(twap.last_update_unix_timestamp, current_ts);

            previous_twap = twap;
        }
    }
}
