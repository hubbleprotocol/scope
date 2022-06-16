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
            price_switchboard_desc.mantissa,
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

fn validate_confidence(
    price_mantissa: i128,
    price_scale: u32,
    stdev_mantissa: i128,
    stdev_scale: u32,
) -> std::result::Result<(), ScopeError> {
    // Step 1: compute scaling factor to bring the stdev to the same scale as the price.
    let (scale_op, scale_diff): (&dyn Fn(i128, i128) -> Option<i128>, _) =
        if price_scale >= stdev_scale {
            (
                &i128::checked_mul,
                price_scale.checked_sub(stdev_scale).unwrap(),
            )
        } else {
            (
                &i128::checked_div,
                stdev_scale.checked_sub(price_scale).unwrap(),
            )
        };

    let scaling_factor = 10_i128
        .checked_pow(scale_diff)
        .ok_or(ScopeError::MathOverflow)?;

    // Step 2: multiply the stdev by the CONFIDENCE_FACTOR and apply scaling factor.

    let stdev_x_confidence_factor_scaled = stdev_mantissa
        .checked_mul(CONFIDENCE_FACTOR.into())
        .and_then(|a| scale_op(a, scaling_factor))
        .ok_or(ScopeError::MathOverflow)?;

    if stdev_x_confidence_factor_scaled >= price_mantissa {
        Err(ScopeError::PriceNotValid)
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

    // V2 Standard Deviation Confidence Tests

    // Success cases
    #[test]
    fn test_valid_switchboard_v2_price_stdev_1_point_99_percent() {
        assert!(validate_confidence(100_000, 3, 1999, 3).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_zero() {
        assert!(validate_confidence(100, 3, 0, 15).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p() {
        assert!(validate_confidence(474003240021234567, 15, 4, 0).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v2_price_stdev_1p9percent_std_exp_larger_than_price_exp() {
        assert!(validate_confidence(100_000, 0, 19, 1).is_ok());
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
    fn test_valid_switchboard_v2_price_array_from_onchain() {
        let valid_onchain_exp = [
            (33794739, 6, 117065947069186545442401589, 28),
            (345089113014, 10, 5311207363673122742057483, 28),
            (61950, 5, 5000000000000000000000000, 28),
        ];
        for (value, exp, stdev_val, stdev_exp) in valid_onchain_exp {
            validate_confidence(value, exp, stdev_val, stdev_exp).unwrap();
        }
    }

    proptest! {
        #[test]
        fn test_valid_switchboard_v2_2p_minus_one_unit_proptest(
            mantissa in 0_i128..=850_705_917_302_346_158,
            scale in 0u32..=20,
            stdev_scale_diff in 0u32..=20, // stdev_scale must be greater than scale to store 2% of the price only
        ) {
            let stdev_scale = scale + stdev_scale_diff;
            let stdev_mantissa = (mantissa * 2 * 10_i128.pow(stdev_scale_diff) / 100) - 1;
            validate_confidence(mantissa, scale, stdev_mantissa, stdev_scale).unwrap();
        }
    }

    // Failure cases

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp() {
        let price = 100000;
        let stdev_scale = 3;
        // stdev at 2% of price
        let stdev = price * 10_i128.pow(stdev_scale) * 2 / 100;
        assert_eq!(
            validate_confidence(price, 0, stdev, stdev_scale).unwrap_err(),
            ScopeError::PriceNotValid
        );
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_2percent_std_exp_larger_than_price_exp_2() {
        assert_eq!(
            validate_confidence(100, 2, 20, 3).unwrap_err(),
            ScopeError::PriceNotValid
        );
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent() {
        assert_eq!(
            validate_confidence(100, 0, 2001, 3).unwrap_err(),
            ScopeError::PriceNotValid
        );
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_above_2percent_2() {
        assert_eq!(
            validate_confidence(100, 1, 201, 3).unwrap_err(),
            ScopeError::PriceNotValid
        );
    }

    #[test]
    fn test_invalid_switchboard_v2_price_stdev_higher_than_price() {
        assert_eq!(
            validate_confidence(100, 0, 100001, 3).unwrap_err(),
            ScopeError::PriceNotValid
        );
    }

    proptest! {
        #[test]
        fn test_invalid_switchboard_v2_2p_plus_one_unit_proptest(
            mantissa in 1_i128..=850_705_917_302_346_158,
            scale in 0u32..=20,
            stdev_scale_diff in 3u32..=20, // stdev_scale must be greater than scale to store 2% of the price only
        ) {
            let stdev_scale = scale + stdev_scale_diff;
            // 2% + 1 unit to be just above the 2% threshold
            let stdev_mantissa = mantissa * 2 * 10_i128.pow(stdev_scale_diff) / 100 + 1;
            prop_assert!(matches!(validate_confidence(mantissa, scale, stdev_mantissa, stdev_scale), Err(ScopeError::PriceNotValid)));
        }
    }
}
