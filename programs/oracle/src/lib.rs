use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};
mod pyth_utils;

use pyth_utils::get_price;

declare_id!("A9DXGTCMLJsX7kMfwJ2aBiAFACPmUsxv6TRxcEohL4CD");

#[program]
mod oracle {
    use std::convert::TryFrom;

    use super::*;
    pub fn initialize(_ctx: Context<Initialize>) -> ProgramResult {
        Ok(())
    }

    pub fn update(ctx: Context<Update>, token: u8) -> ProgramResult {
        let mut oracle = ctx.accounts.oracle.load_mut()?;
        let clock = &ctx.accounts.clock;
        let token = Token::try_from(token).map_err(|_|ProgramError::InvalidArgument)?;
        let slot = clock.slot;
        let epoch = clock.epoch;
        let timestamp = clock.epoch;

        let pyth_price_info = ctx.accounts.pyth_price_info.as_ref();

        // TODO check that the provided "pyth_price_info" is the "token" one
        // or better, remove the "token" parameter and guess it from "pyth_price_info"
        let price = get_price(pyth_price_info, token)?;

        msg!(
            "Setting the price of {:?} to {:?} as of Slot:{} Epoch:{} TS:{}",
            token,
            price,
            slot,
            epoch,
            timestamp
        );

        // TODO change "oracle" to an array indexed by `Token`

        let to_update = match token {
            Token::SOL => &mut oracle.sol,
            Token::ETH => &mut oracle.eth,
            Token::BTC => &mut oracle.btc,
            Token::SRM => &mut oracle.srm,
            Token::RAY => &mut oracle.ray,
            Token::FTT => &mut oracle.ftt,
            Token::MSOL => &mut oracle.msol,
        };

        to_update.price = price;
        to_update.last_updated_slot = slot; // TODO Is it the time reference we want?

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = 8 + (8+8+8)*100)]// Space = account discriminator + (price + exposant + timestamp)*max_stored_prices
    pub oracle: AccountLoader<'info, OraclePrices>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub admin: Signer<'info>,
    #[account(mut)]
    pub oracle: AccountLoader<'info, OraclePrices>,
    pub pyth_price_info: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[zero_copy]
#[derive(Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Default)]
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
