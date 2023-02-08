#![doc = include_str!("../Readme.md")]

use anchor_client::{
    anchor_lang::AccountDeserialize,
    solana_sdk::{
        address_lookup_table_account::AddressLookupTableAccount,
        commitment_config::CommitmentConfig,
        instruction::Instruction,
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::Signature,
        signer::Signer,
        system_instruction,
        transaction::{TransactionError, VersionedTransaction},
    },
};
use errors::ErrorKind;
use futures::future::join_all;

pub mod async_client;
pub mod consts;
pub mod errors;
pub mod tx_builder;

pub use consts::*;

type Result<T> = std::result::Result<T, errors::ErrorKind>;

/// Transaction result. `Ok` if the transaction was successful, `Err` from the transaction otherwise.
type TransactionResult = std::result::Result<(), TransactionError>;

pub struct OrbitLink<T, S>
where
    T: async_client::AsyncClient,
    S: Signer,
{
    pub client: T,
    payer: S,
    lookup_tables: Vec<AddressLookupTableAccount>,
    commitment_config: CommitmentConfig,
}

impl<T, S> OrbitLink<T, S>
where
    T: async_client::AsyncClient,
    S: Signer,
{
    pub fn new(
        client: T,
        payer: S,
        lookup_tables: impl Into<Option<Vec<AddressLookupTableAccount>>>,
        commitment_config: CommitmentConfig,
    ) -> Self {
        let lookup_tables: Option<Vec<AddressLookupTableAccount>> = lookup_tables.into();
        OrbitLink {
            client,
            payer,
            lookup_tables: lookup_tables.unwrap_or_default(),
            commitment_config,
        }
    }

    pub fn payer(&self) -> Pubkey {
        self.payer.pubkey()
    }

    pub fn add_lookup_table(&mut self, table: AddressLookupTableAccount) {
        self.lookup_tables.push(table);
    }

    pub async fn get_anchor_account<AccDeser: AccountDeserialize>(
        &self,
        pubkey: &Pubkey,
    ) -> Result<AccDeser> {
        let account = self.client.get_account(pubkey).await?;
        let mut data: &[u8] = &account.data;
        Ok(AccDeser::try_deserialize(&mut data)?)
    }

    pub fn tx_builder(&self) -> tx_builder::TxBuilder<T, S> {
        tx_builder::TxBuilder::new(self)
    }

    pub async fn create_account_ix(
        &self,
        account_to_create: &Pubkey,
        space: usize,
        new_owner: &Pubkey,
    ) -> Result<Instruction> {
        Ok(system_instruction::create_account(
            &self.payer(),
            account_to_create,
            self.client
                .get_minimum_balance_for_rent_exemption(space)
                .await?,
            space
                .try_into()
                .expect("usize representing size to allocate to u64 conversion failed"),
            new_owner,
        ))
    }

    pub async fn create_tx(
        &self,
        instructions: &[Instruction],
        extra_signers: &[&dyn Signer],
    ) -> Result<VersionedTransaction> {
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(extra_signers.len() + 1);
        signers.push(&self.payer);
        signers.extend_from_slice(extra_signers);

        Ok(VersionedTransaction::try_new(
            VersionedMessage::V0(
                v0::Message::try_compile(
                    &self.payer.pubkey(),
                    instructions,
                    &self.lookup_tables,
                    // TODO: cache blockhash
                    self.client.get_latest_blockhash().await?,
                )
                .map_err(|e| ErrorKind::TransactionCompileError(e.to_string()))?,
            ),
            &signers,
        )?)
    }

    pub async fn create_tx_with_extra_lookup_tables(
        &self,
        instructions: &[Instruction],
        extra_signers: &[&dyn Signer],
        lookup_tables_extra: &[AddressLookupTableAccount],
    ) -> Result<VersionedTransaction> {
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(extra_signers.len() + 1);
        signers.push(&self.payer);
        signers.extend_from_slice(extra_signers);

        let mut lookup_tables = self.lookup_tables.clone();
        lookup_tables.extend_from_slice(lookup_tables_extra);

        Ok(VersionedTransaction::try_new(
            VersionedMessage::V0(
                v0::Message::try_compile(
                    &self.payer.pubkey(),
                    instructions,
                    &lookup_tables,
                    // TODO: cache blockhash
                    self.client.get_latest_blockhash().await?,
                )
                .map_err(|e| ErrorKind::TransactionCompileError(e.to_string()))?,
            ),
            &signers,
        )?)
    }

    pub async fn send_transaction(&self, tx: &VersionedTransaction) -> Result<Signature> {
        self.client.send_transaction(tx).await
    }

    /// Send a group of transactions and wait for them to be confirmed.
    /// Transactions are not guaranteed to be processed in the same order as they are sent.
    ///
    /// Note: In case of early error while sending, it is possible to loose track of which transaction
    /// failed and which succeeded.
    ///
    /// Returns a vector of (signature, result) where result is None if the transaction is was not confirmed.
    pub async fn send_and_confirm_transactions(
        &self,
        transactions: &[VersionedTransaction],
    ) -> Result<Vec<(Signature, Option<TransactionResult>)>> {
        let signatures = join_all(transactions.iter().map(|tx| self.send_transaction(tx)))
            .await
            .into_iter()
            .collect::<Result<Vec<Signature>>>()?;
        let mut tx_to_confirm: Vec<(Signature, Option<TransactionResult>)> = signatures
            .into_iter()
            .zip(std::iter::repeat(None))
            .collect();

        self.confirm_transactions(
            &mut tx_to_confirm,
            self.commitment_config,
            commitment_to_retry_count(self.commitment_config),
        )
        .await?;

        Ok(tx_to_confirm)
    }

    /// Send a group of transactions and wait for them to be confirmed.
    /// Transactions that are not confirmed to the "processed" commitment level are retried once.
    /// Transactions are not guaranteed to be processed in the same order as they are sent.
    ///
    /// Note: In case of early error while sending, it is possible to loose track of which transaction
    /// failed and which succeeded.
    ///
    /// Returns a vector of (signature, result) where result is None if the transaction is was not confirmed.
    pub async fn send_retry_and_confirm_transactions(
        &self,
        transactions: &[VersionedTransaction],
    ) -> Result<Vec<(Signature, Option<TransactionResult>)>> {
        let signatures = join_all(transactions.iter().map(|tx| self.send_transaction(tx)))
            .await
            .into_iter()
            .collect::<Result<Vec<Signature>>>()?;
        let mut tx_to_confirm: Vec<(Signature, Option<TransactionResult>)> = signatures
            .into_iter()
            .zip(std::iter::repeat(None))
            .collect();

        // Step 1: confirm processed and retry all that are not at least processed
        {
            // Use a copy as we don't want to modify the original vector
            let mut tx_to_confirm = tx_to_confirm.clone();

            self.confirm_transactions(
                &mut tx_to_confirm,
                CommitmentConfig::processed(),
                DEFAULT_NUM_FETCH_TO_RETRY,
            )
            .await?;

            // Resend all transactions that are not confirmed at processed level yet.
            let txs_to_retry = tx_to_confirm
                .iter()
                .zip(transactions)
                // Keep not confirmed transactions
                .filter(|((_, result), _)| result.is_none())
                .map(|(_, tx)| tx);

            // Note: signatures cannot change here as we keep the same blockhash as previously.
            // We can ignore the result safely.
            let _ = join_all(txs_to_retry.map(|tx| self.send_transaction(tx))).await;
        }

        // Step 2: confirm all transactions at the configured commitment level
        self.confirm_transactions(
            &mut tx_to_confirm,
            self.commitment_config,
            commitment_to_retry_count(self.commitment_config) - DEFAULT_NUM_FETCH_TO_RETRY,
        )
        .await?;

        Ok(tx_to_confirm)
    }

    pub async fn send_and_confirm_transaction(
        &self,
        transaction: VersionedTransaction,
    ) -> Result<(Signature, Option<TransactionResult>)> {
        let res = self.send_and_confirm_transactions(&[transaction]).await?;
        Ok(res
            .into_iter()
            .next()
            .expect("Sent and confirm one transaction, expect one result"))
    }

    pub async fn send_retry_and_confirm_transaction(
        &self,
        transaction: VersionedTransaction,
    ) -> Result<(Signature, Option<TransactionResult>)> {
        let res = self
            .send_retry_and_confirm_transactions(&[transaction])
            .await?;
        Ok(res
            .into_iter()
            .next()
            .expect("Sent and confirm one transaction, expect one result"))
    }

    // internal tools
    fn get_remaining_signatures_to_confirm(
        tx_to_confirm: &mut [(Signature, Option<TransactionResult>)],
    ) -> (
        Vec<Signature>,
        Vec<&mut (Signature, Option<TransactionResult>)>,
    ) {
        let remaining_to_confirm: Vec<_> = tx_to_confirm
            .iter_mut()
            .filter(|(_, result)| result.is_none())
            .collect();
        let remaining_signatures: Vec<_> =
            remaining_to_confirm.iter().map(|(sig, _)| *sig).collect();
        (remaining_signatures, remaining_to_confirm)
    }

    async fn confirm_transactions(
        &self,
        tx_to_confirm: &mut [(Signature, Option<TransactionResult>)],
        confirmation_level: CommitmentConfig,
        nb_attempts: usize,
    ) -> Result<()> {
        for _retry in 0..nb_attempts {
            let (remaining_signatures, mut remaining_tx_to_confirm) =
                Self::get_remaining_signatures_to_confirm(tx_to_confirm);
            if remaining_signatures.is_empty() {
                return Ok(());
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(
                    DEFAULT_STATUS_FETCH_DELAY_MS,
                ))
                .await;
            }
            let statuses = self
                .client
                .get_signature_statuses(&remaining_signatures)
                .await?;
            for (to_set, status) in remaining_tx_to_confirm
                .iter_mut()
                .zip(statuses)
                .filter_map(|((_sig, to_set), status)| status.map(|s| (to_set, s)))
            {
                if let Some(err) = status.err {
                    *to_set = Some(Err(err));
                } else if status.satisfies_commitment(confirmation_level) {
                    *to_set = Some(Ok(()));
                }
            }
        }
        Ok(())
    }
}
