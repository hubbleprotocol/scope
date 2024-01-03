use std::cmp::Ordering;

use crate::ScopeError;
use crate::ScopeError::PriceAccountNotExpected;
use crate::{DatedPrice, OracleMappings, OracleTwaps, Price};
use anchor_lang::prelude::*;
use intbits::Bits;

use self::utils::{reset_ema_twap, update_ema_twap};

const EMA_1H_DURATION_SECONDS: u64 = 60 * 60;
const MIN_SAMPLES_IN_PERIOD: u32 = 10;
const NUM_SUB_PERIODS: usize = 3;
const MIN_SAMPLES_IN_FIRST_AND_LAST_PERIOD: u32 = 1;

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
    clock: &Clock,
) -> Result<DatedPrice> {
    let source_index = usize::from(oracle_mappings.twap_source[token]);
    msg!("Get twap price at index {source_index} for tk {token}",);

    let twap = oracle_twaps
        .twaps
        .get(source_index)
        .ok_or(ScopeError::TwapSourceIndexOutOfRange)?;

    // let current_ts = clock.unix_timestamp.try_into().unwrap();
    // utils::validate_ema(twap, current_ts)?;

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
            let n = Decimal::from(ema_period_s) / last_sample_delta;

            let adjusted_denom = n + Decimal::one();

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
            let mut tracker: EmaTracker = twap.updates_tracker_1h.into();
            tracker.update_tracker(
                EMA_1H_DURATION_SECONDS,
                price_ts,
                twap.last_update_unix_timestamp,
            );
            twap.updates_tracker_1h = tracker.into();
            twap.last_update_slot = price_slot;
            twap.last_update_unix_timestamp = price_ts;
        }
        Ok(())
    }

    pub(super) fn reset_ema_twap(twap: &mut EmaTwap, price: Price, price_ts: u64, price_slot: u64) {
        twap.current_ema_1h = Decimal::from(price).to_scaled_val().unwrap();
        twap.last_update_slot = price_slot;
        twap.last_update_unix_timestamp = price_ts;
        twap.updates_tracker_1h = 0;
    }

    pub(super) fn validate_ema(twap: &EmaTwap, current_ts: u64) -> ScopeResult<()> {
        let mut tracker: EmaTracker = twap.updates_tracker_1h.into();
        tracker.erase_old_samples(
            EMA_1H_DURATION_SECONDS,
            current_ts,
            twap.last_update_unix_timestamp,
        );

        if tracker.get_samples_count() < MIN_SAMPLES_IN_PERIOD {
            return Err(ScopeError::TwapNotEnoughSamplesInPeriod);
        }

        let samples_count_per_subperiods = tracker
            .get_samples_count_per_subperiods::<NUM_SUB_PERIODS>(
                EMA_1H_DURATION_SECONDS,
                twap.last_update_unix_timestamp,
            );

        if samples_count_per_subperiods[0] < MIN_SAMPLES_IN_FIRST_AND_LAST_PERIOD
            || samples_count_per_subperiods[NUM_SUB_PERIODS - 1]
                < MIN_SAMPLES_IN_FIRST_AND_LAST_PERIOD
        {
            return Err(ScopeError::TwapNotEnoughSamplesInPeriod);
        }

        Ok(())
    }
}

/// The sample tracker is a 64 bit number where each bit represents a point in time.
/// We only track one point per time slot. The time slot being the ema_period / 64.
/// The bit is set to 1 if there is a sample at that point in time slot.
#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
#[repr(transparent)]
pub struct EmaTracker(u64);

impl From<EmaTracker> for u64 {
    fn from(tracker: EmaTracker) -> Self {
        tracker.0
    }
}

impl From<u64> for EmaTracker {
    fn from(tracker: u64) -> Self {
        Self(tracker)
    }
}

impl EmaTracker {
    const NB_POINTS: u64 = u64::N_BITS as u64;
    /// Convert a timestamp to a point in the sample tracker
    const fn ts_to_point(ts: u64, ema_period: u64) -> u64 {
        assert!(
            ema_period >= Self::NB_POINTS,
            "EMA period must be bigger than 64 seconds"
        );
        // point_window_size = ema_period / 64
        // points_since_epoch = ts / point_window_size
        // point_index = points_since_epoch % 64
        (ts * Self::NB_POINTS / ema_period) % Self::NB_POINTS
    }

    /// Erase the sample tracker points that are older than the ema_period.
    pub(super) fn erase_old_samples(
        &mut self,
        ema_period: u64,
        current_update_ts: u64,
        last_update_ts: u64,
    ) {
        assert!(
            current_update_ts >= last_update_ts,
            "current_update_ts must be bigger than last_update_ts"
        );
        let sample_tracker = &mut self.0;

        let ts_to_point = |ts| Self::ts_to_point(ts, ema_period);

        let current_point = ts_to_point(current_update_ts);
        // 1. Reset all points up to the current one if needed.
        if last_update_ts + ema_period <= current_update_ts {
            // Reset all points
            *sample_tracker = 0;
        } else {
            let last_update_point = ts_to_point(last_update_ts);
            if last_update_point == current_point {
                // Nothing to reset
                return;
            }

            let first_point_to_clean = (last_update_point + 1) % Self::NB_POINTS; // +1 because we want to reset the point after the last one we updated
            let last_point_to_clean = current_point;

            match first_point_to_clean.cmp(&last_point_to_clean) {
                Ordering::Equal => {
                    // Nothing to reset
                }
                Ordering::Less => {
                    // Reset all points between the first and the last one
                    sample_tracker.set_bits(first_point_to_clean..=last_point_to_clean, 0);
                }
                Ordering::Greater => {
                    sample_tracker.set_bits(first_point_to_clean..Self::NB_POINTS, 0);
                    sample_tracker.set_bits(0..=last_point_to_clean, 0);
                }
            }
        }
    }

    /// Track updates to the EMA
    pub(super) fn update_tracker(
        &mut self,
        ema_period: u64,
        current_update_ts: u64,
        last_update_ts: u64,
    ) {
        // 1. Reset all points up to the current one if needed.
        self.erase_old_samples(ema_period, current_update_ts, last_update_ts);

        // 2. Update the current point.
        let current_point = Self::ts_to_point(current_update_ts, ema_period);
        self.0.set_bit(current_point, true);
    }

    /// Get the number of samples in the last ema_period.
    pub(super) fn get_samples_count(&self) -> u32 {
        self.0.count_ones()
    }

    /// Get the number of samples per each sub-period of the last ema_period.
    /// The number of sub-periods is defined by the const generic parameter N.
    /// The returned array contains the number of samples in each sub-period sorted from the oldest to the newest.
    pub(super) fn get_samples_count_per_subperiods<const N: usize>(
        &self,
        ema_period: u64,
        current_ts: u64,
    ) -> [u32; N] {
        // Sort the points so that the oldest one is the first one.
        let unsorted_points = self.0;
        let current_point = Self::ts_to_point(current_ts, ema_period);
        let pivot_point = (current_point + 1) % Self::NB_POINTS;
        let jonction_point = Self::NB_POINTS - pivot_point;
        let points_oldest = unsorted_points.bits(pivot_point..Self::NB_POINTS);
        let points_newest = unsorted_points.bits(0..pivot_point);
        let sorted_points = points_oldest.with_bits(jonction_point..Self::NB_POINTS, points_newest);

        // Count the number of samples in each sub-period
        let sub_period_size = Self::NB_POINTS / N as u64;
        let mut counts = [0; N];

        let count_in_period = |start_point: u64, end_point: u64| -> u32 {
            sorted_points.bits(start_point..end_point).count_ones()
        };

        let mut start_period_point = 0;
        for count in counts.iter_mut().take(N - 1) {
            let end_period_point = start_period_point + sub_period_size;
            *count = count_in_period(start_period_point, end_period_point);
            start_period_point = end_period_point;
        }

        // The last sub-period might be bigger than the others
        counts[N - 1] = count_in_period(start_period_point, Self::NB_POINTS);

        counts
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
            ..Default::default()
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
    #[test_case(40*60, 60*60, 2.0/2.5)]
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
            expected_scaled / 10000000000000
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

#[cfg(test)]
mod tests_samples_tracker {
    use super::EmaTracker;
    use test_case::test_case;

    #[derive(Debug, Clone, Copy)]
    struct TrackerTester {
        tracker: EmaTracker,
        ema_period: u64,
        last_update_ts: u64,
    }

    impl TrackerTester {
        fn add_update(&mut self, ts: u64) {
            self.tracker
                .update_tracker(self.ema_period, ts, self.last_update_ts);
            self.last_update_ts = ts;
        }

        fn get_samples_count(&self, ts: u64) -> u32 {
            let mut cpy = self.tracker;
            cpy.erase_old_samples(self.ema_period, ts, self.last_update_ts);
            cpy.get_samples_count()
        }

        fn new(ema_period: u64) -> Self {
            Self {
                tracker: EmaTracker::default(),
                ema_period,
                last_update_ts: 0,
            }
        }

        fn get_samples_count_per_subperiods<const N: usize>(&self, ts: u64) -> [u32; N] {
            let mut cpy = self.tracker;
            cpy.erase_old_samples(self.ema_period, ts, self.last_update_ts);
            cpy.get_samples_count_per_subperiods::<N>(self.ema_period, ts)
        }
    }

    #[test_case(64, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] => 11)]
    #[test_case(64, &[63, 68] => 2)]
    #[test_case(64, &[1, 2, 5, 64, 68, 124] => 3)]
    #[test_case(64, &[0, 2, 5, 63, 66, 67] => 4)]
    #[test_case(64*60, &[60, 120, 240, 2400] => 4)]
    #[test_case(60*60, &[60, 120, 240, 2400] => 4)]
    #[test_case(60*60, &[60, 90, 120, 240, 245, 2400] => 4)]
    #[test_case(60*60, &[30, 120, 240] => 3)]
    #[test_case(60*60, &[30, 120, 240, 3631] => 3)]
    #[test_case(60*60, &[30, 120, 240, 3931] => 1)]
    fn test_tracker_count(ema_period_s: u64, samples_ts: &[u64]) -> u32 {
        let mut tester = TrackerTester::new(ema_period_s);
        for ts in samples_ts {
            tester.add_update(*ts);
        }
        tester.get_samples_count(*samples_ts.last().unwrap())
    }

    #[test]
    fn test_tracker_overrun() {
        let mut tester = TrackerTester::new(64);
        for ts in 0..100 {
            tester.add_update(ts);
        }
        assert_eq!(tester.get_samples_count(100), 64);
        assert_eq!(tester.get_samples_count(200), 0);
    }

    fn test_tracker_subperiod_count_helper<const N: usize>(
        ema_period_s: u64,
        samples_ts: &[u64],
    ) -> [u32; N] {
        let mut tester = TrackerTester::new(ema_period_s);
        for ts in samples_ts {
            tester.add_update(*ts);
        }
        tester.get_samples_count_per_subperiods::<N>(*samples_ts.last().unwrap())
    }

    #[test_case(64, &[0, 21, 42, 63] => [1, 1, 1, 1])]
    #[test_case(64, &[0, 21, 42, 60, 63] => [1, 1, 1, 2])]
    #[test_case(64, &[0, 63] => [1, 0, 0, 1])]
    #[test_case(64, &[0, 2, 3, 21, 25, 28, 30, 42, 60, 63] => [3, 4, 1, 2])]
    #[test_case(64, &[0, 2, 3, 21, 25, 28, 30, 42, 114, 115] => [0, 0, 0, 2])]
    fn test_tracker_subperiod_count_4(ema_period_s: u64, samples_ts: &[u64]) -> [u32; 4] {
        test_tracker_subperiod_count_helper(ema_period_s, samples_ts)
    }

    #[test_case(60*60, &[0, 21*60, 42*60] => [1, 1, 1])]
    #[test_case(60*60, &[10*60, 15*60, 25*60, 30*60, 60*60] => [2, 2, 1])]
    #[test_case(60*60, &[0, 10*60, 15*60, 25*60, 30*60, 60*60, 69*60] => [3, 1, 2])]
    fn test_tracker_subperiod_count_3(ema_period_s: u64, samples_ts: &[u64]) -> [u32; 3] {
        test_tracker_subperiod_count_helper(ema_period_s, samples_ts)
    }
}
