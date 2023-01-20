use async_trait::async_trait;
use solana_banks_client::BanksClient;
use solana_banks_client::TransactionStatus as BankTransactionStatus;
use solana_banks_interface::TransactionConfirmationStatus as BankTransactionConfirmationStatus;
use solana_transaction_status::TransactionConfirmationStatus;
use tokio::sync::Mutex;

use super::*;

fn bank_status_to_transaction_status(bank_status: BankTransactionStatus) -> TransactionStatus {
    let BankTransactionStatus {
        slot,
        confirmations,
        err,
        confirmation_status,
    } = bank_status;
    let status = match err.clone() {
        Some(err) => Err(err),
        None => Ok(()),
    };
    let confirmation_status = confirmation_status.map(|status| match status {
        BankTransactionConfirmationStatus::Processed => TransactionConfirmationStatus::Processed,
        BankTransactionConfirmationStatus::Confirmed => TransactionConfirmationStatus::Confirmed,
        BankTransactionConfirmationStatus::Finalized => TransactionConfirmationStatus::Finalized,
    });
    TransactionStatus {
        slot,
        confirmations,
        status,
        err,
        confirmation_status,
    }
}

#[async_trait]
impl AsyncClient for Mutex<BanksClient> {
    type Error = solana_banks_client::BanksClientError;

    async fn send_transaction(
        &self,
        _transaction: &VersionedTransaction,
    ) -> Result<Signature, Self::Error> {
        unimplemented!(
            "Versioned transactions are not supported by BanksClient yet (wait for solana 1.15.0)"
        )
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>, Self::Error> {
        let mut bank = self.lock().await;
        // Note: There is no point tot join all with bank client as it requires to be mutable
        // so takes the lock force sequencial execution
        let mut statuses = Vec::with_capacity(signatures.len());
        for signature in signatures {
            let status = bank.get_transaction_status(*signature).await?;
            statuses.push(status.map(bank_status_to_transaction_status));
        }
        Ok(statuses)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Self::Error> {
        let mut bank = self.lock().await;
        bank.get_latest_blockhash().await
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64, Self::Error> {
        let mut bank = self.lock().await;
        bank.get_balance(*pubkey).await
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account, Self::Error> {
        let mut bank = self.lock().await;
        bank.get_account(*pubkey)
            .await?
            .ok_or_else(|| solana_banks_client::BanksClientError::ClientError("Account not found"))
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> Result<Vec<Option<Account>>, Self::Error> {
        let mut bank = self.lock().await;
        // Note: There is no point tot join all with bank client as it requires to be mutable
        // so takes the lock force sequencial execution
        let mut accounts = Vec::with_capacity(pubkeys.len());
        for pubkey in pubkeys {
            accounts.push(bank.get_account(*pubkey).await?);
        }
        Ok(accounts)
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64, Self::Error> {
        Ok(0)
    }
}
