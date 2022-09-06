use std::convert::TryInto;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log;

use self::switchboard::*;
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
            return err!(ScopeError::SwitchboardV2Error);
        }
    };

    let last_updated_slot = feed.latest_confirmed_round.round_open_slot;
    let unix_timestamp = feed
        .latest_confirmed_round
        .round_open_timestamp
        .try_into()
        .unwrap();

    Ok(DatedPrice {
        price,
        last_updated_slot,
        unix_timestamp,
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

mod switchboard {

    use std::cell::Ref;

    use anchor_lang::__private::bytemuck::Pod;
    use anchor_lang::__private::bytemuck::{self, Zeroable};
    use rust_decimal::{prelude::FromPrimitive, Decimal};

    use super::*;
    #[zero_copy]
    #[repr(packed)]
    #[derive(Default, Debug, Eq, PartialEq)]
    pub struct SwitchboardDecimal {
        pub mantissa: i128,
        pub scale: u32,
    }

    impl SwitchboardDecimal {
        pub fn new(mantissa: i128, scale: u32) -> SwitchboardDecimal {
            Self { mantissa, scale }
        }
        pub fn from_rust_decimal(d: Decimal) -> SwitchboardDecimal {
            Self::new(d.mantissa(), d.scale())
        }
        pub fn from_f64(v: f64) -> SwitchboardDecimal {
            let dec = Decimal::from_f64(v).unwrap();
            Self::from_rust_decimal(dec)
        }
    }

    #[zero_copy]
    #[repr(packed)]
    #[derive(Debug)]
    pub struct AggregatorAccountData {
        pub name: [u8; 32],
        pub metadata: [u8; 128],
        pub author_wallet: Pubkey,
        pub queue_pubkey: Pubkey,
        // CONFIGS
        // affects update price, shouldnt be changeable
        pub oracle_request_batch_size: u32,
        pub min_oracle_results: u32,
        pub min_job_results: u32,
        // affects update price, shouldnt be changeable
        pub min_update_delay_seconds: u32,
        // timestamp to start feed updates at
        pub start_after: i64,
        pub variance_threshold: SwitchboardDecimal,
        // If no feed results after this period, trigger nodes to report
        pub force_report_period: i64,
        pub expiration: i64,
        //
        pub consecutive_failure_count: u64,
        pub next_allowed_update_time: i64,
        pub is_locked: bool,
        pub _schedule: [u8; 32],
        pub latest_confirmed_round: AggregatorRound,
        pub current_round: AggregatorRound,
        pub job_pubkeys_data: [Pubkey; 16],
        pub job_hashes: [Hash; 16],
        pub job_pubkeys_size: u32,
        // Used to confirm with oracles they are answering what they think theyre answering
        pub jobs_checksum: [u8; 32],
        //
        pub authority: Pubkey,
        pub _ebuf: [u8; 224], // Buffer for future info
    }

    impl AggregatorAccountData {
        pub fn new<'info>(
            switchboard_feed: &'info AccountInfo,
        ) -> Result<Ref<'info, AggregatorAccountData>> {
            let data = switchboard_feed.try_borrow_data()?;

            let mut disc_bytes = [0u8; 8];
            disc_bytes.copy_from_slice(&data[..8]);
            if disc_bytes != AggregatorAccountData::discriminator() {
                msg!("{:?}", disc_bytes);
                return err!(ScopeError::SwitchboardV2Error);
            }

            Ok(Ref::map(data, |data| bytemuck::from_bytes(&data[8..])))
        }

        pub fn get_result(&self) -> std::result::Result<SwitchboardDecimal, ScopeError> {
            if self.min_oracle_results > self.latest_confirmed_round.num_success {
                Err(ScopeError::SwitchboardV2Error)
            } else {
                Ok(self.latest_confirmed_round.result)
            }
        }

        fn discriminator() -> [u8; 8] {
            [217, 230, 65, 101, 201, 162, 27, 125]
        }
    }

    unsafe impl Pod for AggregatorAccountData {}
    unsafe impl Zeroable for AggregatorAccountData {}

    #[zero_copy]
    #[repr(packed)]
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct AggregatorRound {
        // Maintains the number of successful responses received from nodes.
        // Nodes can submit one successful response per round.
        pub num_success: u32,
        pub num_error: u32,
        pub is_closed: bool,
        // Maintains the `solana_program::clock::Slot` that the round was opened at.
        pub round_open_slot: u64,
        // Maintains the `solana_program::clock::UnixTimestamp;` the round was opened at.
        pub round_open_timestamp: i64,
        // Maintains the current median of all successful round responses.
        pub result: SwitchboardDecimal,
        // Standard deviation of the accepted results in the round.
        pub std_deviation: SwitchboardDecimal,
        // Maintains the minimum node response this round.
        pub min_response: SwitchboardDecimal,
        // Maintains the maximum node response this round.
        pub max_response: SwitchboardDecimal,
        // pub lease_key: Option<Pubkey>,
        // Pubkeys of the oracles fulfilling this round.
        pub oracle_pubkeys_data: [Pubkey; 16],
        // pub oracle_pubkeys_size: Option<u32>, IMPLIED BY ORACLE_REQUEST_BATCH_SIZE
        // Represents all successful node responses this round. `NaN` if empty.
        pub medians_data: [SwitchboardDecimal; 16],
        // Current rewards/slashes oracles have received this round.
        pub current_payout: [i64; 16],
        // Optionals do not work on zero_copy. Keep track of which responses are
        // fulfilled here.
        pub medians_fulfilled: [bool; 16],
        // could do specific error codes
        pub errors_fulfilled: [bool; 16],
    }

    #[zero_copy]
    #[repr(packed)]
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct Hash {
        pub data: [u8; 32],
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
