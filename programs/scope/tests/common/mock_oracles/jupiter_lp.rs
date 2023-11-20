use anchor_lang::prelude::Pubkey;
use anchor_lang::{AnchorSerialize, Discriminator};
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::common::WAD;
use decimal_wad::decimal::Decimal;
use scope::oracles::jupiter_lp::perpetuals as jlp;
use scope::Price;
use solana_program::program_pack::Pack;

/// Return a JLP account data as:
/// (MappingAccountData, Owner, AdditionalAccounts)
pub(super) fn get_jlp_price_accounts(
    mapping_pk: &Pubkey,
    price: &Price,
) -> super::PriceSourceAccounts {
    let aum_usd = Decimal::from(*price).to_scaled_val().unwrap();
    let mint_supply = WAD;

    let (mint_pk, mint_bump) = jlp::utils::get_mint_pk(mapping_pk);

    let pool = jlp::Pool {
        name: "This is a test pool".to_string(),
        aum_usd,
        lp_token_bump: mint_bump,
        ..Default::default()
    };
    let mut pool_data = Vec::new();
    pool_data.extend_from_slice(&jlp::Pool::DISCRIMINATOR);
    pool.serialize(&mut pool_data).unwrap();

    let mint = Mint {
        supply: mint_supply,
        decimals: scope::oracles::jupiter_lp::POOL_VALUE_SCALE_DECIMALS,
        is_initialized: true,
        ..Default::default()
    };
    let mut mint_data = [0; Mint::LEN];
    mint.pack_into_slice(&mut mint_data);

    let mint_acc = super::AdditionalAccount {
        address: mint_pk,
        owner: jlp::ID,
        data: mint_data.to_vec(),
    };

    super::PriceSourceAccounts {
        oracle_data: pool_data,
        owner: jlp::ID,
        additional_accs: vec![mint_acc],
    }
}
