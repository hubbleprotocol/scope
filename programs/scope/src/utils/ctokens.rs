use crate::{DatedPrice, Price, Result};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;

use solend_program::state::Reserve;

const DECIMALS: u32 = 15u32;

// Gives the price of 1 cToken in the collateral token
pub fn get_price(solend_reserve_account: &AccountInfo, clock: &Clock) -> Result<DatedPrice> {
    let mut reserve = Reserve::unpack(&solend_reserve_account.data.borrow())?;

    // Manual refresh of the reserve to ensure the most accurate price
    let last_updated_slot = if reserve.accrue_interest(clock.slot).is_ok() {
        // We have just refreshed the price so we can use the current slot
        clock.slot
    } else {
        // This should never happen but on simulations when the current slot is not valid
        // yet we have a default value
        reserve.last_update.slot
    };

    let value = scaled_rate(&reserve)?;

    let price = Price {
        value,
        exp: DECIMALS.into(),
    };
    let dated_price = DatedPrice {
        price,
        last_updated_slot,
        _reserved: Default::default(),
    };

    Ok(dated_price)
}

fn scaled_rate(reserve: &Reserve) -> Result<u64> {
    const FACTOR: u64 = 10u64.pow(DECIMALS);
    let rate = reserve.collateral_exchange_rate()?;
    let value = rate.collateral_to_liquidity(FACTOR)?;

    Ok(value)
}

#[cfg(test)]
mod test {
    use solend_program::state::{ReserveCollateral, ReserveLiquidity};

    use super::*;

    #[test]
    pub fn minted_ctoken_is_equal_to_token_in_vault() {
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
    pub fn minted_ctoken_is_2xtoken_in_vault() {
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
        // Expect ctoken price to be 0.5 token
        assert_eq!(scaled_rate(&reserve).unwrap(), 5 * 10u64.pow(DECIMALS - 1));
    }

    #[test]
    pub fn token_in_vault_is_2xctoken_minted() {
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
        // Expect ctoken price to be 2 tokens
        assert_eq!(scaled_rate(&reserve).unwrap(), 2 * 10u64.pow(DECIMALS));
    }
}
