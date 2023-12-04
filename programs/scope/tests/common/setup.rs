use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use scope::{accounts::Initialize, OracleMappings, OraclePrices, OracleTwaps, TokenMetadatas};
use solana_program::instruction::Instruction;
use solana_program_test::ProgramTest;
use solana_sdk::{
    account::Account, commitment_config::CommitmentLevel, signature::Keypair, signer::Signer,
    system_instruction, system_program, transaction::Transaction,
};
use types::TestContext;

use super::{
    types::{ScopeFeedDefinition, ScopeZeroCopyAccounts},
    *,
};

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
            token_metadatas: Keypair::new(),
            oracle_twaps: Keypair::new(),
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
        test.add_account(
            self.token_metadatas.pubkey(),
            Account::new(
                u32::MAX as u64,
                std::mem::size_of::<TokenMetadatas>() + 8,
                &scope::ID,
            ),
        );
        test.add_account(
            self.oracle_twaps.pubkey(),
            Account::new(
                2 * u32::MAX as u64,
                std::mem::size_of::<OracleTwaps>() + 8,
                &scope::ID,
            ),
        )
    }
}

pub async fn setup_scope_feed() -> (TestContext, ScopeFeedDefinition) {
    let mut test_program = runner::program();
    let admin = funded_kp(&mut test_program, 100000000);
    let zero_copy_accounts = types::ScopeZeroCopyAccounts::new();
    zero_copy_accounts.add_accounts(&mut test_program);
    let mut ctx = runner::start(test_program, admin, Keypair::new()).await;
    let (configuration_acc, _) =
        Pubkey::find_program_address(&[b"conf", DEFAULT_FEED_NAME.as_bytes()], &scope::id());
    let accounts = Initialize {
        admin: ctx.admin.pubkey(),
        system_program: solana_program::system_program::id(),
        configuration: configuration_acc,
        oracle_prices: zero_copy_accounts.prices.pubkey(),
        oracle_mappings: zero_copy_accounts.mapping.pubkey(),
        token_metadatas: zero_copy_accounts.token_metadatas.pubkey(),
        oracle_twaps: zero_copy_accounts.oracle_twaps.pubkey(),
    };
    let args = scope::instruction::Initialize {
        feed_name: DEFAULT_FEED_NAME.to_string(),
    };

    let ix = Instruction {
        program_id: scope::id(),
        accounts: accounts.to_account_metas(None),
        data: args.data(),
    };

    ctx.send_transaction(&[ix]).await.unwrap();

    (
        ctx,
        ScopeFeedDefinition {
            feed_name: DEFAULT_FEED_NAME.to_string(),
            conf: configuration_acc,
            mapping: zero_copy_accounts.mapping.pubkey(),
            prices: zero_copy_accounts.prices.pubkey(),
            twaps: zero_copy_accounts.oracle_twaps.pubkey(),
        },
    )
}
