use crate::ScopeError;
use crate::ScopeError::PriceAccountNotExpected;
use crate::{DatedPrice, OracleMappings, OracleTwaps, Price};
use anchor_lang::prelude::*;

use self::utils::{reset_ema_twap, update_ema_twap};

const EMA_1H_DURATION_SECONDS: u64 = 60 * 60;

pub fn validate_price_account(account: &AccountInfo) -> Result<()> {
    if account.key().eq(&crate::id()) {
        return Ok(());
    }

    Err(PriceAccountNotExpected.into())
}

pub fn update_twap(oracle_twaps: &mut OracleTwaps, token: usize, price: &DatedPrice) -> Result<()> {
    let twap = oracle_twaps
        .twaps
        .get_mut(token)
        .ok_or(ScopeError::TwapSourceIndexOutOfRange)?;

    // if there is no previous twap, store the existent
    update_ema_twap(
        twap,
        price.price,
        price.unix_timestamp,
        price.last_updated_slot,
    )?;
    Ok(())
}

pub fn reset_twap(
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    price_ts: u64,
    price_slot: u64,
) -> Result<()> {
    let twap = oracle_twaps
        .twaps
        .get_mut(token)
        .ok_or(ScopeError::TwapSourceIndexOutOfRange)?;
    reset_ema_twap(twap, price, price_ts, price_slot);
    Ok(())
}

pub fn get_price(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &OracleTwaps,
    token: usize,
) -> Result<DatedPrice> {
    let source_index = usize::from(oracle_mappings.twap_source[token]);
    msg!("Get twap price at index {source_index} for tk {token}",);

    let twap = oracle_twaps
        .twaps
        .get(source_index)
        .ok_or(ScopeError::TwapSourceIndexOutOfRange)?;
    Ok(twap.as_dated_price(source_index.try_into().unwrap()))
}

mod utils {
    use decimal_wad::decimal::Decimal;

    use crate::{EmaTwap, Price, ScopeResult};

    use super::*;

    /// Get the adjusted smoothing factor (alpha) based on the time between the last two samples.
    ///
    /// N = number of samples per period
    /// alpha = smoothing factor
    /// alpha = 2 / (1 + N)
    /// N' = adjusted number of samples per period
    /// delta t = time between the last two samples
    /// T = ema period
    /// N' = T/delta t
    pub(super) fn get_adjusted_smoothing_factor(
        last_sample_ts: u64,
        current_sample_ts: u64,
        ema_period_s: u64,
    ) -> ScopeResult<Decimal> {
        let last_sample_delta = current_sample_ts.saturating_sub(last_sample_ts);

        if last_sample_delta >= ema_period_s {
            // Smoothing factor is capped at 1
            Ok(Decimal::one())
        // If the new sample is too close to the last one, we skip it (min 30 seconds)
        } else if last_sample_delta < ema_period_s / 120 {
            Err(ScopeError::TwapSampleTooFrequent)
        } else {
            // For rounding purposes we add half of denominator to the nominator
            let half_sample_delta = last_sample_delta / 2;
            let n = (ema_period_s + half_sample_delta) / last_sample_delta;

            let adjusted_denom = n + 1;

            Ok(Decimal::from(2) / adjusted_denom)
        }
    }

    /// update the EMA  time weighted on how recent the last price is. EMA is calculated as:
    /// EMA = (price * smoothing_factor) + (1 - smoothing_factor) * previous_EMA. The smoothing factor is calculated as: (last_sample_delta / sampling_rate_in_seconds) * (2 / (1 + samples_number_per_period)).
    pub(super) fn update_ema_twap(
        twap: &mut EmaTwap,
        price: Price,
        price_ts: u64,
        price_slot: u64,
    ) -> ScopeResult<()> {
        // Skip update if the price is the same as the last one
        if price_slot > twap.last_update_slot {
            if twap.last_update_slot == 0 {
                twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
            } else {
                let ema_decimal = Decimal::from_scaled_val(twap.current_ema_1h);
                let price_decimal = Decimal::from(price);

                let smoothing_factor = get_adjusted_smoothing_factor(
                    twap.last_update_unix_timestamp,
                    price_ts,
                    EMA_1H_DURATION_SECONDS,
                )?;
                let new_ema = price_decimal * smoothing_factor
                    + (Decimal::one() - smoothing_factor) * ema_decimal;

                twap.current_ema_1h = new_ema
                    .to_scaled_val()
                    .map_err(|_| ScopeError::IntegerOverflow)?;
            }

            twap.last_update_slot = price_slot;
            twap.last_update_unix_timestamp = price_ts;
        }
        Ok(())
    }

    pub(super) fn reset_ema_twap(twap: &mut EmaTwap, price: Price, price_ts: u64, price_slot: u64) {
        twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        twap.last_update_slot = price_slot;
        twap.last_update_unix_timestamp = price_ts;
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
        let price_ts = 100;
        let price_slot = 1;

        reset_ema_twap(&mut twap, test_price, price_ts, price_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
    }

    #[test]
    fn test_reset_default_twap_big_exp() {
        let mut twap = EmaTwap::default();
        let test_price = Price { value: 12, exp: 18 };
        let price_ts = 143;
        let price_slot = 10;

        reset_ema_twap(&mut twap, test_price, price_ts, price_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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
        let price_ts = 143;
        let price_slot = 10;

        reset_ema_twap(&mut twap, test_price, price_ts, price_slot);

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
    }
}

#[cfg(test)]
mod tests_smoothing_factor {
    use crate::assert_fuzzy_eq;

    use super::utils::*;
    use decimal_wad::common::WAD;
    use test_case::test_case;

    #[test_case(60, 60*60, 2.0/61.0)]
    #[test_case(60*60, 60*60, 1.0)]
    #[test_case(4000, 60*60, 1.0)]
    #[test_case(90, 60*60, 2.0/41.0)]
    #[test_case(120, 60*60, 2.0/31.0)]
    #[test_case(600, 60*60, 2.0/7.0)]
    fn test_get_adjusted_smoothing_factor(
        delta_ts: u64,
        ema_period_s: u64,
        expected_smoothing_factor: f64,
    ) {
        let smoothing_factor =
            get_adjusted_smoothing_factor(100, 100 + delta_ts, ema_period_s).unwrap();
        let expected_scaled = (expected_smoothing_factor * WAD as f64) as u128;
        assert_fuzzy_eq!(
            smoothing_factor.to_scaled_val::<u128>().unwrap(),
            expected_scaled,
            expected_scaled / 1000000000
        );
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
        let price_ts = 160;
        let price_slot = 2;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
    }

    #[test]
    fn test_set_initial_price_big_value() {
        let mut twap = EmaTwap::default();

        let test_price = Price {
            value: 100_000_000,
            exp: 0,
        };
        let price_ts = 100;
        let price_slot = 20;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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
        let price_ts = 200;
        let price_slot = 18;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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
        let price_ts = 80;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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
        let price_ts = 80;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert_eq!(
            twap.current_ema_1h,
            Decimal::from(test_price).to_scaled_val().unwrap()
        );
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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

        let price_ts = 100;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) < initial_ema);
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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

        let price_ts = 400;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) < initial_ema);
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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

        let mut twap_with_late_sample = twap_with_early_sample;

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
        )
        .unwrap();
        update_ema_twap(&mut twap_with_late_sample, test_price, late_ts, late_slot).unwrap();

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
        let mut price_ts = 150;
        let mut price_slot = 8;

        let mut previous_twap = twap;
        for _ in 1..10 {
            price_value += 50;
            let test_price = Decimal::from(price_value).into();
            price_ts += 50;
            price_slot += 4;

            update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

            assert!(
                Decimal::from_scaled_val(twap.current_ema_1h)
                    > Decimal::from_scaled_val(previous_twap.current_ema_1h)
            );
            assert_eq!(twap.last_update_slot, price_slot);
            assert_eq!(twap.last_update_unix_timestamp, price_ts);

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

        let price_ts = 100;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) > initial_ema);
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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

        let price_ts = 400;
        let price_slot = 8;

        update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

        assert!(Decimal::from_scaled_val(twap.current_ema_1h) > initial_ema);
        assert_eq!(twap.last_update_slot, price_slot);
        assert_eq!(twap.last_update_unix_timestamp, price_ts);
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
        let mut price_ts = 150;
        let mut price_slot = 8;

        let mut previous_twap = twap;
        for _ in 1..10 {
            price_value += 5;
            let test_price = Decimal::from(price_value).into();
            price_ts += 30;
            price_slot += 60;

            update_ema_twap(&mut twap, test_price, price_ts, price_slot).unwrap();

            assert!(
                Decimal::from_scaled_val(twap.current_ema_1h)
                    > Decimal::from_scaled_val(previous_twap.current_ema_1h)
            );
            assert_eq!(twap.last_update_slot, price_slot);
            assert_eq!(twap.last_update_unix_timestamp, price_ts);

            previous_twap = twap;
        }
    }
}
