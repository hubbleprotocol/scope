use std::cmp::{max, min};

use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log;
use switchboard_v2::decimal::SwitchboardDecimal;
use switchboard_v2::AggregatorAccountData;

use crate::{DatedPrice, Price, Result, ScopeError};

const MAX_EXPONENT: u32 = 10;

const MIN_CONFIDENCE_PERCENTAGE: u64 = 2u64;
const CONFIDENCE_FACTOR: u64 = 100 / MIN_CONFIDENCE_PERCENTAGE;

pub fn get_price(switchboard_feed_info: &AccountInfo) -> Result<DatedPrice> {
    let feed = AggregatorAccountData::new(switchboard_feed_info)
        .map_err(|_| ScopeError::SwitchboardV2Error)?;

    let price_switchboard_desc = feed.get_result().map_err(|e| {
        msg!(
            "Switchboard v2 get result from feed {} failed with {:#?}",
            switchboard_feed_info.key().to_string(),
            e
        );
        ScopeError::SwitchboardV2Error
    })?;

    let price: Price = price_switchboard_desc.try_into()?;

    if !cfg!(feature = "skip_price_validation") {
        let stdev_mantissa = feed.latest_confirmed_round.std_deviation.mantissa;
        let stdev_scale = feed.latest_confirmed_round.std_deviation.scale;
        if validate_confidence(
            price.value,
            price_switchboard_desc.scale,
            stdev_mantissa,
            stdev_scale,
        )
        .is_err()
        {
            // Using sol log because with exactly 5 parameters, msg! expect u64s.
            sol_log(&format!("Validation of confidence interval for switchboard v2 feed {} failed. Price: {:?}, stdev_mantissa: {:?}, stdev_scale: {:?}",
             switchboard_feed_info.key(),
              price,
              stdev_mantissa,
              stdev_scale));
            return Err(ScopeError::SwitchboardV2Error.into());
        }
    };

    let last_updated_slot = feed.latest_confirmed_round.round_open_slot;

    Ok(DatedPrice {
        price,
        last_updated_slot,
        ..Default::default()
    })
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

impl TryFrom<SwitchboardDecimal> for Price {
    type Error = ScopeError;

    fn try_from(sb_decimal: SwitchboardDecimal) -> std::result::Result<Self, Self::Error> {
        if sb_decimal.mantissa < 0 {
            msg!("Switchboard v2 oracle price feed is negative");
            return Err(ScopeError::PriceNotValid);
        }
        let (exp, value) = if sb_decimal.scale > MAX_EXPONENT {
            // exp is capped. Remove the extra digits from the mantissa.
            let exp_diff = sb_decimal
                .scale
                .checked_sub(MAX_EXPONENT)
                .ok_or(ScopeError::MathOverflow)?;
            let factor = 10_i128
                .checked_pow(exp_diff)
                .ok_or(ScopeError::MathOverflow)?;
            // Loss of precision here is expected.
            let value = sb_decimal.mantissa / factor;
            (MAX_EXPONENT, value)
        } else {
            (sb_decimal.scale, sb_decimal.mantissa)
        };
        let exp: u64 = exp.into();
        let value: u64 = value.try_into().map_err(|_| ScopeError::IntegerOverflow)?;
        Ok(Price { value, exp })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const U64_MAX: i128 = std::u64::MAX as i128;

    proptest! {
        #[test]
        fn price_from_valid_switchboard_decimal(
            mantissa in 0_i128..=U64_MAX,
            scale in 0u32..=10,
        ) {
            let sb_decimal = SwitchboardDecimal {
                mantissa,
                scale,
            };
            let price = Price::try_from(sb_decimal).unwrap();
            prop_assert_eq!(price.value, mantissa as u64);
            prop_assert_eq!(price.exp, scale as u64);
        }
    }

    proptest! {
        #[test]
        fn price_from_caped_switchboard_decimal(
            mantissa in 0_i128..=U64_MAX,
            scale in 11u32..=30,
        ) {
            let sb_decimal = SwitchboardDecimal {
                mantissa,
                scale,
            };
            let price = Price::try_from(sb_decimal).unwrap();
            prop_assert_eq!(price.exp, 10);

            let exp_diff = scale.checked_sub(10).unwrap();
            let scaled_up_value = price.value as i128 * 10_i128.pow(exp_diff);

            let mantissa_diff = mantissa.checked_sub(scaled_up_value).unwrap();
            prop_assert!(mantissa_diff < 10_i128.pow(exp_diff + 1));
        }
    }

    #[test]
    fn test_valid_switchboard_v2_price() {
        assert!(validate_confidence(1, 1, 0, 1).is_ok());
    }

    //V2 Standard Deviation Confidence Tests
    #[test]
    fn test_valid_switchboard_v2_price_stdev_1_point_99_percent() {
        assert!(validate_confidence(100, 3, 1999, 0).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_zero() {
        assert!(validate_confidence(100, 3, 0, 15).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_zero_1() {
        assert!(validate_confidence(474003240021234567, 15, 0, 1).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9percent_std_exp_larger_than_price_exp() {
        assert!(validate_confidence(100000, 0, 19, 2).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9_std_exp_larger_than_price_exp_8_decimals_diff() {
        assert!(validate_confidence(100_000_000_000, 0, 19, 8).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9_std_exp_larger_than_price_exp_9_decimals_diff() {
        assert!(validate_confidence(100_000_000_000, 0, 1, 9).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp() {
        assert!(validate_confidence(100000, 0, 2, 3).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp_2() {
        assert!(validate_confidence(100, 3, 20, 2).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent() {
        assert!(validate_confidence(100, 3, 2001, 0).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent_2() {
        assert!(validate_confidence(100, 3, 201, 1).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_higher_than_price() {
        assert!(validate_confidence(100, 3, 100001, 0).is_err());
    }
}
