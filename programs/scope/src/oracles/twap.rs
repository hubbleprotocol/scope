use crate::{DatedPrice, OracleMappings, OracleTwaps, Price};

pub fn update_twap(
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: Price,
    current_ts: u64,
    current_slot: u64,
) -> crate::Result<Price> {
    // todo: impl this to calculate and update the new twap value
    Ok(Price::default())
}

pub fn get_price(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &OracleTwaps,
    token_index: usize,
) -> DatedPrice {
    let source_index = usize::from(oracle_mappings.twap_source[token_index]);

    let twap = oracle_twaps.twaps[source_index];
    return twap.to_dated_price();
}
