pub mod pyth;
pub mod yitoken;

use crate::{DatedPrice, ScopeError};
use anchor_lang::prelude::{AccountInfo, Context, ProgramResult};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

pub fn check_context<T>(ctx: &Context<T>) -> ProgramResult {
    //make sure there are no extra accounts
    if !ctx.remaining_accounts.is_empty() {
        return Err(ScopeError::UnexpectedAccount.into());
    }

    Ok(())
}

#[derive(
    Serialize, Deserialize, IntoPrimitive, TryFromPrimitive, Clone, Copy, PartialEq, Debug,
)]
#[repr(u8)]
pub enum PriceType {
    Pyth,
    Switchboard,
    YiToken,
}

pub fn get_price(price_type: PriceType, price_acc: &AccountInfo) -> crate::Result<DatedPrice> {
    match price_type {
        PriceType::Pyth => pyth::get_price(price_acc),
        PriceType::Switchboard => todo!(),
        PriceType::YiToken => Err(ScopeError::BadTokenType.into()),
    }
}
