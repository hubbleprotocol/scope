#![allow(unaligned_references)]
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

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
}

#[zero_copy]
#[repr(packed)]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Hash {
    pub data: [u8; 32],
}

#[zero_copy]
#[repr(packed)]
#[derive(Debug, PartialEq)]
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
