use anchor_lang::prelude::{AccountInfo, Clock, ProgramResult, Pubkey, SolanaSysvar};
use anchor_lang::solana_program::program_pack::Pack;
use solend_program::state::{
    LastUpdate, Reserve, ReserveCollateral, ReserveLiquidity, PROGRAM_VERSION,
};

pub fn initialize(
    ctoken_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> ProgramResult {
    let reserve = Reserve {
        version: PROGRAM_VERSION,
        last_update: LastUpdate {
            slot: Clock::get()?.slot,
            stale: false,
        },
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
    let mut data = ctoken_account.data.borrow_mut();
    solend_program::state::Reserve::pack(reserve, &mut data)?;
    Ok(())
}

pub fn update(
    ctoken_account: &AccountInfo,
    mint_total_supply: u64,
    total_liquidity: u64,
) -> ProgramResult {
    let mut data = ctoken_account.data.borrow_mut();
    let mut reserve = Reserve::unpack(&data)?;

    reserve.last_update.slot = Clock::get()?.slot;
    reserve.liquidity.available_amount = total_liquidity;
    reserve.collateral.mint_total_supply = mint_total_supply;

    solend_program::state::Reserve::pack(reserve, &mut data)?;
    Ok(())
}
