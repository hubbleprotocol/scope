use anchor_lang::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};
use std::convert::TryInto;
pub mod handlers;
pub mod utils;

pub use handlers::*;

declare_id!("A9DXGTCMLJsX7kMfwJ2aBiAFACPmUsxv6TRxcEohL4CD");

#[program]
mod oracle {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> ProgramResult {
        msg!("ix=initialize");
        Ok(())
    }

    pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u8) -> ProgramResult {
        handler_refresh_prices::refresh_one_price(ctx, token)
    }

    pub fn update_mapping(ctx: Context<UpdateOracleMapping>, token: u8) -> ProgramResult {
        let token: Token = token.try_into().map_err(|err| ScopeError::from(err))?;
        handler_update_mapping::process(ctx, token)
    }
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, Default)]
pub struct Price {
    // Pyth price, integer + exponent representation
    // decimal price would be
    // as integer: 6462236900000, exponent: 8
    // as float:   64622.36900000

    // value is the scaled integer
    // for example, 6462236900000 for btc
    pub value: u64,

    // exponent represents the number of decimals
    // for example, 8 for btc
    pub exp: u64,
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, Default)]
pub struct DatedPrice {
    pub price: Price,
    pub last_updated_slot: u64,
}

#[account(zero_copy)]
#[derive(Default)]
pub struct OraclePrices {
    pub sol: DatedPrice,
    pub eth: DatedPrice,
    pub btc: DatedPrice,
    pub srm: DatedPrice,
    pub ftt: DatedPrice,
    pub ray: DatedPrice,
    pub msol: DatedPrice,
}

#[account(zero_copy)]
#[derive(Default)]
pub struct OracleMappings {
    // Validated pyth accounts
    pub pyth_sol_price_info: Pubkey,
    pub pyth_srm_price_info: Pubkey,
    pub pyth_eth_price_info: Pubkey,
    pub pyth_btc_price_info: Pubkey,
    pub pyth_ray_price_info: Pubkey,
    pub pyth_ftt_price_info: Pubkey,
    pub pyth_msol_price_info: Pubkey,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
#[non_exhaustive]
pub enum Token {
    SOL,
    ETH,
    BTC,
    SRM,
    RAY,
    FTT,
    MSOL,
}

#[error]
#[derive(PartialEq, Eq)]
pub enum ScopeError {
    #[msg("Integer overflow")]
    IntegerOverflow,

    #[msg("Conversion failure")]
    ConversionFailure,

    #[msg("Mathematical operation with overflow")]
    MathOverflow,

    #[msg("Out of range integral conversion attempted")]
    OutOfRangeIntegralConversion,

    #[msg("Unexpected account in instruction")]
    UnexpectedAccount,

    #[msg("Price is not valid")]
    PriceNotValid,
}

impl<T> From<TryFromPrimitiveError<T>> for ScopeError
where
    T: TryFromPrimitive,
{
    fn from(_: TryFromPrimitiveError<T>) -> Self {
        ScopeError::ConversionFailure
    }
}
