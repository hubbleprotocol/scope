use anchor_lang::prelude::Pubkey;
use anchor_lang::{AnchorSerialize, Discriminator};
use anchor_spl::token::spl_token::state::Mint;
use decimal_wad::common::WAD;
use decimal_wad::decimal::Decimal;
use scope::oracles::jupiter_lp::perpetuals as jlp;
use scope::Price;
use solana_program::clock::Clock;
use solana_program::program_pack::Pack;

/// Return a JLP account data as:
/// (MappingAccountData, Owner, AdditionalAccounts)
pub(super) fn get_jlp_price_accounts(
    mapping_pk: &Pubkey,
    price: &Price,
    clock: &Clock,
    compute: bool,
) -> super::PriceSourceAccounts {
    let aum_usd = Decimal::from(*price).to_scaled_val().unwrap();
    let mint_supply = WAD;

    let (mint_pk, mint_bump) = jlp::utils::get_mint_pk(mapping_pk);
    let custodies_pk = [(); 3].map(|_| Pubkey::new_unique());

    let pool = jlp::Pool {
        name: "This is a test pool".to_string(),
        aum_usd,
        lp_token_bump: mint_bump,
        custodies: custodies_pk.to_vec(),
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

    let mut additional_accs = vec![mint_acc];

    if compute {
        let mut custodies_acc = Vec::new();
        let mut oracles_acc = Vec::new();

        // Add some 3 custodies and oracles
        // Prices of 1.0, 2.0 and 3.0
        // Split aum_usd over the 3 custodies
        let prices = [1_u64, 2, 3];
        let custodies_amounts = prices.map(|p| aum_usd / (3 * p as u128));
        let oracles_pk = [(); 3].map(|_| Pubkey::new_unique());
        for (((price, c_amount), custody_pk), oracle_pk) in prices
            .into_iter()
            .zip(custodies_amounts)
            .zip(custodies_pk)
            .zip(oracles_pk)
        {
            let custody = jlp::Custody {
                oracle: jlp::OracleParams {
                    oracle_account: oracle_pk,
                    oracle_type: jlp::OracleType::Pyth,
                    max_price_age_sec: u32::MAX,
                    max_price_error: u64::MAX,
                },
                decimals: 6,
                assets: jlp::Assets {
                    owned: c_amount.try_into().unwrap(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut custody_data = Vec::new();
            custody_data.extend_from_slice(&jlp::Custody::DISCRIMINATOR);
            custody.serialize(&mut custody_data).unwrap();

            let custody_acc = super::AdditionalAccount {
                address: custody_pk,
                owner: jlp::ID,
                data: custody_data,
            };

            let price_a = Price {
                value: price,
                exp: 0,
            };

            let oracle_acc = super::AdditionalAccount {
                address: oracle_pk,
                owner: super::pyth::id(),
                data: super::pyth::get_account_data_for_price(&price_a, clock),
            };

            custodies_acc.push(custody_acc);
            oracles_acc.push(oracle_acc);
        }
        additional_accs.extend(custodies_acc);
        additional_accs.extend(oracles_acc);
    }

    super::PriceSourceAccounts {
        oracle_data: pool_data,
        owner: jlp::ID,
        additional_accs,
    }
}
