use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
pub struct Pool {
    pub name: String,
    pub custodies: Vec<Pubkey>,
    /// Pool value in usd scaled by 6 decimals
    pub aum_usd: u128,
    pub limit: Limit,
    pub fees: Fees,
    pub pool_apr: PoolApr,
    pub max_request_execution_sec: i64,
    pub bump: u8,
    pub lp_token_bump: u8,
    pub inception_time: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct Limit {
    pub max_aum_usd: u128,
    pub max_individual_lp_token: u128,
    pub max_position_usd: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct Fees {
    pub increase_position_bps: u64,
    pub decrease_position_bps: u64,
    pub add_remove_liquidity_bps: u64,
    pub swap_bps: u64,
    pub tax_bps: u64,
    pub stable_swap_bps: u64,
    pub stable_swap_tax_bps: u64,
    pub liquidation_reward_bps: u64,
    pub protocol_share_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct PoolApr {
    pub last_updated: i64,
    pub fee_apr_bps: u64,
    pub realized_fee_usd: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct Custody {
    pub pool: Pubkey,
    pub mint: Pubkey,
    pub token_account: Pubkey,
    pub decimals: u8,
    pub is_stable: bool,
    pub oracle: OracleParams,
    pub pricing: PricingParams,
    pub permissions: Permissions,
    pub target_ratio_bps: u64,
    pub assets: Assets,
    pub funding_rate_state: FundingRateState,

    pub bump: u8,
    pub token_account_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct OracleParams {
    pub oracle_account: Pubkey,
    pub oracle_type: OracleType,
    pub max_price_error: u64,
    pub max_price_age_sec: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub enum OracleType {
    #[default]
    None,
    Test,
    Pyth,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct PricingParams {
    trade_spread_long: u64,
    trade_spread_short: u64,
    swap_spread: u64,
    max_leverage: u64,
    max_global_long_sizes: u64,
    max_global_short_sizes: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct Assets {
    fees_reserves: u64,
    owned: u64,
    locked: u64,
    guaranteed_usd: u64,
    global_short_sizes: u64,
    global_short_average_prices: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct Permissions {
    allow_swap: bool,
    allow_add_liquidity: bool,
    allow_remove_liquidity: bool,
    allow_increase_position: bool,
    allow_decrease_position: bool,
    allow_collateral_withdrawal: bool,
    allow_liquidate_position: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct FundingRateState {
    cumulative_interest_rate: u128,
    last_updated: i64,
    hourly_funding_bps: u64,
}
