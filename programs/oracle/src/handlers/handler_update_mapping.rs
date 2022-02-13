use crate::utils::{check_context, pyth};
use crate::{OracleMappings, Token};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateOracleMapping<'info> {
    pub owner: Signer<'info>,
    #[account(mut)]
    pub oracle_mappings: AccountLoader<'info, OracleMappings>,
    pub pyth_product_info: AccountInfo<'info>,
    pub pyth_price_info: AccountInfo<'info>,
}

pub fn process(ctx: Context<UpdateOracleMapping>, token: Token) -> ProgramResult {
    msg!("ix=update_oracle_mapping");
    check_context(&ctx)?;

    let new_price_pubkey = ctx.accounts.pyth_price_info.key();
    let oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;
    let ref mut current_price_pubkey = match token {
        Token::SOL => oracle_mappings.pyth_sol_price_info,
        Token::ETH => oracle_mappings.pyth_eth_price_info,
        Token::BTC => oracle_mappings.pyth_btc_price_info,
        Token::SRM => oracle_mappings.pyth_srm_price_info,
        Token::RAY => oracle_mappings.pyth_ray_price_info,
        Token::FTT => oracle_mappings.pyth_ftt_price_info,
        Token::MSOL => oracle_mappings.pyth_msol_price_info,
    };

    if new_price_pubkey.eq(current_price_pubkey) {
        // Key already set
        return Ok(());
    }

    let pyth_product_info = ctx.accounts.pyth_product_info.as_ref();
    let pyth_product_data = pyth_product_info.try_borrow_data()?;
    let pyth_product = pyth_client::cast::<pyth_client::Product>(&pyth_product_data);

    pyth::validate_pyth_product(pyth_product)?;
    pyth::validate_pyth_product_symbol(pyth_product, &token)?;
    pyth::validate_pyth_price_pubkey(pyth_product, &new_price_pubkey)?;

    let pyth_price_info = ctx.accounts.pyth_price_info.as_ref();
    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth_client::cast::<pyth_client::Price>(&pyth_price_data);

    pyth::validate_pyth_price(pyth_price)?;

    // Every check succeeded, replace current with new
    *current_price_pubkey = new_price_pubkey;

    Ok(())
}
