use async_trait::async_trait;
use solana_client::nonblocking::rpc_client::RpcClient;

use super::*;

#[async_trait]
impl AsyncClient for RpcClient {
    type Error = solana_client::client_error::ClientError;

    async fn send_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Self::Error> {
        <RpcClient>::send_transaction(self, transaction).await
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>, Self::Error> {
        <RpcClient>::get_signature_statuses(self, signatures)
            .await
            .map(|response| response.value)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Self::Error> {
        <RpcClient>::get_latest_blockhash(self).await
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64, Self::Error> {
        <RpcClient>::get_balance(self, pubkey).await
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account, Self::Error> {
        <RpcClient>::get_account(self, pubkey).await
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> Result<Vec<Option<Account>>, Self::Error> {
        <RpcClient>::get_multiple_accounts(self, pubkeys).await
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64, Self::Error> {
        // Fixed to 10 lamports per 200_000 CU (default 1 ix transaction) for now
        // 10 * 1M / 200_000 = 50
        Ok(50)
    }
}
