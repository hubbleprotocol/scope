use anchor_client::solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};

pub const DEFAULT_STATUS_FETCH_DELAY_MS: u64 = 500;
pub const CONFIRMATION_TIMEOUT_PROCESSED_MS: u64 = 6_000;
pub const CONFIRMATION_TIMEOUT_CONFIRMED_MS: u64 = 15_000;
pub const CONFIRMATION_TIMEOUT_FINALIZED_MS: u64 = 30_000;
pub const DEFAULT_RETRY_TIMEOUT_MS: u64 = 5_000;
pub const DEFAULT_NUM_FETCH_TO_RETRY: usize = timeout_to_retry_count(DEFAULT_RETRY_TIMEOUT_MS);

pub const fn commitment_to_timeout(commitment: CommitmentConfig) -> u64 {
    #[allow(deprecated)]
    match commitment.commitment {
        CommitmentLevel::Processed => CONFIRMATION_TIMEOUT_PROCESSED_MS,
        CommitmentLevel::Confirmed => CONFIRMATION_TIMEOUT_CONFIRMED_MS,
        CommitmentLevel::Finalized => CONFIRMATION_TIMEOUT_FINALIZED_MS,
        // Deprecated commitment levels, defined for completeness
        CommitmentLevel::Max => CONFIRMATION_TIMEOUT_FINALIZED_MS,
        CommitmentLevel::Recent => CONFIRMATION_TIMEOUT_PROCESSED_MS,
        CommitmentLevel::Root => CONFIRMATION_TIMEOUT_FINALIZED_MS,
        CommitmentLevel::Single => CONFIRMATION_TIMEOUT_CONFIRMED_MS,
        CommitmentLevel::SingleGossip => CONFIRMATION_TIMEOUT_CONFIRMED_MS,
    }
}

pub const fn timeout_to_retry_count(timeout: u64) -> usize {
    // Ensure usize is as large as u64
    assert!(u64::MAX as u128 == usize::MAX as u128);
    (timeout / DEFAULT_STATUS_FETCH_DELAY_MS) as usize
}

pub const fn commitment_to_retry_count(commitment: CommitmentConfig) -> usize {
    timeout_to_retry_count(commitment_to_timeout(commitment))
}
