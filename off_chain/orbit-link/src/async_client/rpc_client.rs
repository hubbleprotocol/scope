use async_trait::async_trait;
use solana_client::nonblocking::rpc_client::RpcClient;

use super::*;
use crate::Result;

#[async_trait]
impl AsyncClient for RpcClient {
    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature> {
        <RpcClient>::send_transaction(self, transaction)
            .await
            .map_err(Into::into)
    }

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        <RpcClient>::get_minimum_balance_for_rent_exemption(self, data_len)
            .await
            .map_err(Into::into)
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>> {
        <RpcClient>::get_signature_statuses(self, signatures)
            .await
            .map(|response| response.value)
            .map_err(Into::into)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash> {
        <RpcClient>::get_latest_blockhash(self)
            .await
            .map_err(Into::into)
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        <RpcClient>::get_balance(self, pubkey)
            .await
            .map_err(Into::into)
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        <RpcClient>::get_account(self, pubkey)
            .await
            .map_err(Into::into)
    }

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        <RpcClient>::get_multiple_accounts(self, pubkeys)
            .await
            .map_err(Into::into)
    }

    async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<Slot> {
        <RpcClient>::get_slot_with_commitment(self, commitment)
            .await
            .map_err(Into::into)
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64> {
        // Fixed to 10 lamports per 200_000 CU (default 1 ix transaction) for now
        // 10 * 1M / 200_000 = 50
        Ok(50)
    }
}
