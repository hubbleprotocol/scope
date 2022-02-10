use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};
mod pyth_utils;

use pyth_utils::get_price;

declare_id!("6jnS9rvUGxu4TpwwuCeF12Ar9Cqk2vKbufqc6Hnharnz");

#[program]
mod oracle {
    use std::convert::TryFrom;

    use super::*;
    pub fn initialize(_ctx: Context<Initialize>) -> ProgramResult {
        Ok(())
    }

    pub fn update(ctx: Context<Update>, token: u8) -> ProgramResult {
        let oracle = &mut ctx.accounts.oracle;
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

        match token {
            Token::SOL => oracle.sol.price = price,
            Token::ETH => oracle.eth.price = price,
            Token::BTC => oracle.btc.price = price,
            Token::SRM => oracle.srm.price = price,
            Token::RAY => oracle.ray.price = price,
            Token::FTT => oracle.ftt.price = price,
            Token::MSOL => oracle.msol.price = price,
        };

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin)]
    pub oracle: Account<'info, OraclePrices>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct OracleMappings {
    // Validated pyth accounts
    pub pyth_sol_price_info: Pubkey,
    pub pyth_srm_price_info: Pubkey,
    pub pyth_eth_price_info: Pubkey,
    pub pyth_btc_price_info: Pubkey,
    pub pyth_ray_price_info: Pubkey,
    pub pyth_ftt_price_info: Pubkey,
    pub pyth_msol_price_info: Pubkey,
    pub _reserved: [u64; 128],
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub admin: Signer<'info>,
    pub oracle_mappings: Box<Account<'info, OracleMappings>>,
    #[account(mut)]
    pub oracle: Account<'info, OraclePrices>,
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
#[derive(Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Default)]
pub struct DatedPrice {
    pub price: Price,
    pub decimals: u8,
    pub last_updated_slot: u64,
}

#[account]
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
pub enum Token {
    SOL,
    ETH,
    BTC,
    SRM,
    RAY,
    FTT,
    MSOL,
}
