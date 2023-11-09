use std::ops::Deref;

use anchor_lang::prelude::*;
use anchor_spl::token::Mint;
use decimal_wad::decimal::Decimal;

use crate::utils::account_deserialize;
use crate::{DatedPrice, Result, ScopeError};

pub use perpetuals::ID as JLP_ID;
pub use perpetuals::MINT_SEED;

// Gives the price of 1 staked SOL in SOL
pub fn get_price<'a, 'b>(
    jup_pool_acc: &AccountInfo,
    clock: &Clock,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> Result<DatedPrice>
where
    'a: 'b,
{
    let jup_pool_pk = jup_pool_acc.key;
    let jup_pool: perpetuals::Pool = account_deserialize(jup_pool_acc)?;

    let mint_pk = Pubkey::create_program_address(
        &[
            MINT_SEED,
            &jup_pool_pk.to_bytes(),
            &[jup_pool.lp_token_bump],
        ],
        &perpetuals::ID,
    )
    .map_err(|_| ScopeError::UnableToDerivePDA)?;

    let mint_acc = extra_accounts
        .next()
        .ok_or(ScopeError::AccountsAndTokenMismatch)?;

    require_keys_eq!(mint_pk, *mint_acc.key, ScopeError::UnexpectedAccount);

    let mint = {
        let mint_borrow = mint_acc.data.borrow();
        let mut data: &[u8] = mint_borrow.deref();
        Mint::try_deserialize(&mut data)
    }?;

    let lp_value = jup_pool.aum_usd;
    let lp_token_supply = mint.supply;

    // This is a sanity check to make sure the mint is configured as expected
    // This allows to just divide the two values to get the price
    require_eq!(mint.decimals, perpetuals::POOL_VALUE_SCALE_DECIMALS);

    let price_dec = Decimal::from(lp_value) / lp_token_supply;
    let dated_price = DatedPrice {
        price: price_dec.into(),
        // TODO: find a way to get the last update time
        last_updated_slot: clock.slot,
        unix_timestamp: u64::try_from(clock.unix_timestamp).unwrap(),
        ..Default::default()
    };

    Ok(dated_price)
}

pub fn validate_jlp_pool(account: &AccountInfo) -> Result<()> {
    let _jlp_pool: perpetuals::Pool = account_deserialize(account)?;
    Ok(())
}

mod perpetuals {
    use super::*;
    declare_id!("PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu");

    pub(super) const POOL_VALUE_SCALE_DECIMALS: u8 = 6;

    pub const MINT_SEED: &[u8] = b"lp_token_mint";

    #[account]
    pub(super) struct Pool {
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

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
    pub(super) struct Limit {
        pub max_aum_usd: u128,
        pub max_individual_lp_token: u128,
        pub max_position_usd: u64,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
    pub(super) struct Fees {
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

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
    pub(super) struct PoolApr {
        pub last_updated: i64,
        pub fee_apr_bps: u64,
        pub realized_fee_usd: u64,
    }
}
