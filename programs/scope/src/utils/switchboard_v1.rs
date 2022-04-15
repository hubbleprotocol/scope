use crate::{DatedPrice, Price, Result, ScopeError};
use anchor_lang::prelude::*;
use std::cmp::min;

use switchboard_program::{
    get_aggregator, get_aggregator_result, AggregatorState, RoundResult,
    SwitchboardAccountType,
};

const SWITCHBOARD_V1_PRICE_DECIMALS: u32 = 8u32;
const PRICE_MULTIPLIER: f64 = 10u64.pow(SWITCHBOARD_V1_PRICE_DECIMALS) as f64;
const MAX_PRICE_FLOAT: f64 = 10_000_000_000f64; //we choose an arbitrarily high number to do a sanity check and avoid overflow in the multiplication below
const MIN_NUM_SUCCESS: i32 = 3i32;
const MIN_CONFIDENCE_PERCENTAGE: f64 = 2f64;
const CONFIDENCE_FACTOR: f64 = 100f64/MIN_CONFIDENCE_PERCENTAGE;

pub fn get_price(switchboard_feed_info: &AccountInfo) -> Result<DatedPrice> {
    let account_buf = switchboard_feed_info.try_borrow_data()?;
    // first byte type discriminator
    if account_buf[0] != SwitchboardAccountType::TYPE_AGGREGATOR as u8 {
        msg!("switchboard address not of type aggregator");
        return Err(ScopeError::UnexpectedAccount.into());
    }

    let aggregator: AggregatorState = get_aggregator(switchboard_feed_info)?;
    let round_result: RoundResult = get_aggregator_result(&aggregator)?;

    let price_float = round_result.result.ok_or(ScopeError::PriceNotValid)?;

    if price_float >= MAX_PRICE_FLOAT {
        return Err(ScopeError::MathOverflow.into());
    }
    let price: u64 = (price_float * PRICE_MULTIPLIER) as u64;
    let slot: u64 = round_result.round_open_slot.unwrap();

    let max_response = round_result.max_response.ok_or(ScopeError::PriceNotValid)?;
    let min_response = round_result.min_response.ok_or(ScopeError::PriceNotValid)?;
    validate_valid_price(price, slot, aggregator, round_result, price_float, max_response, min_response)
}

pub fn validate_valid_price(
    price: u64,
    slot: u64,
    aggregator: AggregatorState,
    round_result: RoundResult,
    price_float: f64,
    max_response: f64,
    min_response: f64,
) -> Result<DatedPrice> {
    let dated_price = DatedPrice {
        price: Price {
            value: price,
            exp: SWITCHBOARD_V1_PRICE_DECIMALS.into(),
        },
        last_updated_slot: slot,
        ..Default::default()
    };
    if cfg!(feature = "skip_price_validation") {
        return Ok(dated_price);
    };

    validate_min_success(aggregator, round_result)?;
    validate_confidence(price_float, max_response, min_response)?;

    Ok(dated_price)
}

fn validate_min_success(aggregator: AggregatorState, round_result: RoundResult) -> Result<()> {
    let aggregator_min_confirmations = aggregator
        .configs
        .ok_or(ScopeError::PriceNotValid)?
        .min_confirmations
        .ok_or(ScopeError::PriceNotValid)?;

    let min_num_success_for_oracle = min(aggregator_min_confirmations, MIN_NUM_SUCCESS);
    let num_success = round_result.num_success.ok_or(ScopeError::PriceNotValid)?;
    if num_success < min_num_success_for_oracle {
        return Err(ScopeError::PriceNotValid.into());
    };
    Ok(())
}

fn validate_confidence(price: f64, max_response: f64, min_response: f64) -> Result<()> {
    //st_dev_estimate = (max_response - min_response) / 4
    //st_dev_estimate < CONFIDENCE_PERCENTAGE * price/100 <-- THIS IS MEANS VALID PRICE
    // st_dev_estimate * (100/CONFIDENCE_PERCENTAGE) = st_dev_estimate * CONFIDENCE_FACTOR < price
    if max_response < min_response || (max_response - min_response) * CONFIDENCE_FACTOR >= 4f64 * price {
        return Err(ScopeError::PriceNotValid.into());
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::switchboard_v1;
    use switchboard_program::{mod_AggregatorState, AggregatorState, RoundResult};

    fn get_structs_from_min_confirmations_and_num_success(
        min_confirmations: i32,
        num_success: i32,
    ) -> (AggregatorState, RoundResult) {
        let configs = mod_AggregatorState::Configs {
            min_confirmations: Some(min_confirmations),
            ..mod_AggregatorState::Configs::default()
        };
        let aggregator = AggregatorState {
            configs: Some(configs),
            ..AggregatorState::default()
        };
        let round_result = RoundResult {
            num_success: Some(num_success),
            ..RoundResult::default()
        };
        (aggregator, round_result)
    }

    //V1 Tests
    #[test]
    fn test_valid_switchboard_v1_price() {
        let (aggregator, round_result) = get_structs_from_min_confirmations_and_num_success(1, 1);
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1f64).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v1_price_min_1_success_2() {
        let (aggregator, round_result) = get_structs_from_min_confirmations_and_num_success(1, 2);
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1f64).is_ok());
    }

    #[test]
    fn test_valid_switchboard_v1_price_default_min_success() {
        let (aggregator, round_result) = get_structs_from_min_confirmations_and_num_success(4, 3);
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1f64).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_1() {
        let (aggregator, round_result) = get_structs_from_min_confirmations_and_num_success(2, 1);
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1f64).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_2() {
        let (aggregator, round_result) = get_structs_from_min_confirmations_and_num_success(4, 2);
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1f64).is_err());
    }

    //validate_confidence_tests
    fn get_valid_structs() -> (AggregatorState, RoundResult) {
        get_structs_from_min_confirmations_and_num_success(1, 1)
    }

    #[test]
    fn test_valid_switchboard_v1_price_stdev_estimate() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1.079f64, 1f64).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_stdev_estimate() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1.08f64, 1f64).is_err());
    }

    #[test]
    fn test_valid_switchboard_v1_price_stdev_estimate_1() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1.039f64, 0.96f64).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_stdev_estimate_1() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1.04f64, 0.96f64).is_err());
    }


    #[test]
    fn test_valid_switchboard_v1_price_stdev_estimate_2() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 0.921f64).is_ok());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_stdev_estimate_2() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 0.919999f64).is_err());
    }

    #[test]
    fn test_invalid_switchboard_v1_price_wrong_min_max() {
        let (aggregator, round_result) = get_valid_structs();
        assert!(switchboard_v1::validate_valid_price(1, 1, aggregator, round_result, 1f64, 1f64, 1.001f64).is_err());
    }
}
