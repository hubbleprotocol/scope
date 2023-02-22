use anchor_lang::prelude::Pubkey;
use scope::{OracleMappings, OraclePrices};
use solana_program_test::ProgramTest;
use solana_sdk::{
    account::Account, commitment_config::CommitmentLevel, signature::Keypair, signer::Signer,
    system_instruction, system_program, transaction::Transaction,
};
use types::TestContext;

use super::{types::ScopeZeroCopyAccounts, *};

pub async fn new_keypair(ctx: &mut TestContext, min_lamports: u64) -> Keypair {
    let account = Keypair::new();
    let transaction = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &ctx.context.payer.pubkey(),
            &account.pubkey(),
            min_lamports,
            0,
            &system_program::id(),
        )],
        Some(&ctx.context.payer.pubkey()),
        &[&ctx.context.payer, &account],
        ctx.context
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap(),
    );

    ctx.context
        .banks_client
        .process_transaction_with_commitment(transaction, CommitmentLevel::Processed)
        .await
        .unwrap();

    account
}

pub fn fund_kp(test: &mut ProgramTest, min_balance_lamports: u64, user: Pubkey) {
    test.add_account(
        user,
        Account {
            lamports: min_balance_lamports,
            ..Account::default()
        },
    );
}

pub fn funded_kp(test: &mut ProgramTest, min_balance_lamports: u64) -> Keypair {
    let kp = Keypair::new();
    fund_kp(test, min_balance_lamports, kp.pubkey());
    kp
}

impl ScopeZeroCopyAccounts {
    pub fn new() -> Self {
        Self {
            mapping: Keypair::new(),
            prices: Keypair::new(),
        }
    }

    pub fn add_accounts(&self, test: &mut ProgramTest) {
        test.add_account(
            self.mapping.pubkey(),
            Account::new(
                u32::MAX as u64,
                std::mem::size_of::<OracleMappings>() + 8,
                &scope::ID,
            ),
        );
        test.add_account(
            self.prices.pubkey(),
            Account::new(
                u32::MAX as u64,
                std::mem::size_of::<OraclePrices>() + 8,
                &scope::ID,
            ),
        );
    }
}
