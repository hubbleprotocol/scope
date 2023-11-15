use anchor_lang::AnchorSerialize;
use anchor_lang::{prelude::Pubkey, Discriminator};
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::common::WAD;
use decimal_wad::decimal::{Decimal, U192};
use raydium_amm_v3::states::PoolState as RaydiumPoolState;
use scope::utils::math::ten_pow;
use scope::Price;
use solana_program::program_pack::Pack;
use solana_sdk::pubkey;
use whirlpool::state::Whirlpool;

const MINT_DECIMALS: u8 = 6;

pub(super) fn get_orca_whirlpool_accounts(
    price: &Price,
    a_to_b: bool,
) -> super::PriceSourceAccounts {
    let sqrt_price = price_to_sqrt_price(price, a_to_b, MINT_DECIMALS, MINT_DECIMALS);

    let mint_a_pk = pubkey!("orcaWhirLpooLMintA1111111111111111111111111");
    let mint_b_pk = pubkey!("orcaWhirLpooLMintB1111111111111111111111111");
    let mint_a_acc = get_mint_acc(mint_a_pk);
    let mint_b_acc = get_mint_acc(mint_b_pk);

    let whirlpool = Whirlpool {
        token_mint_a: mint_a_pk,
        token_mint_b: mint_b_pk,
        sqrt_price,
        ..Default::default()
    };

    let mut whirlpool_data = Vec::new();
    whirlpool_data.extend_from_slice(&Whirlpool::DISCRIMINATOR);
    whirlpool.serialize(&mut whirlpool_data).unwrap();

    super::PriceSourceAccounts {
        oracle_data: whirlpool_data,
        owner: whirlpool::id(),
        additional_accs: vec![mint_a_acc, mint_b_acc],
    }
}

pub(super) fn get_raydium_amm_v3_accounts(
    price: &Price,
    a_to_b: bool,
) -> super::PriceSourceAccounts {
    let price_sqrt = price_to_sqrt_price(price, a_to_b, MINT_DECIMALS, MINT_DECIMALS);

    let pool = RaydiumPoolState {
        mint_decimals_0: MINT_DECIMALS,
        mint_decimals_1: MINT_DECIMALS,
        sqrt_price_x64: price_sqrt,
        ..Default::default()
    };
    let mut pool_data = Vec::new();
    pool_data.extend_from_slice(&RaydiumPoolState::DISCRIMINATOR);
    pool_data.extend_from_slice(bytemuck::bytes_of(&pool));

    super::PriceSourceAccounts {
        oracle_data: pool_data,
        owner: raydium_amm_v3::id(),
        additional_accs: vec![],
    }
}

fn get_mint_acc(mint_pk: Pubkey) -> super::AdditionalAccount {
    let mint = Mint {
        supply: 10000000000,
        decimals: MINT_DECIMALS,
        is_initialized: true,
        ..Default::default()
    };
    let mut mint_data = [0; Mint::LEN];
    mint.pack_into_slice(&mut mint_data);

    super::AdditionalAccount {
        address: mint_pk,
        owner: Pubkey::new_unique(),
        data: mint_data.to_vec(),
    }
}

pub fn price_to_sqrt_price(price: &Price, a_to_b: bool, decimals_a: u8, decimals_b: u8) -> u128 {
    if price.value == 0 {
        return 0;
    }
    let price_dec: Decimal = dbg!((*price).into());
    let price_dec = if a_to_b {
        price_dec
    } else {
        Decimal::one() / price_dec
    };
    decimal_price_to_sqrt_price(price_dec, decimals_a, decimals_b)
}

pub fn decimal_price_to_sqrt_price(price: Decimal, decimals_a: u8, decimals_b: u8) -> u128 {
    let scaled_price_x64 = (price.0 << 64) / WAD;

    let price = if decimals_b >= decimals_a {
        scaled_price_x64 * U192::from(ten_pow(decimals_b - decimals_a))
    } else {
        scaled_price_x64 / U192::from(ten_pow(decimals_a - decimals_b))
    };
    dbg!(price.integer_sqrt().as_u128())
}
