use std::fmt::Debug;

use num_enum::TryFromPrimitive;
use scope::oracles::OracleType;
use solana_program::instruction::{AccountMeta, InstructionError};
use solana_program_test::BanksClientError;
use solana_sdk::transaction::TransactionError;

use crate::common::types::{OracleConf, TestContext};

pub async fn get_refresh_list_accounts(
    ctx: &mut TestContext,
    conf: &OracleConf,
) -> Vec<AccountMeta> {
    let mut accounts: Vec<AccountMeta> = vec![];
    let mut remaining_accounts = get_remaining_accounts(ctx, conf).await;
    accounts.push(AccountMeta::new_readonly(conf.pubkey, false));
    accounts.append(&mut remaining_accounts);
    accounts
}

pub async fn get_remaining_accounts(ctx: &mut TestContext, conf: &OracleConf) -> Vec<AccountMeta> {
    #[allow(unused_mut)]
    let mut accounts: Vec<AccountMeta> = vec![];
    match conf.price_type.into() {
        #[cfg(feature = "yvaults")]
        OracleType::KToken | OracleType::KTokenToTokenA | OracleType::KTokenToTokenB => {
            accounts.append(&mut ktokens::get_ktoken_remaining_accounts(ctx, conf).await);
        }
        #[cfg(not(feature = "yvaults"))]
        OracleType::KToken | OracleType::KTokenToTokenA | OracleType::KTokenToTokenB => {
            let _ = ctx;
            panic!("KToken oracle type is not supported")
        }
        // Single account oracles
        OracleType::Pyth
        | OracleType::SwitchboardV2
        | OracleType::SplStake
        | OracleType::PythEMA
        | OracleType::MsolStake
        | OracleType::ScopeTwap
        | OracleType::RaydiumAmmV3AtoB
        | OracleType::RaydiumAmmV3BtoA => {}
        OracleType::JupiterLpFetch => {
            accounts.extend_from_slice(&get_jlp_fetch_remaining_accounts(conf))
        }
        OracleType::CToken => panic!("CToken is not supported in tests"),
        OracleType::OrcaWhirlpoolAtoB | OracleType::OrcaWhirlpoolBtoA => {
            accounts.extend_from_slice(&get_orca_whirlpool_remaining_accounts(ctx, conf).await)
        }
        OracleType::JupiterLpCompute => {
            accounts.append(&mut get_jlp_compute_remaining_accounts(ctx, conf).await)
        }
        OracleType::MeteoraDlmmAtoB | OracleType::MeteoraDlmmBtoA => {
            unimplemented!("MeteoraDlmm is not yet supported in tests")
        }
        OracleType::DeprecatedPlaceholder1 | OracleType::DeprecatedPlaceholder2 => {
            panic!("DeprecatedPlaceholder is not a valid oracle type")
        }
    }
    accounts
}

pub fn get_jlp_fetch_remaining_accounts(conf: &OracleConf) -> [AccountMeta; 1] {
    use scope::oracles::jupiter_lp as jlp;
    let (mint_pk, _) = jlp::get_mint_pk(&conf.pubkey);
    [AccountMeta::new_readonly(mint_pk, false)]
}

pub async fn get_jlp_compute_remaining_accounts(
    ctx: &mut TestContext,
    conf: &OracleConf,
) -> Vec<AccountMeta> {
    use scope::oracles::jupiter_lp as jlp;
    let pool: jlp::perpetuals::Pool = ctx.get_anchor_account(&conf.pubkey).await.unwrap();
    let (mint_pk, _) = jlp::get_mint_pk(&conf.pubkey);
    let custodies_pks = &pool.custodies;
    let oracles_pk_it = ctx
        .get_anchor_accounts::<jlp::perpetuals::Custody>(custodies_pks)
        .await
        .unwrap()
        .into_iter()
        .map(|c: jlp::perpetuals::Custody| c.oracle.oracle_account);

    let mut accounts = vec![AccountMeta::new_readonly(mint_pk, false)];
    accounts.extend(
        custodies_pks
            .iter()
            .map(|pk| AccountMeta::new_readonly(*pk, false)),
    );
    accounts.extend(oracles_pk_it.map(|pk| AccountMeta::new_readonly(pk, false)));
    accounts
}

pub async fn get_orca_whirlpool_remaining_accounts(
    ctx: &mut TestContext,
    conf: &OracleConf,
) -> [AccountMeta; 2] {
    let pool: whirlpool::state::Whirlpool = ctx.get_anchor_account(&conf.pubkey).await.unwrap();
    [
        AccountMeta::new_readonly(pool.token_mint_a, false),
        AccountMeta::new_readonly(pool.token_mint_b, false),
    ]
}

#[cfg(feature = "yvaults")]
mod ktokens {
    use kamino::state::{GlobalConfig, WhirlpoolStrategy};
    use yvaults as kamino;

    use super::*;

    pub async fn get_ktoken_remaining_accounts(
        ctx: &mut TestContext,
        conf: &OracleConf,
    ) -> Vec<AccountMeta> {
        let strategy: WhirlpoolStrategy = ctx.get_zero_copy_account(&conf.pubkey).await.unwrap();
        let global_config: GlobalConfig = ctx
            .get_zero_copy_account(&strategy.global_config)
            .await
            .unwrap();

        vec![
            AccountMeta::new_readonly(strategy.global_config, false),
            AccountMeta::new_readonly(global_config.token_infos, false),
            AccountMeta::new_readonly(strategy.pool, false),
            AccountMeta::new_readonly(strategy.position, false),
            AccountMeta::new_readonly(strategy.scope_prices, false),
        ]
    }
}

pub fn map_scope_error<T: Debug>(res: Result<T, BanksClientError>) -> scope::ScopeError {
    if let Err(BanksClientError::TransactionError(TransactionError::InstructionError(
        _y,
        InstructionError::Custom(z),
    ))) = &res
    {
        if *z >= 6000 {
            let z: scope::ScopeError = scope::ScopeError::try_from_primitive(*z - 6000).unwrap(); // as borrowing::BorrowError;
            return z;
        }
    }
    panic!("Result is {:?}", res)
}

pub fn map_custom_error(res: Result<(), BanksClientError>) -> u32 {
    if let Err(BanksClientError::TransactionError(TransactionError::InstructionError(
        _y,
        InstructionError::Custom(z),
    ))) = &res
    {
        return *z;
    }
    panic!("Result is {:?}", res)
}

pub fn map_anchor_error(res: Result<(), BanksClientError>) -> AnchorErrorCode {
    let error_code = map_custom_error(res);
    AnchorErrorCode::try_from(error_code).unwrap()
}

pub fn map_tx_error<T: Debug>(res: Result<T, BanksClientError>) -> TransactionError {
    if let Err(BanksClientError::TransactionError(x)) = res {
        return x;
    }
    panic!("Result is {:?}", res)
}

#[derive(Debug, TryFromPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum AnchorErrorCode {
    // Instructions
    /// 100 - 8 byte instruction identifier not provided
    InstructionMissing = 100,
    /// 101 - Fallback functions are not supported
    InstructionFallbackNotFound,
    /// 102 - The program could not deserialize the given instruction
    InstructionDidNotDeserialize,
    /// 103 - The program could not serialize the given instruction
    InstructionDidNotSerialize,

    // IDL instructions
    /// 1000 - The program was compiled without idl instructions
    IdlInstructionStub = 1000,
    /// 1001 - Invalid program given to the IDL instruction
    IdlInstructionInvalidProgram,

    // Constraints
    /// 2000 - A mut constraint was violated
    ConstraintMut = 2000,
    /// 2001 - A has one constraint was violated
    ConstraintHasOne,
    /// 2002 - A signer constraint was violated
    ConstraintSigner,
    /// 2003 - A raw constraint was violated
    ConstraintRaw,
    /// 2004 - An owner constraint was violated
    ConstraintOwner,
    /// 2005 - A rent exemption constraint was violated
    ConstraintRentExempt,
    /// 2006 - A seeds constraint was violated
    ConstraintSeeds,
    /// 2007 - An executable constraint was violated
    ConstraintExecutable,
    /// 2008 - A state constraint was violated
    ConstraintState,
    /// 2009 - An associated constraint was violated
    ConstraintAssociated,
    /// 2010 - An associated init constraint was violated
    ConstraintAssociatedInit,
    /// 2011 - A close constraint was violated
    ConstraintClose,
    /// 2012 - An address constraint was violated
    ConstraintAddress,
    /// 2013 - Expected zero account discriminant
    ConstraintZero,
    /// 2014 - A token mint constraint was violated
    ConstraintTokenMint,
    /// 2015 - A token owner constraint was violated
    ConstraintTokenOwner,
    /// The mint mint is intentional -> a mint authority for the mint.
    ///
    /// 2016 - A mint mint authority constraint was violated
    ConstraintMintMintAuthority,
    /// 2017 - A mint freeze authority constraint was violated
    ConstraintMintFreezeAuthority,
    /// 2018 - A mint decimals constraint was violated
    ConstraintMintDecimals,
    /// 2019 - A space constraint was violated
    ConstraintSpace,

    // Accounts.
    /// 3000 - The account discriminator was already set on this account
    AccountDiscriminatorAlreadySet = 3000,
    /// 3001 - No 8 byte discriminator was found on the account
    AccountDiscriminatorNotFound,
    /// 3002 - 8 byte discriminator did not match what was expected
    AccountDiscriminatorMismatch,
    /// 3003 - Failed to deserialize the account
    AccountDidNotDeserialize,
    /// 3004 - Failed to serialize the account
    AccountDidNotSerialize,
    /// 3005 - Not enough account keys given to the instruction
    AccountNotEnoughKeys,
    /// 3006 - The given account is not mutable
    AccountNotMutable,
    /// 3007 - The given account is owned by a different program than expected
    AccountOwnedByWrongProgram,
    /// 3008 - Program ID was not as expected
    InvalidProgramId,
    /// 3009 - Program account is not executable
    InvalidProgramExecutable,
    /// 3010 - The given account did not sign
    AccountNotSigner,
    /// 3011 - The given account is not owned by the system program
    AccountNotSystemOwned,
    /// 3012 - The program expected this account to be already initialized
    AccountNotInitialized,
    /// 3013 - The given account is not a program data account
    AccountNotProgramData,
    /// 3014 - The given account is not the associated token account
    AccountNotAssociatedTokenAccount,

    // State.
    /// 4000 - The given state account does not have the correct address
    StateInvalidAddress = 4000,

    // Deprecated
    /// 5000 - The API being used is deprecated and should no longer be used
    Deprecated = 5000,
}
