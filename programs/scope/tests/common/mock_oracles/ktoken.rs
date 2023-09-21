use anchor_lang::{
    prelude::{borsh, Clock, Pubkey},
    Discriminator,
};
use kamino::{
    state::{CollateralInfo, CollateralInfos, GlobalConfig, WhirlpoolStrategy},
    whirlpool,
};
use scope::{scope_chain::MAX_CHAIN_LENGTH, OracleMappings, Price};
use solana_sdk::pubkey;
use whirlpool::state::{Position, Whirlpool};
use yvaults as kamino;
use yvaults::{
    raydium_amm_v3::states::{PersonalPositionState, PoolState},
    utils::types::DEX,
};

use crate::common::{
    mock_oracles, operations,
    types::{OracleConf, ScopeFeedDefinition, TestContext, TestOracleType},
};

pub const fn id() -> Pubkey {
    // It does not matter what the pubkey is
    pubkey!("Kamino1111111111111111111111111111111111111")
}

pub async fn get_ktoken_price_accounts(
    ctx: &mut TestContext,
    feed: &ScopeFeedDefinition,
    dex: DEX,
    price: &Price,
    clock: &Clock,
) -> (Vec<u8>, Pubkey, Vec<(Pubkey, Pubkey, Vec<u8>)>) {
    // Create 2 new scope oracle mappings for token A and token B with price 1 USD
    let oracle_mappings: OracleMappings = ctx.get_zero_copy_account(&feed.mapping).await.unwrap();
    // Find the first 2 empty mappings - in reverse to not interfere with user defined prices
    let (mut token_a, mut token_b) = (0, 0);
    for (i, mapping) in oracle_mappings.price_info_accounts.iter().enumerate().rev() {
        if mapping == &Pubkey::default() {
            if token_a == 0 {
                token_a = i;
            } else {
                token_b = i;
                break;
            }
        }
    }
    let token_a_oracle_conf = OracleConf {
        pubkey: pubkey!("KaminoTokenAPyth111111111111111111111111111"),
        price_type: TestOracleType::Pyth,
        token: token_a,
    };
    let token_b_oracle_conf = OracleConf {
        pubkey: pubkey!("KaminoTokenBPyth111111111111111111111111111"),
        price_type: TestOracleType::Pyth,
        token: token_b,
    };
    // Set the price
    mock_oracles::set_price(
        ctx,
        &feed,
        &token_a_oracle_conf,
        &Price {
            value: 1_000_000,
            exp: 6,
        },
    )
    .await;
    mock_oracles::set_price(
        ctx,
        &feed,
        &token_b_oracle_conf,
        &Price {
            value: 1_000_000,
            exp: 6,
        },
    )
    .await;
    // Set the mappings
    operations::update_oracle_mapping(ctx, &feed, &token_a_oracle_conf).await;
    operations::update_oracle_mapping(ctx, &feed, &token_b_oracle_conf).await;
    // Refresh the prices
    operations::refresh_price(ctx, &feed, &token_a_oracle_conf).await;
    operations::refresh_price(ctx, &feed, &token_b_oracle_conf).await;

    let collateral_infos = get_account_data_for_collateral_infos(
        &[
            token_a_oracle_conf.token as u16,
            u16::MAX,
            u16::MAX,
            u16::MAX,
        ],
        &[
            token_b_oracle_conf.token as u16,
            u16::MAX,
            u16::MAX,
            u16::MAX,
        ],
    );
    let global_config = get_account_data_for_global_config(feed.prices, collateral_infos.0);
    let (dex_pool, dex_position) = match dex {
        DEX::Orca => {
            let pool = get_account_data_for_orca_pool();
            let position = get_account_data_for_orca_position(pool.0);
            (pool, position)
        }
        DEX::Raydium => {
            let pool = get_account_data_for_raydium_pool();
            let position = get_account_data_for_raydium_position(pool.0);
            (pool, position)
        }
    };

    let strategy = get_account_data_for_strategy(
        global_config.0,
        feed.prices,
        dex,
        dex_pool.0,
        dex_position.0,
        price,
        clock,
    );
    (
        strategy,
        id(),
        vec![global_config, collateral_infos, dex_pool, dex_position],
    )
}

pub fn get_account_data_for_global_config(
    scope_prices: Pubkey,
    token_infos: Pubkey,
) -> (Pubkey, Pubkey, Vec<u8>) {
    let global_config = GlobalConfig {
        scope_price_id: scope_prices,
        token_infos,
        ..new_global_config()
    };
    let mut data = [0u8; 8 + std::mem::size_of::<GlobalConfig>()];
    data[0..8].copy_from_slice(&GlobalConfig::discriminator());
    data[8..].copy_from_slice(bytemuck::bytes_of(&global_config));
    (
        pubkey!("KaminoGC11111111111111111111111111111111111"),
        id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_orca_pool() -> (Pubkey, Pubkey, Vec<u8>) {
    let mut data = [0u8; Whirlpool::LEN];
    data[0..8].copy_from_slice(&Whirlpool::discriminator());
    let bytes = borsh::to_vec(&Whirlpool::default()).unwrap();
    data[8..].copy_from_slice(&bytes);
    (
        pubkey!("KaminoorcaPoo111111111111111111111111111111"),
        whirlpool::id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_orca_position(whirlpool: Pubkey) -> (Pubkey, Pubkey, Vec<u8>) {
    let mut data = [0u8; Position::LEN];
    data[0..8].copy_from_slice(&Position::discriminator());
    let position = Position {
        whirlpool,
        ..Default::default()
    };
    let bytes = borsh::to_vec(&position).unwrap();
    data[8..].copy_from_slice(&bytes);
    (
        pubkey!("KaminoorcaPos111111111111111111111111111111"),
        whirlpool::id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_raydium_pool() -> (Pubkey, Pubkey, Vec<u8>) {
    let mut data = [0u8; PoolState::LEN];
    data[0..8].copy_from_slice(&PoolState::discriminator());
    let state = PoolState::default();
    data[8..].copy_from_slice(&bytemuck::bytes_of(&state));
    (
        pubkey!("KaminoRaydiumPoo111111111111111111111111111"),
        whirlpool::id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_raydium_position(raydium_pool: Pubkey) -> (Pubkey, Pubkey, Vec<u8>) {
    let mut data = [0u8; PersonalPositionState::LEN];
    data[0..8].copy_from_slice(&PersonalPositionState::discriminator());
    let position = PersonalPositionState {
        pool_id: raydium_pool,
        ..Default::default()
    };
    let bytes = borsh::to_vec(&position).unwrap();
    data[8..].copy_from_slice(&bytes);
    (
        pubkey!("KaminoRaydiumPos111111111111111111111111111"),
        whirlpool::id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_collateral_infos(
    a_scope_chain: &[u16; MAX_CHAIN_LENGTH],
    b_scope_chain: &[u16; MAX_CHAIN_LENGTH],
) -> (Pubkey, Pubkey, Vec<u8>) {
    let token_a_info = CollateralInfo {
        scope_price_chain: a_scope_chain.clone(),
        ..Default::default()
    };
    let token_b_info = CollateralInfo {
        scope_price_chain: b_scope_chain.clone(),
        ..Default::default()
    };
    let mut collateral_infos = new_collateral_infos();
    collateral_infos.infos[0] = token_a_info;
    collateral_infos.infos[1] = token_b_info;
    let mut data = [0u8; 8 + std::mem::size_of::<CollateralInfos>()];
    data[0..8].copy_from_slice(&CollateralInfos::discriminator());
    data[8..].copy_from_slice(bytemuck::bytes_of(&collateral_infos));
    (
        pubkey!("KaminoCi11111111111111111111111111111111111"),
        id(),
        data.to_vec(),
    )
}

pub fn get_account_data_for_strategy(
    global_config: Pubkey,
    scope_prices: Pubkey,
    dex: DEX,
    pool: Pubkey,
    position: Pubkey,
    price: &Price,
    _clock: &Clock,
) -> Vec<u8> {
    // assume token_a and token_b each = 1 USD
    // set token_a and token_b amounts each = (share_px * 1,000,000) / 2
    // no_shares = 1,000,000
    let token_amt = ((price.value * 1_000_000_000_000) / 10_u64.pow(price.exp as u32)) / 2;
    let strategy = WhirlpoolStrategy {
        strategy_dex: dex.into(),
        token_a_collateral_id: 0,
        token_b_collateral_id: 1,
        global_config,
        pool,
        position,
        scope_prices,
        shares_mint_decimals: 6,
        token_a_mint_decimals: 6,
        token_b_mint_decimals: 6,
        token_a_amounts: token_amt,
        token_b_amounts: token_amt,
        shares_issued: 1_000_000_000_000,
        ..Default::default()
    };
    let mut data = [0u8; 8 + std::mem::size_of::<WhirlpoolStrategy>()];
    data[0..8].copy_from_slice(&WhirlpoolStrategy::discriminator());
    data[8..].copy_from_slice(bytemuck::bytes_of(&strategy));
    data.to_vec()
}

pub fn new_collateral_infos() -> CollateralInfos {
    CollateralInfos {
        infos: [CollateralInfo::default(); 256],
    }
}

pub fn new_global_config() -> GlobalConfig {
    let vaults: [Pubkey; 256] = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

    GlobalConfig {
        emergency_mode: 0,
        block_deposit: 0,
        block_invest: 0,
        block_withdraw: 0,
        block_collect_fees: 0,
        block_collect_rewards: 0,
        block_swap_rewards: 0,
        block_swap_uneven_vaults: 0,
        block_emergency_swap: 0,
        fees_bps: 0,
        scope_program_id: Pubkey::default(),
        scope_price_id: Pubkey::default(),
        swap_rewards_discount_bps: [0; 256],
        actions_authority: Pubkey::default(),
        admin_authority: Pubkey::default(),
        token_infos: Pubkey::default(),
        treasury_fee_vaults: vaults,
        block_local_admin: 0,
        min_performance_fee_bps: 0,
        _padding: [0; 2042],
    }
}
