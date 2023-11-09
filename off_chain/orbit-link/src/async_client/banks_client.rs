use async_trait::async_trait;
use solana_banks_client::{BanksClient, TransactionStatus as BankTransactionStatus};
use solana_banks_interface::{
    TransactionConfirmationStatus as BankTransactionConfirmationStatus,
    TransactionSimulationDetails,
};
use solana_transaction_status::{
    TransactionConfirmationStatus, UiReturnDataEncoding, UiTransactionReturnData,
};
use tokio::sync::Mutex;

use super::*;
use crate::Result;

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
    async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<RpcSimulateTransactionResult> {
        let mut bank = self.lock().await;

        bank.simulate_transaction(transaction.clone())
            .await
            .map(|response| {
                let err = response.result.and_then(|r| match r {
                    Ok(_) => None,
                    Err(e) => Some(e),
                });
                let (logs, units_consumed, return_data) = response
                    .simulation_details
                    .map(|v| {
                        let TransactionSimulationDetails {
                            logs,
                            units_consumed,
                            return_data,
                        } = v;
                        let return_data = return_data.map(|v| {
                            let data_str = BS64.encode(&v.data);
                            UiTransactionReturnData {
                                program_id: v.program_id.to_string(),
                                data: (data_str, UiReturnDataEncoding::Base64),
                            }
                        });
                        (Some(logs), Some(units_consumed), return_data)
                    })
                    .unwrap_or_default();
                RpcSimulateTransactionResult {
                    err,
                    logs,
                    accounts: None,
                    units_consumed,
                    return_data,
                }
            })
            .map_err(Into::into)
    }

    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature> {
        let mut bank = self.lock().await;

        bank.send_transaction(transaction.clone()).await?;
        Ok(transaction.signatures[0])
    }

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        let mut bank = self.lock().await;
        let rent = bank.get_rent().await.unwrap();
        Ok(rent.minimum_balance(data_len))
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>> {
        let mut bank = self.lock().await;
        // Note: There is no point to join all with bank client as it requires to be mutable
        // so takes the lock force sequential execution
        let mut statuses = Vec::with_capacity(signatures.len());
        for signature in signatures {
            let status = bank.get_transaction_status(*signature).await?;
            statuses.push(status.map(bank_status_to_transaction_status));
        }
        Ok(statuses)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash> {
        let mut bank = self.lock().await;
        bank.get_latest_blockhash().await.map_err(Into::into)
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        let mut bank = self.lock().await;
        bank.get_balance(*pubkey).await.map_err(Into::into)
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        let mut bank = self.lock().await;
        bank.get_account(*pubkey)
            .await?
            .ok_or_else(|| solana_banks_client::BanksClientError::ClientError("Account not found"))
            .map_err(Into::into)
    }

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        let mut bank = self.lock().await;
        // Note: There is no point tot join all with bank client as it requires to be mutable
        // so takes the lock force sequencial execution
        let mut accounts = Vec::with_capacity(pubkeys.len());
        for pubkey in pubkeys {
            accounts.push(bank.get_account(*pubkey).await?);
        }
        Ok(accounts)
    }

    async fn get_slot_with_commitment(&self, _commitment: CommitmentConfig) -> Result<Slot> {
        let mut bank = self.lock().await;
        bank.get_root_slot().await.map_err(Into::into)
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64> {
        Ok(0)
    }
}
