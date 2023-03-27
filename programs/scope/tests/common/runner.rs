use anchor_lang::prelude::AccountMeta;
use anchor_lang::{
    prelude::{Clock, Pubkey},
    Owner,
};
use solana_program::pubkey;
use solana_program_test::{processor, BanksClientError, ProgramTest};
use solana_sdk::{
    account::{Account, AccountSharedData},
    instruction::Instruction,
    signature::Keypair,
    signer::Signer,
    signers::Signers,
    transaction::Transaction,
};
use types::*;

use super::*;

pub const TEST_CPI_CALLER_PK: Pubkey = pubkey!("cpiCaLL111111111111111111111111111111111111");

pub fn program() -> ProgramTest {
    let mut prog = ProgramTest::new("scope", scope::ID, processor!(scope::entry));
    prog.add_program(
        "test_cpi_caller",
        TEST_CPI_CALLER_PK,
        processor!(test_cpi_caller::process_instruction),
    );
    prog
}

pub async fn start(test: ProgramTest, admin: Keypair, bot: Keypair) -> TestContext {
    let mut context = test.start_with_context().await;
    let rent = context.banks_client.get_rent().await.unwrap();

    TestContext {
        context,
        rent,
        admin,
        bot,
        token_confs: Vec::new(),
    }
}

impl TestContext {
    pub async fn get_account(&mut self, pubkey: &Pubkey) -> Result<Account, TestError> {
        self.context
            .banks_client
            .get_account(*pubkey)
            .await?
            .ok_or(TestError::AccountNotFound)
    }

    pub async fn get_account_data(&mut self, pubkey: &Pubkey) -> Result<Vec<u8>, TestError> {
        let account = self.get_account(pubkey).await?;
        Ok(account.data)
    }

    pub async fn get_anchor_account<T: anchor_lang::AccountDeserialize>(
        &mut self,
        pubkey: &Pubkey,
    ) -> Result<T, TestError> {
        let account_data = self.get_account_data(pubkey).await?;
        let mut data_ref = &account_data[..];
        T::try_deserialize(&mut data_ref).map_err(|_| TestError::CannotDeserialize)
    }

    pub async fn get_zero_copy_account<T: anchor_lang::ZeroCopy>(
        &mut self,
        pubkey: &Pubkey,
    ) -> Result<T, TestError> {
        let account_data = self.get_account_data(pubkey).await?;
        if account_data.len() < 8 {
            return Err(TestError::BadDiscriminator);
        }
        if account_data[0..8] != T::DISCRIMINATOR {
            return Err(TestError::BadDiscriminator);
        }
        let data_ref = &account_data[8..];
        bytemuck::try_from_bytes(data_ref)
            .map_err(|_| TestError::CannotDeserialize)
            .copied()
    }

    /// Set an account data to the given values.
    ///
    /// Warning: this function will overwrite the account data if it already exists.
    pub fn set_account(&mut self, pubkey: &Pubkey, data: Vec<u8>, owner: &Pubkey) {
        let mut updated_account = AccountSharedData::new(u64::MAX / 2, data.len(), owner);
        updated_account.set_data(data);
        self.context.set_account(pubkey, &updated_account);
    }

    pub fn set_anchor_account<T: anchor_lang::AccountSerialize + Owner + Sized>(
        &mut self,
        pubkey: &Pubkey,
        account: &T,
    ) {
        let mut data: Vec<u8> = Vec::with_capacity(std::mem::size_of::<T>());
        account.try_serialize(&mut data).unwrap();
        self.set_account(pubkey, data, &T::owner());
    }

    pub fn set_zero_copy_account<T: anchor_lang::ZeroCopy + Owner + Sized>(
        &mut self,
        pubkey: &Pubkey,
        account: &T,
    ) {
        let mut data: Vec<u8> = Vec::with_capacity(std::mem::size_of::<T>() + 8);
        data.extend_from_slice(&T::DISCRIMINATOR);
        data.extend_from_slice(bytemuck::bytes_of(account));
        self.set_account(pubkey, data, &T::owner());
    }

    pub async fn clone_account(&mut self, previous_address: &Pubkey, new_address: &Pubkey) {
        let account_to_clone = self
            .context
            .banks_client
            .get_account(*previous_address)
            .await
            .unwrap()
            .unwrap();
        let mut cloned_account = AccountSharedData::new(
            account_to_clone.lamports,
            account_to_clone.data.len(),
            &account_to_clone.owner,
        );
        cloned_account.set_data(account_to_clone.data);
        self.context.set_account(new_address, &cloned_account);
    }

    pub async fn clone_account_with_different_owner(
        &mut self,
        previous_address: Pubkey,
        new_address: &Pubkey,
        new_owner: &Pubkey,
    ) {
        let account_to_clone = self
            .context
            .banks_client
            .get_account(previous_address)
            .await
            .unwrap()
            .unwrap();
        let mut cloned_account = AccountSharedData::new(
            account_to_clone.lamports,
            account_to_clone.data.len(),
            new_owner,
        );
        cloned_account.set_data(account_to_clone.data);
        self.context.set_account(new_address, &cloned_account);
    }

    pub async fn get_clock(&mut self) -> Clock {
        self.context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .unwrap()
    }

    pub async fn get_now_timestamp(&mut self) -> u64 {
        let clock: Clock = self
            .context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .unwrap();
        clock.unix_timestamp as u64
    }

    pub async fn send_transaction(&mut self, ixs: &[Instruction]) -> Result<(), BanksClientError> {
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.admin.pubkey()),
            &[&self.admin],
            self.context.banks_client.get_latest_blockhash().await?,
        );
        self.context.banks_client.process_transaction(tx).await
    }

    pub async fn send_transaction_through_cpi(
        &mut self,
        ixs: &[Instruction],
    ) -> Result<(), BanksClientError> {
        let instruction_cpi: Vec<Instruction> = ixs
            .iter()
            .map(|ix| {
                let mut cpi_accounts = Vec::with_capacity(ix.accounts.len() + 1);
                cpi_accounts.push(AccountMeta::new_readonly(ix.program_id, false));
                cpi_accounts.extend_from_slice(&ix.accounts);
                Instruction::new_with_bytes(TEST_CPI_CALLER_PK, &ix.data, cpi_accounts)
            })
            .collect();
        let tx = Transaction::new_signed_with_payer(
            &instruction_cpi,
            Some(&self.admin.pubkey()),
            &[&self.admin],
            self.context.banks_client.get_latest_blockhash().await?,
        );
        self.context.banks_client.process_transaction(tx).await
    }

    pub async fn send_transaction_with_bot(
        &mut self,
        ixs: &[Instruction],
    ) -> Result<(), BanksClientError> {
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.bot.pubkey()),
            &[&self.bot],
            self.context.banks_client.get_latest_blockhash().await?,
        );
        self.context.banks_client.process_transaction(tx).await
    }

    pub async fn send_transaction_with_payer(
        &mut self,
        ixs: &[Instruction],
        payer: &Keypair,
    ) -> Result<(), BanksClientError> {
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&payer.pubkey()),
            &[payer],
            self.context.banks_client.get_latest_blockhash().await?,
        );
        self.context.banks_client.process_transaction(tx).await
    }

    pub async fn send_transaction_with_signers_and_payer<T: Signers>(
        &mut self,
        ixs: &[Instruction],
        signers: &T,
        payer: &Pubkey,
    ) -> Result<(), BanksClientError> {
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(payer),
            signers,
            self.context.banks_client.get_latest_blockhash().await?,
        );
        self.context.banks_client.process_transaction(tx).await
    }
}
