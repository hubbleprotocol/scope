use std::cmp::{max, min};

use anchor_lang::prelude::*;
use switchboard_v2::AggregatorAccountData;

use crate::{DatedPrice, Price, Result, ScopeError};

const MIN_NUM_SUCCESS: u32 = 3u32;
const MIN_CONFIDENCE_PERCENTAGE: u64 = 2u64;
const CONFIDENCE_FACTOR: u64 = 100 / MIN_CONFIDENCE_PERCENTAGE;

pub fn get_price(switchboard_feed_info: &AccountInfo) -> Result<DatedPrice> {
    let feed = AggregatorAccountData::new(switchboard_feed_info)
        .map_err(|_| ScopeError::SwitchboardV2Error)?;

    let price_switchboard_desc = feed
        .get_result()
        .map_err(|_| ScopeError::SwitchboardV2Error)?;
    if price_switchboard_desc.mantissa < 0 {
        msg!("Switchboard oracle price is negative which is not allowed");
        return Err(ScopeError::PriceNotValid.into());
    }
    let price: u64 = price_switchboard_desc
        .mantissa
        .try_into()
        .map_err(|_| ScopeError::MathOverflow)?;
    let exp: u32 = price_switchboard_desc.scale;
    let slot = feed.latest_confirmed_round.round_open_slot;
    let stdev_mantissa = feed.latest_confirmed_round.std_deviation.mantissa;
    let stdev_scale = feed.latest_confirmed_round.std_deviation.scale;
    validate_valid_price(
        price,
        exp,
        slot,
        feed.min_oracle_results,
        feed.latest_confirmed_round.num_success,
        stdev_mantissa,
        stdev_scale,
    )
}

pub fn validate_valid_price(
    price: u64,
    exp: u32,
    slot: u64,
    min_oracle_results: u32,
    num_success: u32,
    stdev_mantissa: i128,
    stdev_scale: u32,
) -> Result<DatedPrice> {
    let dated_price = DatedPrice {
        price: Price {
            value: price,
            exp: exp.into(),
        },
        last_updated_slot: slot,
        ..Default::default()
    };
    if cfg!(feature = "skip_price_validation") {
        return Ok(dated_price);
    };
    validate_min_success(min_oracle_results, num_success)?;
    validate_confidence(price, exp, stdev_mantissa, stdev_scale)?;

    Ok(dated_price)
}

fn validate_min_success(min_oracle_results: u32, num_success: u32) -> Result<()> {
    let min_num_success_for_oracle = min(min_oracle_results, MIN_NUM_SUCCESS);
    if num_success < min_num_success_for_oracle {
        return Err(ScopeError::PriceNotValid.into());
    };
    Ok(())
}

fn validate_confidence(price: u64, exp: u32, stdev_mantissa: i128, stdev_scale: u32) -> Result<()> {
    let stdev_mantissa: u64 = stdev_mantissa
        .try_into()
        .map_err(|_| ScopeError::MathOverflow)?;
    let scale_op = if exp >= stdev_scale {
        u64::checked_div
    } else {
        u64::checked_mul
    };
    let interval = max(exp, stdev_scale)
        .checked_sub(min(exp, stdev_scale))
        .unwrap(); // This cannot fail

    let scaling_factor = 10u64
        .checked_pow(interval)
        .ok_or(ScopeError::MathOverflow)?;

    let stdev_x_confidence_factor_scaled = stdev_mantissa
        .checked_mul(CONFIDENCE_FACTOR)
        .and_then(|a| scale_op(a, scaling_factor))
        .ok_or(ScopeError::MathOverflow)?;

    if stdev_x_confidence_factor_scaled >= price {
        Err(ScopeError::PriceNotValid.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::switchboard_v2;

    #[test]
    fn test_valid_switchboard_v2_price() {
        assert!(switchboard_v2::validate_valid_price(1, 1, 1, 1, 1, 0, 1).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_min_1_success_2() {
        assert!(switchboard_v2::validate_valid_price(1, 1, 1, 1, 2, 0, 1).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_default_min_success() {
        assert!(switchboard_v2::validate_valid_price(1, 1, 1, 4, 3, 0, 1).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_1() {
        assert!(switchboard_v2::validate_valid_price(1, 1, 1, 2, 1, 0, 1).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_2() {
        assert!(switchboard_v2::validate_valid_price(1, 1, 1, 4, 2, 0, 1).is_err());
    }

    //V2 Standard Deviation Confidence Tests
    #[test]
    fn test_valid_switchboard_v2_price_stdev_1_point_99_percent() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 1999, 0).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_zero() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 0, 15).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_zero_1() {
        assert!(
            switchboard_v2::validate_valid_price(474003240021234567, 15, 1, 1, 1, 0, 1).is_ok()
        );
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9percent_std_exp_larger_than_price_exp() {
        assert!(switchboard_v2::validate_valid_price(100000, 0, 1, 1, 1, 19, 2).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9_std_exp_larger_than_price_exp_8_decimals_diff() {
        assert!(switchboard_v2::validate_valid_price(100_000_000_000, 0, 1, 1, 1, 19, 8).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9_std_exp_larger_than_price_exp_9_decimals_diff() {
        assert!(switchboard_v2::validate_valid_price(100_000_000_000, 0, 1, 1, 1, 1, 9).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp() {
        assert!(switchboard_v2::validate_valid_price(100000, 0, 1, 1, 1, 2, 3).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp_2() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 20, 2).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 2001, 0).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent_2() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 201, 1).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_higher_than_price() {
        assert!(switchboard_v2::validate_valid_price(100, 3, 1, 1, 1, 100001, 0).is_err());
    }
}
