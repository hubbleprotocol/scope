mod common;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use common::*;
use scope::{assert_fuzzy_price_eq, OraclePrices, Price, ScopeError};
use solana_program::{
    instruction::Instruction, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use solana_program_test::tokio;
use solana_sdk::pubkey;
use types::*;

use crate::utils::map_scope_error;

// Note: those tests aims to check the exact returned errors for a given price type while
// `tests_security_refresh_list.rs` only check the absence of update because individual price
// validation are not terminal unless only one price is being refreshed.
// They are mostly kept as legacy from the old "RefreshOne" ix but can be interesting in some cases.

// KTokens:
// - [x] Wrong kToken additional global config account
// - [x] Wrong kToken additional collateral infos account
// - [x] Wrong kToken additional orca whirlpool account
// - [x] Wrong kToken additional orca position account
// - [x] Wrong kToken additional scope prices account

// Jupiter LP Fetch:
// - [x] Wrong Jupiter LP additional mint account

// Jupiter LP Compute:
// - [x] Wrong Jupiter LP additional mint account
// - [x] Wrong Jupiter LP additional custodies account
// - [x] Wrong Jupiter LP additional oracles account

// Spl Stake:
// - [x] Working case, refreshed in current epoch
// - [x] Working case, refreshed in previous epoch but less than 1hour ago
// - [x] Fail case, refreshed in previous epoch but more than 1hour ago
// - [x] Fail case, refreshed in more than 1 epoch ago and epoch started less than 1hour ago

#[cfg(feature = "yvaults")]
mod ktoken_tests {
    use kamino::state::{GlobalConfig, WhirlpoolStrategy};
    use yvaults as kamino;
    use yvaults::utils::types::DEX;

    use super::*;

    const TEST_ORCA_KTOKEN_ORACLE: OracleConf = OracleConf {
        pubkey: pubkey!("SomeKaminoorcaStrategyAccount11111111111111"),
        token: 2,
        price_type: TestOracleType::KToken(DEX::Orca),
        twap_enabled: false,
        twap_source: None,
    };

    const TEST_RAYDIUM_KTOKEN_ORACLE: OracleConf = OracleConf {
        pubkey: pubkey!("SomeKaminoRaydiumStrategyAccount11111111111"),
        token: 2,
        price_type: TestOracleType::KToken(DEX::Raydium),
        twap_enabled: false,
        twap_source: None,
    };

    #[tokio::test]
    async fn test_working_refresh_one_orca_ktoken() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        ctx.send_transaction(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(
            data.prices[TEST_ORCA_KTOKEN_ORACLE.token].price,
            Price { value: 1, exp: 6 }
        );
        assert!(data.prices[TEST_ORCA_KTOKEN_ORACLE.token].last_updated_slot > 0);
    }

    #[tokio::test]
    async fn test_working_refresh_one_raydium_ktoken() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_RAYDIUM_KTOKEN_ORACLE,
            &Price { value: 100, exp: 6 },
        )
        .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        ctx.send_transaction(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(
            data.prices[TEST_RAYDIUM_KTOKEN_ORACLE.token].price,
            Price { value: 100, exp: 6 }
        );
        assert!(data.prices[TEST_RAYDIUM_KTOKEN_ORACLE.token].last_updated_slot > 0);
    }

    // - [ ] Wrong kToken additional global config account
    #[tokio::test]
    async fn test_wrong_orca_ktoken_global_config() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_ORCA_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake global config
        let wrong_global_config = Pubkey::new_unique();
        ctx.clone_account(&strategy.global_config, &wrong_global_config)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong global config
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.global_config {
                account.pubkey = wrong_global_config;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional global config account
    #[tokio::test]
    async fn test_wrong_raydium_ktoken_global_config() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_RAYDIUM_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake global config
        let wrong_global_config = Pubkey::new_unique();
        ctx.clone_account(&strategy.global_config, &wrong_global_config)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong global config
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.global_config {
                account.pubkey = wrong_global_config;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional collateral infos account
    #[tokio::test]
    async fn test_wrong_orca_ktoken_collateral_infos() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_ORCA_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();
        let global_config: GlobalConfig = ctx
            .get_zero_copy_account(&strategy.global_config)
            .await
            .unwrap();

        // Create the fake collateral infos
        let wrong_token_infos = Pubkey::new_unique();
        ctx.clone_account(&global_config.token_infos, &wrong_token_infos)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong collateral infos
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == global_config.token_infos {
                account.pubkey = wrong_token_infos;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional collateral infos account
    #[tokio::test]
    async fn test_wrong_raydium_ktoken_collateral_infos() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_RAYDIUM_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_RAYDIUM_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();
        let global_config: GlobalConfig = ctx
            .get_zero_copy_account(&strategy.global_config)
            .await
            .unwrap();

        // Create the fake collateral infos
        let wrong_token_infos = Pubkey::new_unique();
        ctx.clone_account(&global_config.token_infos, &wrong_token_infos)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong collateral infos
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == global_config.token_infos {
                account.pubkey = wrong_token_infos;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional orca whirlpool account
    #[tokio::test]
    async fn test_wrong_ktoken_orca_whirlpool() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_ORCA_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake orca whirlpool
        let wrong_orca_whirlpool = Pubkey::new_unique();
        ctx.clone_account(&strategy.pool, &wrong_orca_whirlpool)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong orca whirlpool
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.pool {
                account.pubkey = wrong_orca_whirlpool;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional raydium pool account
    #[tokio::test]
    async fn test_wrong_ktoken_raydium_pool() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_RAYDIUM_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_RAYDIUM_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake raydium pool
        let wrong_raydium_pool = Pubkey::new_unique();
        ctx.clone_account(&strategy.pool, &wrong_raydium_pool).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong orca whirlpool
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.pool {
                account.pubkey = wrong_raydium_pool;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional orca position account
    #[tokio::test]
    async fn test_wrong_ktoken_orca_position() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_ORCA_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake orca position
        let wrong_orca_position = Pubkey::new_unique();
        ctx.clone_account(&strategy.position, &wrong_orca_position)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong orca position
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.position {
                account.pubkey = wrong_orca_position;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional raydium position account
    #[tokio::test]
    async fn test_wrong_ktoken_raydium_position() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_RAYDIUM_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_RAYDIUM_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake orca position
        let wrong_orca_position = Pubkey::new_unique();
        ctx.clone_account(&strategy.position, &wrong_orca_position)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        accounts.append(&mut refresh_accounts);
        // Set the wrong orca position
        accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.position {
                account.pubkey = wrong_orca_position;
            }
        });

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional scope prices account
    #[tokio::test]
    async fn test_wrong_orca_ktoken_scope_prices() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_ORCA_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_ORCA_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_ORCA_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake scope prices
        let wrong_scope_prices = Pubkey::new_unique();
        ctx.clone_account(&strategy.scope_prices, &wrong_scope_prices)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_ORCA_KTOKEN_ORACLE)
                .await;
        // Set the wrong scope prices
        refresh_accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.scope_prices {
                account.pubkey = wrong_scope_prices;
            }
        });
        accounts.append(&mut refresh_accounts);

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_ORCA_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }

    // - [ ] Wrong kToken additional scope prices account
    #[tokio::test]
    async fn test_wrong_raydium_ktoken_scope_prices() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_RAYDIUM_KTOKEN_ORACLE]).await;

        // Change price
        mock_oracles::set_price(
            &mut ctx,
            &feed,
            &TEST_RAYDIUM_KTOKEN_ORACLE,
            &Price { value: 1, exp: 6 },
        )
        .await;

        let strategy: WhirlpoolStrategy = ctx
            .get_zero_copy_account(&TEST_RAYDIUM_KTOKEN_ORACLE.pubkey)
            .await
            .unwrap();

        // Create the fake scope prices
        let wrong_scope_prices = Pubkey::new_unique();
        ctx.clone_account(&strategy.scope_prices, &wrong_scope_prices)
            .await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);
        let mut refresh_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_RAYDIUM_KTOKEN_ORACLE)
                .await;
        refresh_accounts.iter_mut().for_each(|account| {
            if account.pubkey == strategy.scope_prices {
                account.pubkey = wrong_scope_prices;
            }
        });
        accounts.append(&mut refresh_accounts);

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_RAYDIUM_KTOKEN_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts,
            data: args.data(),
        };

        let res = ctx.send_transaction(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }
}

mod test_jlp_fetch {
    use super::*;

    const TEST_JLP_ORACLE: OracleConf = OracleConf {
        pubkey: pubkey!("SomeJLPPriceAccount111111111111111111111111"),
        token: 0,
        price_type: TestOracleType::JupiterLPFetch,
        twap_enabled: false,
        twap_source: None,
    };

    #[tokio::test]
    async fn test_working_refresh_one_jlp() {
        let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_JLP_ORACLE]).await;

        let price = Price {
            value: 1000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_JLP_ORACLE, &price).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_JLP_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_JLP_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_eq!(data.prices[TEST_JLP_ORACLE.token].price, price);
    }

    #[tokio::test]
    async fn test_refresh_one_jlp_wrong_mint() {
        let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_JLP_ORACLE]).await;

        let price = Price {
            value: 1000,
            exp: 1,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_JLP_ORACLE, &price).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        let mut remaining_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_JLP_ORACLE).await;

        let mint = &mut remaining_accounts[0];
        let mint_pk = mint.pubkey;
        let cloned_mint = Pubkey::new_unique();
        ctx.clone_account(&mint_pk, &cloned_mint).await;
        mint.pubkey = cloned_mint;
        accounts.append(&mut remaining_accounts);

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_JLP_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        let res = ctx.send_transaction_with_bot(&[ix]).await;

        assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
    }
}

mod test_jlp_compute {
    use super::*;

    const TEST_JLP_ORACLE: OracleConf = OracleConf {
        pubkey: pubkey!("SomeJLPPriceAccount111111111111111111111111"),
        token: 0,
        price_type: TestOracleType::JupiterLpCompute,
        twap_enabled: false,
        twap_source: None,
    };

    #[tokio::test]
    async fn test_working_refresh_one_jlp() {
        let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_JLP_ORACLE]).await;

        let price = Price {
            value: 1000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_JLP_ORACLE, &price).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_JLP_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_JLP_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_fuzzy_price_eq!(
            data.prices[TEST_JLP_ORACLE.token].price,
            price,
            decimal_wad::decimal::Decimal::from(price) / 1000,
            "Price {:?}",
            data.prices[TEST_JLP_ORACLE.token]
        );
    }

    // All extra accounts at once
    // - [x] Wrong Jupiter LP additional mint account
    // - [x] Wrong Jupiter LP additional custodies account
    // - [x] Wrong Jupiter LP additional oracles account
    #[tokio::test]
    async fn test_refresh_one_jlp_wrong_extra_accounts() {
        let (mut ctx, feed) = fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_JLP_ORACLE]).await;

        let price = Price {
            value: 1000,
            exp: 3,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_JLP_ORACLE, &price).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        let remaining_accounts =
            utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_JLP_ORACLE).await;

        for i in 0..remaining_accounts.len() {
            let mut remaining_accounts = remaining_accounts.clone();
            let acc = &mut remaining_accounts[i];
            let acc_pk = acc.pubkey;
            let cloned_acc = Pubkey::new_unique();
            ctx.clone_account(&acc_pk, &cloned_acc).await;
            acc.pubkey = cloned_acc;
            accounts.append(&mut remaining_accounts);

            let args = scope::instruction::RefreshPriceList {
                tokens: vec![TEST_JLP_ORACLE.token.try_into().unwrap()],
            };

            let ix = Instruction {
                program_id: scope::id(),
                accounts: accounts.to_account_metas(None),
                data: args.data(),
            };

            let res = ctx.send_transaction_with_bot(&[ix]).await;

            assert_eq!(map_scope_error(res), ScopeError::UnexpectedAccount);
        }
    }
}

mod test_spl_stake {
    use super::*;
    const TEST_STAKE_ORACLE: OracleConf = OracleConf {
        pubkey: pubkey!("SomeStakePriceAccount1111111111111111111111"),
        token: 0,
        price_type: TestOracleType::SplStake,
        twap_enabled: false,
        twap_source: None,
    };

    // - [] Working case, refreshed in current epoch
    #[tokio::test]
    async fn test_working_refresh_one_spl_stake() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_fuzzy_price_eq!(
            data.prices[TEST_STAKE_ORACLE.token].price,
            price,
            decimal_wad::decimal::Decimal::from(price) / 10000,
            "Price {:?}",
            data.prices[TEST_STAKE_ORACLE.token]
        );
    }

    // - [] Working case, refreshed in previous epoch but less than 1hour ago
    #[tokio::test]
    async fn test_working_refresh_one_new_epoch_spl_stake() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Change epoch + 30 minutes
        let mut clock = ctx.get_clock().await;
        clock.epoch += 1;
        clock.epoch_start_timestamp = clock.unix_timestamp;
        clock.unix_timestamp += 30 * 60;
        ctx.context.set_sysvar(&clock);

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        ctx.send_transaction_with_bot(&[ix]).await.unwrap();

        // Check price
        let data: OraclePrices = ctx.get_zero_copy_account(&feed.prices).await.unwrap();
        assert_fuzzy_price_eq!(
            data.prices[TEST_STAKE_ORACLE.token].price,
            price,
            decimal_wad::decimal::Decimal::from(price) / 10000,
            "Price {:?}",
            data.prices[TEST_STAKE_ORACLE.token]
        );
    }
    // - [] Fail case, refreshed in previous epoch but more than 1hour ago
    #[tokio::test]
    async fn test_fail_refresh_one_new_epoch_spl_stake() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Change epoch + 61 minutes
        let mut clock = ctx.get_clock().await;
        clock.epoch += 1;
        clock.epoch_start_timestamp = clock.unix_timestamp;
        clock.unix_timestamp += 61 * 60;
        ctx.context.set_sysvar(&clock);

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        let res = ctx.send_transaction_with_bot(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::PriceNotValid);
    }

    // - [] Fail case, refreshed in more than 1 epoch ago and epoch started less than 1hour ago
    #[tokio::test]
    async fn test_fail_refresh_one_old_epoch_just_started_spl_stake() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Change 2 epoch + 30 minutes
        let mut clock = ctx.get_clock().await;
        clock.epoch += 2;
        clock.epoch_start_timestamp = clock.unix_timestamp;
        clock.unix_timestamp += 30 * 60;
        ctx.context.set_sysvar(&clock);

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        let res = ctx.send_transaction_with_bot(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::PriceNotValid);
    }

    #[tokio::test]
    async fn test_fail_refresh_one_old_epoch_just_started_spl_stake_2() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Change 10 epoch + 30 minutes
        let mut clock = ctx.get_clock().await;
        clock.epoch += 10;
        clock.epoch_start_timestamp = clock.unix_timestamp;
        clock.unix_timestamp += 30 * 60;
        ctx.context.set_sysvar(&clock);

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        let res = ctx.send_transaction_with_bot(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::PriceNotValid);
    }

    #[tokio::test]
    async fn test_fail_refresh_one_old_epoch_just_started_spl_stake_3() {
        let (mut ctx, feed) =
            fixtures::setup_scope(DEFAULT_FEED_NAME, vec![TEST_STAKE_ORACLE]).await;

        let price = Price {
            value: 1100000,
            exp: 6,
        };
        // Change price
        mock_oracles::set_price(&mut ctx, &feed, &TEST_STAKE_ORACLE, &price).await;

        // Change 2 epoch + 61 minutes
        let mut clock = ctx.get_clock().await;
        clock.epoch += 2;
        clock.epoch_start_timestamp = clock.unix_timestamp;
        clock.unix_timestamp += 30 * 60;
        ctx.context.set_sysvar(&clock);

        // Refresh
        let mut accounts = scope::accounts::RefreshList {
            oracle_prices: feed.prices,
            oracle_mappings: feed.mapping,
            oracle_twaps: feed.twaps,
            instruction_sysvar_account_info: SYSVAR_INSTRUCTIONS_ID,
        }
        .to_account_metas(None);

        accounts.append(
            &mut utils::get_refresh_list_accounts(&mut ctx, &feed.prices, &TEST_STAKE_ORACLE).await,
        );

        let args = scope::instruction::RefreshPriceList {
            tokens: vec![TEST_STAKE_ORACLE.token.try_into().unwrap()],
        };

        let ix = Instruction {
            program_id: scope::id(),
            accounts: accounts.to_account_metas(None),
            data: args.data(),
        };

        let res = ctx.send_transaction_with_bot(&[ix]).await;
        assert_eq!(map_scope_error(res), ScopeError::PriceNotValid);
    }
}
