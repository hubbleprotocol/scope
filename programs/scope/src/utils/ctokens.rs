use crate::{DatedPrice, Price, Result};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;

use solend_program::state::Reserve;

const DECIMALS: u32 = 15u32;

pub fn get_price(solend_reserve_account: &AccountInfo) -> Result<DatedPrice> {
    let reserve = Reserve::unpack(&solend_reserve_account.data.borrow())?;

    let value = scaled_rate(&reserve)?;

    let price = Price {
        value,
        exp: DECIMALS.into(),
    };
    let dated_price = DatedPrice {
        price,
        last_updated_slot: reserve.last_update.slot,
        _reserved: Default::default(),
    };

    Ok(dated_price)
}

fn scaled_rate(reserve: &Reserve) -> Result<u64> {
    const FACTOR: u64 = 10u64.pow(DECIMALS);
    let rate = reserve.collateral_exchange_rate()?;
    let value = rate.liquidity_to_collateral(FACTOR)?;

    Ok(value)
}

#[cfg(test)]
mod test {
    use solend_program::state::{ReserveCollateral, ReserveLiquidity};

    use super::*;

    #[test]
    pub fn scale_1_to_1() {
        let total_liquidity = 10u64.pow(5);
        let mint_total_supply = 10u64.pow(5);
        let reserve = Reserve {
            version: 1,
            lending_market: Pubkey::default(),
            liquidity: ReserveLiquidity {
                available_amount: total_liquidity,
                ..Default::default()
            },
            collateral: ReserveCollateral {
                mint_total_supply,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(scaled_rate(&reserve).unwrap(), 10u64.pow(DECIMALS));
    }

    #[test]
    pub fn scale_1_to_2() {
        let total_liquidity = 10u64.pow(5);
        let mint_total_supply = 2 * 10u64.pow(5);
        let reserve = Reserve {
            version: 1,
            lending_market: Pubkey::default(),
            liquidity: ReserveLiquidity {
                available_amount: total_liquidity,
                ..Default::default()
            },
            collateral: ReserveCollateral {
                mint_total_supply,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(scaled_rate(&reserve).unwrap(), 2 * 10u64.pow(DECIMALS));
    }

    #[test]
    pub fn scale_2_to_1() {
        let total_liquidity = 2 * 10u64.pow(5);
        let mint_total_supply = 10u64.pow(5);
        let reserve = Reserve {
            version: 1,
            lending_market: Pubkey::default(),
            liquidity: ReserveLiquidity {
                available_amount: total_liquidity,
                ..Default::default()
            },
            collateral: ReserveCollateral {
                mint_total_supply,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(scaled_rate(&reserve).unwrap(), 5 * 10u64.pow(DECIMALS - 1));
    }
}
