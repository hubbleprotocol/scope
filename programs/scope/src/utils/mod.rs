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
pub enum OracleType {
    Pyth = 0,
    Switchboard,
    YiToken,
}

/// Get the price for a given oracle type
///
/// The `base_account` should have been checked against the oracle mapping
/// If needed the `extra_accounts` will be extracted from the provided iterator and checked
/// with the data contained in the `base_account`
pub fn get_price<'a, 'b>(
    price_type: OracleType,
    base_account: &AccountInfo,
    extra_accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
) -> crate::Result<DatedPrice>
where
    'a: 'b,
{
    match price_type {
        OracleType::Pyth => pyth::get_price(base_account),
        OracleType::Switchboard => todo!(),
        OracleType::YiToken => yitoken::get_price(base_account, extra_accounts),
    }
}
