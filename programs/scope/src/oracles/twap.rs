use anchor_lang::{err, require};
use solana_program::clock::{self, Clock};

use crate::{
    DatedPrice, OracleMappings, OracleTwaps, Price, ScopeError, TWAP_INTERVAL_SECONDS, TWAP_NUM_OBS,
};

#[cfg(test)]
use crate::TwapBuffer;

pub fn store_observation(
    oracle_twaps: &mut OracleTwaps,
    token: usize,
    price: u128,
    current_ts: u64,
    current_slot: u64,
) -> crate::Result<()> {
    require!(current_ts > 0, ScopeError::BadTimestamp);
    require!(current_slot > 0, ScopeError::BadSlot);

    let twap_buffer = &mut oracle_twaps.twap_buffers[token];
    let curr_index = twap_buffer.curr_index as usize;

    // If the buffer is uninitialized, we directly store the observation at index 0.
    if twap_buffer.last_update_slot == 0 {
        twap_buffer.observations[0].observation = price;
        twap_buffer.observations[0].unix_timestamp = current_ts;
        twap_buffer.last_update_slot = current_slot;
        twap_buffer.curr_index = 0; // Explicitly set to 0 for clarity.
        return Ok(());
    }

    // Determine if the new timestamp is valid for storing a new observation.
    let last_timestamp = twap_buffer.last_update_unix_timestamp;
    let last_slot = twap_buffer.last_update_slot;

    require!(last_timestamp <= current_ts, ScopeError::BadTimestamp);
    require!(last_slot <= current_slot, ScopeError::BadSlot);

    // Check if enough time has elapsed since the last timestamp.
    if current_ts.saturating_sub(last_timestamp) < TWAP_INTERVAL_SECONDS {
        return Ok(()); // Not enough time has passed.
    }

    // Calculate the next index.
    let next_index = (curr_index + 1) % TWAP_NUM_OBS;

    // Update the TWAP buffer with the new observation.
    twap_buffer.observations[next_index].observation = price;
    twap_buffer.observations[next_index].unix_timestamp = current_ts;
    twap_buffer.curr_index = next_index as u64;

    Ok(())
}

pub fn get_twap_from_observations(
    oracle_mappings: &OracleMappings,
    oracle_twaps: &OracleTwaps,
    twap_buffer_source: usize,
    twap_duration_seconds: u64,
    current_unix_timestamp: u64,
    min_twap_observations: usize,
) -> crate::Result<DatedPrice> {
    // Basically iterate through the observations of the [token] from OracleTwaps
    // and calculate twap up to a certain point in time, given how far back this current
    // OracleTwap twap duration is
    // TODO: add constraints about min num observations

    let oldest_ts = current_unix_timestamp - twap_duration_seconds;

    let twap_buffer = oracle_twaps.twap_buffers[twap_buffer_source];

    let (mut running_index, mut twap, mut num_obs, mut max_exp) =
        (twap_buffer.curr_index as usize, 0, 0, 0);
    loop {
        let obs = twap_buffer.observations[running_index].observation;
        // let ts = twap_buffer.unix_timestamps[running_index];
        let ts = twap_buffer.observations[running_index].unix_timestamp;

        if ts < oldest_ts || ts == 0 || num_obs >= TWAP_NUM_OBS {
            break;
        }

        twap += obs; // * 10u64.pow(obs.exp as u32);
        num_obs += 1;
        // max_exp = max_exp.max(obs.exp);
        running_index = (running_index + TWAP_NUM_OBS - 1) % TWAP_NUM_OBS;
    }

    println!(
        "Mint twap obs {} num obs {}",
        min_twap_observations, num_obs
    );
    if min_twap_observations > num_obs {
        return err!(ScopeError::NotEnoughTwapObservations);
    }

    twap /= num_obs as u128;
    // twap /= 10u64.pow(max_exp as u32);

    Ok(DatedPrice {
        price: crate::Price {
            value: twap as u64, // todo: siviu fix this
            exp: max_exp,
        },
        last_updated_slot: twap_buffer.last_update_slot,
        unix_timestamp: twap_buffer.last_update_unix_timestamp,
        _reserved: [0; 2],
        _reserved2: [0; 3],
        index: 0,
    })
}

// Helper function to create a populated TwapBuffer for testing.
#[cfg(test)]
fn create_populated_twap_buffer() -> TwapBuffer {
    create_partially_empty_twap_buffer(0)
}

// Helper function to create a default OracleTwaps for testing.
#[cfg(test)]
fn create_default_oracle_twaps() -> OracleTwaps {
    use solana_program::pubkey::Pubkey;

    use crate::MAX_ENTRIES;

    OracleTwaps {
        oracle_prices: Pubkey::new_unique(),
        tokens_metadata: Pubkey::new_unique(),
        twap_buffers: [create_partially_empty_twap_buffer(TWAP_NUM_OBS); MAX_ENTRIES],
    }
}

// Helper to create a partially empty TwapBuffer
#[cfg(test)]
fn create_partially_empty_twap_buffer(empty_slots: usize) -> TwapBuffer {
    use crate::TwapEntry;

    let mut buffer = TwapBuffer {
        curr_index: 0,
        observations: [TwapEntry::default(); TWAP_NUM_OBS],
        last_update_slot: 0,
        last_update_unix_timestamp: 0,
        padding: [0; 65536],
    };

    // Fill up to `empty_slots` with non-zero timestamps
    let start_ts = 100;
    let start_slot = 100;
    for i in 0..TWAP_NUM_OBS - empty_slots {
        buffer.observations[i].observation = (i + 1) as u128;
        buffer.observations[i].unix_timestamp = start_ts + TWAP_INTERVAL_SECONDS * i as u64;
        buffer.last_update_slot = start_slot + i as u64;
    }

    if empty_slots == TWAP_NUM_OBS {
        // Empty buffer
        buffer.curr_index = 0;
    } else {
        // The current index points to the last non-empty slot
        buffer.curr_index = TWAP_NUM_OBS as u64 - empty_slots as u64 - 1;
    }

    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test adding an observation with proper interval elapsed.
    #[test]
    fn test_add_observation_proper_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS; // Assuming the initial timestamp was 0.
        let current_slot = 1;

        // Act
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();

        // Assert
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].unix_timestamp,
            current_ts
        );
        assert_eq!(oracle_twaps.twap_buffers[0].last_update_slot, current_slot);
        assert_eq!(oracle_twaps.twap_buffers[0].curr_index, 0); // todo: shouldn't this be 1?
    }
}

#[cfg(test)]
mod boundary_tests {
    use crate::MAX_ENTRIES;

    use super::*;

    #[test]
    fn test_store_observation_at_first_index() {
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS + 1; // Ensure the timestamp interval has passed.
        let current_slot = 1;

        let mut oracle_twaps = create_default_oracle_twaps();
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );
    }

    #[test]
    fn test_store_observation_at_last_index() {
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS + 1; // Ensure the timestamp interval has passed.
        let current_slot = 1;

        let mut oracle_twaps = create_default_oracle_twaps();
        let last_index = MAX_ENTRIES - 1;
        store_observation(
            &mut oracle_twaps,
            last_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();
        assert_eq!(
            oracle_twaps.twap_buffers[last_index].observations[0].observation,
            test_price
        );
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_store_observation_at_out_of_bounds_index() {
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS + 1; // Ensure the timestamp interval has passed.
        let current_slot = 1;

        let mut oracle_twaps = create_default_oracle_twaps();
        store_observation(
            &mut oracle_twaps,
            MAX_ENTRIES,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();
    }

    #[test]
    fn test_store_observation_at_boundary_after_wrap() {
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS + 1; // Ensure the timestamp interval has passed.
        let current_slot = 1;

        let mut oracle_twaps = create_default_oracle_twaps();
        // Simulate the situation where curr_index has wrapped around
        oracle_twaps.twap_buffers[0].curr_index = TWAP_NUM_OBS as u64 - 1;
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();
        // Expect that the observation is stored at index 0 after the wrap
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );
    }

    #[test]
    fn test_store_observation_multiple_boundaries() {
        let test_price = 100;
        let current_ts = TWAP_INTERVAL_SECONDS + 1; // Ensure the timestamp interval has passed.
        let current_slot = 1;

        let mut oracle_twaps = create_default_oracle_twaps();
        // Simulate the situation where curr_index has wrapped around
        oracle_twaps.twap_buffers[0].curr_index = TWAP_NUM_OBS as u64 - 1;
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );

        // Now add another observation to ensure curr_index correctly wraps and stores
        let new_price = 200;

        println!("Before: {:?}", oracle_twaps.twap_buffers[0].observations);
        store_observation(
            &mut oracle_twaps,
            0,
            new_price,
            current_ts + TWAP_INTERVAL_SECONDS,
            current_slot + 1,
        )
        .unwrap();
        println!("After: {:?}", oracle_twaps.twap_buffers[1].observations);
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[1].observation,
            new_price
        );
    }
}

#[cfg(test)]
mod empty_buffer_tests {
    use crate::{TwapBuffer, TwapEntry};

    use super::*;

    // Helper to create an empty TwapBuffer
    fn create_empty_twap_buffer() -> TwapBuffer {
        TwapBuffer {
            curr_index: 0,
            observations: [TwapEntry::default(); TWAP_NUM_OBS],
            last_update_slot: 0,
            last_update_unix_timestamp: 0,
            padding: [0; 65536],
        }
    }

    #[test]
    fn test_store_observation_with_empty_buffer() {
        let mut oracle_twaps = create_default_oracle_twaps();
        // Ensure the buffer is empty
        oracle_twaps.twap_buffers[0] = create_empty_twap_buffer();

        let test_price = 100;
        let current_ts = 1; // Non-zero timestamp
        let current_slot = 1;

        // Act
        let _ = store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot);

        // Assert that the observation was stored correctly at the first index
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].unix_timestamp,
            current_ts
        );
        assert_eq!(oracle_twaps.twap_buffers[0].last_update_slot, current_slot);
        assert_eq!(oracle_twaps.twap_buffers[0].curr_index, 0);
    }

    #[test]
    fn test_store_observation_does_not_override_when_buffer_empty_and_interval_not_passed() {
        let mut oracle_twaps = create_default_oracle_twaps();
        // Ensure the buffer is empty
        oracle_twaps.twap_buffers[0] = create_empty_twap_buffer();

        let test_price = 100;
        let current_ts = 1; // Non-zero timestamp
        let current_slot = 1;

        // Act
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();

        // Try to store another observation with the same timestamp, which should not override the previous one
        let new_price = Price { value: 200, exp: 2 };
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();

        // Assert that the first observation remains unchanged
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[0].unix_timestamp,
            current_ts
        );
        assert_eq!(oracle_twaps.twap_buffers[0].last_update_slot, current_slot);
        assert_eq!(oracle_twaps.twap_buffers[0].curr_index, 0);
    }

    // Additional tests could include variations where the buffer is partially empty,
    // for instance, with only some of the timestamps set to zero. This would test
    // that the initialization logic correctly identifies the first zero timestamp.
}

#[cfg(test)]
mod partial_empty_buffer_tests {

    use super::*;

    #[test]
    fn test_store_observation_with_partially_empty_buffer() {
        let mut oracle_twaps = create_default_oracle_twaps();
        // Create a buffer with the last 5 slots empty
        oracle_twaps.twap_buffers[0] = create_partially_empty_twap_buffer(5);

        let current_index = oracle_twaps.twap_buffers[0].curr_index as usize;
        let test_price = 999;
        let current_ts = oracle_twaps.twap_buffers[0].observations[current_index].unix_timestamp
            + TWAP_INTERVAL_SECONDS; // Ensure we're past the last non-empty slot's timestamp
        let current_slot = oracle_twaps.twap_buffers[0].last_update_slot;

        // Act
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();

        // Assert that the observation was stored correctly at the next index
        let expected_index = TWAP_NUM_OBS - 5; // The first empty slot
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[expected_index].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[0].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_eq!(oracle_twaps.twap_buffers[0].last_update_slot, current_slot);
        assert_eq!(
            oracle_twaps.twap_buffers[0].curr_index,
            expected_index as u64
        );
    }

    #[test]
    fn test_store_observation_does_not_update_when_interval_not_passed_in_partially_empty_buffer() {
        let mut oracle_twaps = create_default_oracle_twaps();
        // Create a buffer with the last 5 slots empty
        oracle_twaps.twap_buffers[0] = create_partially_empty_twap_buffer(5);

        println!("Observations {:?}", oracle_twaps.twap_buffers[0]);

        let current_index = oracle_twaps.twap_buffers[0].curr_index as usize;
        let test_price = 999;
        let current_ts =
            oracle_twaps.twap_buffers[0].observations[current_index].unix_timestamp + 1; // One second past the second-to-last non-empty slot's timestamp
        let current_slot = 10000;

        // Act
        store_observation(&mut oracle_twaps, 0, test_price, current_ts, current_slot).unwrap();

        // Assert that the observation has not been updated since the interval has not passed
        let expected_index = TWAP_NUM_OBS - 6; // Second-to-last non-empty slot
        assert_ne!(
            oracle_twaps.twap_buffers[0].observations[expected_index].observation,
            test_price
        );
        assert_ne!(
            oracle_twaps.twap_buffers[0].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_ne!(oracle_twaps.twap_buffers[0].last_update_slot, current_slot);
    }
}

#[cfg(test)]
mod extended_partial_empty_buffer_tests {
    use super::*;

    // Test where current timestamp is exactly on the interval boundary
    #[test]
    fn test_store_observation_on_interval_boundary() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 3; // Arbitrary index for the test
        oracle_twaps.twap_buffers[buffer_index] = create_partially_empty_twap_buffer(5);

        let test_price = 12345;
        // Simulate a timestamp exactly at the interval after the last non-zero timestamp
        let last_non_zero_index = oracle_twaps.twap_buffers[buffer_index].curr_index; // The last entry before the empty slots
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations
            [last_non_zero_index as usize]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS;
        let current_slot = 20000; // Next slot to be filled

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that the observation is stored exactly at the interval boundary
        let expected_index = TWAP_NUM_OBS - 5; // The first empty slot
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].last_update_slot,
            current_slot
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            expected_index as u64
        );
    }

    // Test where the buffer needs to wrap around
    #[test]
    fn test_store_observation_with_wraparound() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 5; // Arbitrary index for the test
                              // Create a buffer with all but the last slot filled
        oracle_twaps.twap_buffers[buffer_index] = create_partially_empty_twap_buffer(0);

        let test_price = 67890;
        // Simulate a timestamp far enough in the future to cause a wraparound
        let current_ts = 1000 + TWAP_INTERVAL_SECONDS * TWAP_NUM_OBS as u64 + 1;
        let current_slot = 1000 + TWAP_NUM_OBS as u64 + 1; // Simulate a future slot

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that the observation wraps around to the start of the buffer
        let expected_index = 0;
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].last_update_slot,
            current_slot
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            expected_index as u64
        );
    }

    #[test]
    fn test_store_observation_at_the_end() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 5; // Arbitrary index for the test
                              // Create a buffer with all but the last slot filled
        oracle_twaps.twap_buffers[buffer_index] = create_partially_empty_twap_buffer(1);

        let test_price = 67890;
        // Simulate a timestamp far enough in the future to cause a wraparound
        let current_ts = 1000 + TWAP_INTERVAL_SECONDS * TWAP_NUM_OBS as u64 + 1;
        let current_slot = 1000 + TWAP_NUM_OBS as u64 + 1; // Simulate a future slot

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that the observation wraps around to the start of the buffer
        let expected_index = TWAP_NUM_OBS - 1;
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].last_update_slot,
            current_slot
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            expected_index as u64
        );
    }
}

#[cfg(test)]
mod additional_tests_partial_empty_buffer_tests {
    use super::*;

    // Test that entries are overwritten correctly once the buffer is full
    #[test]
    fn test_overwrite_when_buffer_full() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 7; // Arbitrary index for the test
                              // Create a full buffer with no empty slots
        oracle_twaps.twap_buffers[buffer_index] = create_populated_twap_buffer();

        let test_price = 10000;
        // Simulate a timestamp that would cause an overwrite
        let current_ts = TWAP_INTERVAL_SECONDS * (TWAP_NUM_OBS as u64 + 1);
        let current_slot = 10000 + TWAP_NUM_OBS as u64 + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that the oldest observation is overwritten
        let expected_index = 0; // Since buffer was full, we overwrite the second oldest entry
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
            test_price
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].last_update_slot,
            current_slot
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            expected_index as u64
        );
    }

    // Test that no update occurs if the current timestamp is not enough past the last timestamp
    #[test]
    fn test_no_update_for_close_timestamps() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 2; // Arbitrary index for the test
        oracle_twaps.twap_buffers[buffer_index] = create_partially_empty_twap_buffer(10);

        let last_filled_index = oracle_twaps.twap_buffers[buffer_index].curr_index as usize;
        let test_price = 20000;
        // Simulate a timestamp that is not enough past the last timestamp
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[last_filled_index]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS / 2;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 10;

        // Remember current state for assertions
        let original_state = oracle_twaps.twap_buffers[buffer_index].clone();

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            test_price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that no new observation is stored
        assert_eq!(oracle_twaps.twap_buffers[buffer_index], original_state);
    }

    // Test updating at various points in the buffer
    #[test]
    fn test_various_update_points_in_buffer() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 1; // Arbitrary index for the test
                              // Create a partially filled buffer
        oracle_twaps.twap_buffers[buffer_index] = create_partially_empty_twap_buffer(20);

        let start_filled_index = TWAP_NUM_OBS - 21; // Start index for filled slots
        let test_price = 30000;
        let mut current_ts =
            oracle_twaps.twap_buffers[buffer_index].observations[start_filled_index].unix_timestamp;
        let mut current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot;

        for i in 1..21 {
            current_ts += TWAP_INTERVAL_SECONDS;
            current_slot += 1;

            // Act
            store_observation(
                &mut oracle_twaps,
                buffer_index,
                test_price,
                current_ts,
                current_slot,
            )
            .unwrap();

            // Assert that the new observation is stored at the correct index
            let expected_index = (start_filled_index + i) % TWAP_NUM_OBS;
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
                test_price
            );
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
                current_ts
            );
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].last_update_slot,
                current_slot
            );
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].curr_index,
                expected_index as u64
            );
        }
    }

    // Utility functions for creating buffers in certain states would be implemented here.
    // ...
}

#[cfg(test)]
mod time_interval_checks {
    use crate::TwapEntry;

    use super::*;

    // Test that an observation is stored when the interval is exactly the defined TWAP interval
    #[test]
    fn store_when_interval_equals_twap_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        oracle_twaps.twap_buffers[buffer_index] = create_initial_twap_buffer_with_one_entry();
        let price = 10000;

        let last_index = oracle_twaps.twap_buffers[buffer_index].curr_index as usize;
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[last_index]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[1].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[1].observation,
            price
        );

        // assert incremented
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            (last_index + 1) as u64
        )
    }

    // Test that an observation is stored when the interval is more than the defined TWAP interval
    #[test]
    fn store_when_interval_greater_than_twap_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        oracle_twaps.twap_buffers[buffer_index] = create_initial_twap_buffer_with_one_entry();
        let price = 20000;
        // Adding extra time to ensure we are beyond the TWAP interval
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[0].unix_timestamp
            + TWAP_INTERVAL_SECONDS
            + 10;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[1].unix_timestamp,
            current_ts
        );
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[1].observation,
            price
        );
    }

    // Test that an observation is not stored when the interval is less than the defined TWAP interval
    #[test]
    fn no_store_when_interval_less_than_twap_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        oracle_twaps.twap_buffers[buffer_index] = create_initial_twap_buffer_with_one_entry();
        let price = 30000;
        // Subtracting time to ensure we are within the TWAP interval
        let last_idx = oracle_twaps.twap_buffers[buffer_index].curr_index as usize;
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[last_idx]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS
            - 10;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert that no new observation was stored
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[1],
            TwapEntry::default()
        );
    }

    // Utility function for creating a TwapBuffer with a single initial entry
    fn create_initial_twap_buffer_with_one_entry() -> TwapBuffer {
        let mut buffer = TwapBuffer::default();
        buffer.observations[0].observation = 50000;
        buffer.observations[0].unix_timestamp = 1_000_000; // Arbitrary past timestamp
        buffer.last_update_slot = 100; // Arbitrary slot number
        buffer
    }
}

#[cfg(test)]
mod index_incrementation_tests {
    use super::*;

    // Test that the index increments correctly when it is not at the end of the buffer
    #[test]
    fn index_increments_normally() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;

        oracle_twaps.twap_buffers[buffer_index] =
            create_initial_twap_buffer_with_sequential_entries(TWAP_NUM_OBS / 2);

        let last_index = oracle_twaps.twap_buffers[buffer_index].curr_index;
        let price = 10000;

        println!(
            "Initial index: {:?} vs {} TWAP_NUM_OBS: {}, TWAP_NUM_OBS / 2 = {}",
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            last_index,
            TWAP_NUM_OBS,
            TWAP_NUM_OBS / 2
        );
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[last_index as usize]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            last_index + 1
        );
    }

    // Test that the index wraps around to 0 when it reaches the end of the buffer
    #[test]
    fn index_wraps_around_at_end_of_buffer() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_twap_buffer = create_initial_twap_buffer_with_sequential_entries(TWAP_NUM_OBS);

        oracle_twaps.twap_buffers[buffer_index] = initial_twap_buffer;
        let last_index = (TWAP_NUM_OBS - 1) as u64;
        let price = 20000;
        let current_ts = oracle_twaps.twap_buffers[buffer_index].observations[last_index as usize]
            .unix_timestamp
            + TWAP_INTERVAL_SECONDS;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();

        // Assert
        assert_eq!(oracle_twaps.twap_buffers[buffer_index].curr_index, 0);
    }

    // Test that the index correctly increments when storing multiple observations sequentially
    #[test]
    fn index_increments_over_multiple_observations() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_twap_buffer = create_initial_twap_buffer_with_sequential_entries(2);

        oracle_twaps.twap_buffers[buffer_index] = initial_twap_buffer;
        let mut current_ts = oracle_twaps.twap_buffers[buffer_index].observations[0].unix_timestamp
            + TWAP_INTERVAL_SECONDS;
        let mut current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        let price = 30000;

        println!(
            "Initial index: {:?}",
            oracle_twaps.twap_buffers[buffer_index].curr_index
        );

        // Act & Assert loop
        for i in 2..TWAP_NUM_OBS as u64 + 1 {
            store_observation(
                &mut oracle_twaps,
                buffer_index,
                price,
                current_ts,
                current_slot,
            )
            .unwrap();

            let expected_index = (i - 1) % TWAP_NUM_OBS as u64;
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].curr_index,
                expected_index
            );

            current_ts += TWAP_INTERVAL_SECONDS;
            current_slot += 1;
        }
    }

    // Utility function for creating a TwapBuffer with a certain number of sequential entries
    fn create_initial_twap_buffer_with_sequential_entries(num_entries: usize) -> TwapBuffer {
        let mut buffer = TwapBuffer::default();

        let start_ts = 100;
        let start_slot = 100;
        for i in 0..num_entries {
            buffer.observations[i].observation = (i * 10000) as u128;
            buffer.observations[i].unix_timestamp = start_ts + i as u64 * TWAP_INTERVAL_SECONDS;
            buffer.last_update_slot = start_slot + i as u64;
        }

        buffer.curr_index = match num_entries {
            0 => 0,
            1 => 1,
            _ => (num_entries - 1) as u64 % TWAP_NUM_OBS as u64,
        };

        buffer
    }
}

#[cfg(test)]
mod non_chronological_timestamps_tests {
    use crate::TwapEntry;

    use super::*;

    // Test that a newer observation with a timestamp earlier than the previous one is ignored
    #[test]
    fn ignores_newer_observation_with_earlier_timestamp() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_twap_buffer = create_initial_twap_buffer_with_sequential_entries(2);

        oracle_twaps.twap_buffers[buffer_index] = initial_twap_buffer;
        let last_index = oracle_twaps.twap_buffers[buffer_index].curr_index;
        let last_timestamp = oracle_twaps.twap_buffers[buffer_index].observations
            [last_index as usize]
            .unix_timestamp;
        let out_of_order_timestamp = last_timestamp - 10; // Earlier than the last timestamp
        let price = 40000;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act
        let res = store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            out_of_order_timestamp,
            current_slot,
        );
        assert!(res.is_err());

        // Assert
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            last_index
        ); // Index should not increment
        assert_ne!(
            oracle_twaps.twap_buffers[buffer_index].observations[last_index as usize].observation,
            price
        ); // Price should not update
        assert_ne!(
            oracle_twaps.twap_buffers[buffer_index].observations[last_index as usize]
                .unix_timestamp,
            out_of_order_timestamp
        ); // Timestamp should not update
    }

    // Test that a correct observation updates the system even if previous ones were out of order
    #[test]
    fn correct_observation_updates_after_out_of_order_ones() {
        let buffer_index = 0;
        let mut oracle_twaps = create_default_oracle_twaps();
        oracle_twaps.twap_buffers[buffer_index] =
            create_initial_twap_buffer_with_sequential_entries(2);

        let last_index = oracle_twaps.twap_buffers[buffer_index].curr_index;
        let last_timestamp = oracle_twaps.twap_buffers[buffer_index].observations
            [last_index as usize]
            .unix_timestamp;

        let out_of_order_timestamp = last_timestamp - 10;
        let price_out_of_order = 40000;
        let price_correct = 50000;
        let correct_timestamp = last_timestamp + TWAP_INTERVAL_SECONDS;
        let current_slot = oracle_twaps.twap_buffers[buffer_index].last_update_slot + 1;

        // Act with out-of-order timestamp
        let res = store_observation(
            &mut oracle_twaps,
            buffer_index,
            price_out_of_order,
            out_of_order_timestamp,
            current_slot,
        );
        assert!(res.is_err());

        // Act with correct timestamp
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            price_correct,
            correct_timestamp,
            current_slot + 1,
        )
        .unwrap();

        // Assert
        let new_index = (last_index + 1) % TWAP_NUM_OBS as u64;
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].curr_index,
            new_index
        ); // Index should increment now
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[new_index as usize].observation,
            price_correct
        ); // Price should update
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[new_index as usize].unix_timestamp,
            correct_timestamp
        ); // Timestamp should update
    }

    // Utility function for creating a TwapBuffer with a certain number of sequential entries
    fn create_initial_twap_buffer_with_sequential_entries(num_entries: usize) -> TwapBuffer {
        let mut buffer = TwapBuffer::default();

        let start_ts = 100;
        let start_slot = 100;

        for i in 0..num_entries {
            buffer.observations[i].observation = (i * 10000) as u128;
            buffer.observations[i].unix_timestamp =
                start_ts + (i as u64 + 1) * TWAP_INTERVAL_SECONDS;

            buffer.curr_index = (num_entries - 1) as u64 % TWAP_NUM_OBS as u64;
            buffer.last_update_slot = start_slot + i as u64;
        }
        buffer
    }
}

#[cfg(test)]
mod error_handling_tests {
    use crate::MAX_ENTRIES;

    use super::*;

    // Test that an error is returned when trying to store an observation with an invalid timestamp
    #[test]
    fn returns_error_for_invalid_timestamp() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let price = 60000;
        let invalid_timestamp = 0; // Simulate an invalid timestamp
        let current_slot = 1;

        // Act
        let result = store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            invalid_timestamp,
            current_slot,
        );

        // Assert
        assert_eq!(result.err().unwrap(), ScopeError::BadTimestamp.into());
    }

    #[test]
    fn returns_error_for_invalid_slot() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let price = 60000;
        let current_timestamp = 1; // Simulate an invalid timestamp
        let invalid_slot = 0;

        // Act
        let result = store_observation(
            &mut oracle_twaps,
            buffer_index,
            price,
            current_timestamp,
            invalid_slot,
        );

        // Assert
        assert_eq!(result.err().unwrap(), ScopeError::BadSlot.into());
    }

    // Test that an error is returned when trying to store an observation for a nonexistent buffer index
    #[should_panic]
    #[test]
    fn returns_error_for_nonexistent_buffer_index() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let nonexistent_buffer_index = MAX_ENTRIES + 10; // Simulate an index that is out of bounds
        let price = 70000;
        let current_ts = 1;
        let current_slot = 1;

        // Act
        store_observation(
            &mut oracle_twaps,
            nonexistent_buffer_index,
            price,
            current_ts,
            current_slot,
        )
        .unwrap();
    }
}

#[cfg(test)]
mod successive_addition_tests {
    use super::*;

    #[test]
    fn test_successive_adds_within_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_price = 100;
        let additional_price = 10;
        let initial_ts = 1000;
        let initial_slot = 1;

        // Store the initial observation
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            initial_ts,
            initial_slot,
        )
        .unwrap();

        // Successive additions within the interval should be ignored
        for i in 1..=5 {
            let current_ts = initial_ts + i * (TWAP_INTERVAL_SECONDS - 1); // Timestamp within the interval
            let current_slot = initial_slot + i;

            store_observation(
                &mut oracle_twaps,
                buffer_index,
                additional_price,
                current_ts,
                current_slot,
            )
            .unwrap();

            // The initial observation should remain unchanged
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[0].observation,
                initial_price
            );
        }
    }

    #[test]
    fn test_successive_adds_across_intervals() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_price = 100_u128;
        let additional_price = 10_u128;
        let initial_ts = 1000;
        let initial_slot = 1;

        // Store the initial observation
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            initial_ts,
            initial_slot,
        )
        .unwrap();

        // Add new observations after sufficient time has passed to trigger a new interval
        for i in 1..=5 {
            let current_ts = initial_ts + i * TWAP_INTERVAL_SECONDS; // Timestamp across the interval
            let current_slot = initial_slot + i;

            store_observation(
                &mut oracle_twaps,
                buffer_index,
                additional_price,
                current_ts,
                current_slot,
            )
            .unwrap();

            // Verify that the observations are added to the buffer successively
            let expected_index = (i as usize) % TWAP_NUM_OBS;
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[expected_index].observation,
                additional_price as u128
            );
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[expected_index].unix_timestamp,
                current_ts
            );
        }
    }
}
#[cfg(test)]
mod slots_update_tests {
    use super::*;

    #[test]
    fn test_slots_update_on_new_observation() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_price = 100;
        let initial_ts = 1000;
        let initial_slot = 1;

        // Store the initial observation
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            initial_ts,
            initial_slot,
        )
        .unwrap();

        // Verify the initial slot is set
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[0].unix_timestamp,
            initial_ts
        );

        // Add a new observation with a new slot
        let new_slot = initial_slot + 1;
        let new_ts = initial_ts + TWAP_INTERVAL_SECONDS; // Ensure it's a new interval
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            new_ts,
            new_slot,
        )
        .unwrap();

        // Check the new slot update
        let expected_next_index = 1; // Since we're expecting an increment in the index
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[expected_next_index]
                .unix_timestamp,
            new_ts
        );
    }

    #[test]
    fn test_no_slot_update_within_same_interval() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_price = 100;
        let initial_ts = 1000;
        let initial_slot = 1;

        // Store the initial observation
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            initial_ts,
            initial_slot,
        )
        .unwrap();

        // Attempt to add another observation within the same interval, with a new slot value
        let new_slot = initial_slot + 10;
        let within_same_interval_ts = initial_ts + TWAP_INTERVAL_SECONDS - 1; // Still within the same interval
        store_observation(
            &mut oracle_twaps,
            buffer_index,
            initial_price,
            within_same_interval_ts,
            new_slot,
        )
        .unwrap();

        // The slot should not update since we are within the same interval
        assert_eq!(
            oracle_twaps.twap_buffers[buffer_index].observations[0].unix_timestamp,
            initial_ts
        );
    }

    #[test]
    fn test_slots_monotonically_increasing_on_new_intervals() {
        let mut oracle_twaps: OracleTwaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let initial_price = 100;
        let initial_ts = 1000;
        let initial_slot: u64 = 1;

        // Store initial observations across intervals
        for i in 0..TWAP_NUM_OBS {
            let ts = initial_ts + (i as u64) * TWAP_INTERVAL_SECONDS;
            let slot = initial_slot + i as u64;
            store_observation(&mut oracle_twaps, buffer_index, initial_price, ts, slot).unwrap();
        }

        // Verify that slots are monotonically increasing
        for i in 0..TWAP_NUM_OBS {
            assert_eq!(
                oracle_twaps.twap_buffers[buffer_index].observations[i].unix_timestamp,
                initial_slot + i as u64 * TWAP_INTERVAL_SECONDS
            );
        }
    }
}

#[cfg(test)]
mod price_value_tests {
    use super::*;

    #[test]
    fn test_store_minimal_price_value() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let minimal_price = u128::MIN;

        store_and_verify_price_observation(&mut oracle_twaps, buffer_index, minimal_price, 1000, 1);
    }

    #[test]
    fn test_store_maximal_price_value() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let maximal_price = u128::MAX;

        store_and_verify_price_observation(&mut oracle_twaps, buffer_index, maximal_price, 1000, 1);
    }

    // #[test]
    // fn test_store_zero_exponent_price() {
    //     let mut oracle_twaps = create_default_oracle_twaps();
    //     let buffer_index = 0;
    //     let zero_exp_price = 100_000;

    //     store_and_verify_price_observation(
    //         &mut oracle_twaps,
    //         buffer_index,
    //         zero_exp_price,
    //         1000,
    //         1,
    //     );
    // }

    // #[test]
    // fn test_store_high_exponent_price() {
    //     let mut oracle_twaps = create_default_oracle_twaps();
    //     let buffer_index = 0;
    //     let high_exp_price = 1;

    //     store_and_verify_price_observation(
    //         &mut oracle_twaps,
    //         buffer_index,
    //         high_exp_price,
    //         1000,
    //         1,
    //     );
    // }

    #[test]
    fn test_store_varied_exponent_prices() {
        let mut oracle_twaps = create_default_oracle_twaps();
        let buffer_index = 0;
        let exp_values = [2, 4, 6, 8, 10, 12, 14, 16];

        for &exp in exp_values.iter() {
            let price = 100_000;
            store_and_verify_price_observation(
                &mut oracle_twaps,
                buffer_index,
                price,
                1000 + exp * TWAP_INTERVAL_SECONDS as u64,
                1000 + exp,
            );
        }
    }

    fn store_and_verify_price_observation(
        oracle_twaps: &mut OracleTwaps,
        token: usize,
        price: u128,
        current_ts: u64,
        current_slot: u64,
    ) {
        // Store the observation
        store_observation(oracle_twaps, token, price, current_ts, current_slot).unwrap();

        // Retrieve the current index
        let curr_index = oracle_twaps.twap_buffers[token].curr_index as usize;

        // Verify the price and slot are stored correctly
        assert_eq!(
            oracle_twaps.twap_buffers[token].observations[curr_index].observation,
            price
        );
        // assert_eq!(
        //     oracle_twaps.twap_buffers[token].observations[curr_index],
        //     current_slot
        // );
        assert_eq!(
            oracle_twaps.twap_buffers[token].observations[curr_index].unix_timestamp,
            current_ts
        );
    }
}

// #[cfg(test)]
// mod proptests {

//     use super::*;
//     use proptest::prelude::*;

//     proptest! {
//         #![proptest_config(ProptestConfig::with_cases(10000))]

//         #[test]
//         fn test_store_observation_properties(
//             token in 0..MAX_ENTRIES,
//             value in any::<u64>(),
//             exp in any::<u64>(),
//             current_ts in TWAP_INTERVAL_SECONDS..u64::MAX,
//             current_slot in 1..u64::MAX,
//         ) {
//             let mut oracle_twaps = create_default_oracle_twaps();
//             let price = Price { value, exp };

//             let prev_index = oracle_twaps.twap_buffers[token].curr_index as usize;
//             let prev_timestamp = oracle_twaps.twap_buffers[token].unix_timestamps[prev_index];

//             // Apply the function under test
//             let result = store_observation(&mut oracle_twaps, token, price, current_ts, current_slot);

//             // Retrieve the current index
//             let curr_index = oracle_twaps.twap_buffers[token].curr_index as usize;

//             // Properties to test
//             // If the current timestamp is greater by at least TWAP_INTERVAL_SECONDS, an update should occur
//             if current_ts - prev_timestamp >= TWAP_INTERVAL_SECONDS {
//                 // Ensure the function returns success
//                 prop_assert!(result.is_ok());

//                 // Verify that the current index's timestamp matches the current timestamp
//                 prop_assert_eq!(oracle_twaps.twap_buffers[token].unix_timestamps[curr_index], current_ts);

//                 // Verify the slots update
//                 prop_assert_eq!(oracle_twaps.twap_buffers[token].slots[curr_index], current_slot);
//             } else {
//                 // If not enough time has passed, the observation should not be stored
//                 prop_assert!(result.is_err() || oracle_twaps.twap_buffers[token].unix_timestamps[curr_index] == prev_timestamp);
//             }
//         }
//     }
// }

#[cfg(test)]
mod tests_twap_calculation {
    use super::*;

    #[test]
    fn test_correct_twap_calculation() {
        // // Setup mock data for OracleTwaps, Clock, etc.
        // let mut oracle_twaps = create_default_oracle_twaps();
        // let unix_timestamp = 1_000_000;

        // // Setup a price type that requires a certain twap duration and minimum observations
        // let (_price_type, twap_duration_seconds, min_twap_observations) =
        //     (OracleType::ScopeTwap, 5 * 60, 3); // 5 minutes duration, minimum 3 observations

        // // Fill the buffer with mock observations
        // // Assuming TWAP_NUM_OBS = 10 for this example
        // for i in 0..10 {
        //     oracle_twaps.twap_buffers[0].observations[i] = Price {
        //         value: 100 * (i as u64 + 1),
        //         exp: 2,
        //     };
        //     oracle_twaps.twap_buffers[0].unix_timestamps[i] = unix_timestamp - (i as u64 * 60);
        //     oracle_twaps.twap_buffers[0].slots[i] = i as u64;
        // }
        // oracle_twaps.twap_buffers[0].curr_index = 9;

        // // Calculate TWAP
        // let result = get_twap_from_observations(
        //     &oracle_twaps,
        //     0,
        //     unix_timestamp,
        //     twap_duration_seconds,
        //     min_twap_observations,
        // );

        // // Check for Ok result and correct twap calculation
        // println!("Result: {:?}", result);
        // assert!(result.is_ok());
        // let dated_price = result.unwrap();
        // assert_eq!(dated_price.price.value, 550); // Expected TWAP with mock data
        // assert_eq!(dated_price.price.exp, 2);
    }

    #[test]
    fn test_zero_timestamp_observations_ignored() {
        // // Setup mock data for OracleTwaps, etc.
        // let mut oracle_twaps = create_default_oracle_twaps();

        // let (_price_type, twap_duration_seconds, min_twap_observations) =
        //     (OracleType::ScopeTwap, 300, 3);

        // // Provide a mix of valid and uninitialized observations
        // for i in 0..3 {
        //     oracle_twaps.twap_buffers[0].observations[i] = Price { value: 200, exp: 2 };
        //     oracle_twaps.twap_buffers[0].unix_timestamps[i] = if i == 1 {
        //         0
        //     } else {
        //         1_000_000 - (i as u64 * 60)
        //     };
        //     oracle_twaps.twap_buffers[0].slots[i] = i as u64;
        // }
        // oracle_twaps.twap_buffers[0].curr_index = 2;

        // let unix_timestamp = 1_000_000;

        // // Calculate TWAP
        // let result = get_twap_from_observations(
        //     &oracle_twaps,
        //     0,
        //     unix_timestamp,
        //     twap_duration_seconds,
        //     min_twap_observations,
        // );

        // // Check for Ok result and that the zero timestamp observation was ignored in the calculation
        // assert!(result.is_ok());
        // let dated_price = result.unwrap();
        // // Expected TWAP calculation with one observation ignored needs to be asserted here
    }
}
