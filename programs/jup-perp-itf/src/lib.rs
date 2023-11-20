#![allow(clippy::result_large_err)]

pub mod states;
pub mod utils;

use anchor_lang::prelude::*;
pub use states::*;

declare_id!("PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu");

#[program]
pub mod perpetuals {
    use super::*;

    pub fn get_assets_under_management(
        _ctx: Context<GetAssetsUnderManagement>,
        _params: GetAssetsUnderManagementParams,
    ) -> Result<u128> {
        // We only need the interface, not the actual implementation here.
        unimplemented!("jup-perp-itf is just an interface")
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetAssetsUnderManagementParams {
    pub mode: Option<PriceCalcMode>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum PriceCalcMode {
    Min,
    Max,
    Ignore,
}

#[derive(Accounts)]
pub struct GetAssetsUnderManagement<'info> {
    // H4ND9aYttUVLFmNypZqLjZ52FYiGvdEB45GmwNoKEjTj
    /// CHECK: don't care this is just an interface
    #[account()]
    pub perpetuals: AccountInfo<'info>,

    /// CHECK: don't care this is just an interface
    #[account()]
    pub pool: Box<Account<'info, Pool>>,
    // remaining accounts:
    //   pool.tokens.len() custody accounts (read-only, unsigned)
    //   pool.tokens.len() custody oracles (read-only, unsigned)
}
