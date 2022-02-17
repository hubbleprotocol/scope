use crate::{utils::pyth::get_price, Token};
use anchor_lang::prelude::*;
use std::convert::TryFrom;

#[derive(Accounts)]
pub struct RefreshOne<'info> {
    pub admin: Signer<'info>,
    #[account(mut)]
    pub oracle_prices: AccountLoader<'info, crate::OraclePrices>,
    #[account(mut)]
    pub oracle_mappings: AccountLoader<'info, crate::OracleMappings>,
    pub pyth_price_info: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn refresh_one_price(ctx: Context<RefreshOne>, token: u8) -> ProgramResult {
    msg!("ix=refresh_one_price");
    let mut oracle = ctx.accounts.oracle_prices.load_mut()?;
    let clock = &ctx.accounts.clock;
    let token = Token::try_from(token).map_err(|_| ProgramError::InvalidArgument)?;

    let pyth_price_info = ctx.accounts.pyth_price_info.as_ref();

    // TODO check that the provided "pyth_price_info" is the "token" one
    // or better, remove the "token" parameter and guess it from "pyth_price_info"
    let price = get_price(pyth_price_info, token)?;

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
    to_update.last_updated_slot = clock.slot; // TODO Use price `valid_slot` for pyth prices

    Ok(())
}
