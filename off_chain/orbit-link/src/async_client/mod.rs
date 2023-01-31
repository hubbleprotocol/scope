#[cfg(feature = "banks-client")]
pub mod banks_client;
#[cfg(feature = "rpc-client")]
pub mod rpc_client;

use anchor_client::solana_sdk::{
    account::Account, hash::Hash, pubkey::Pubkey, signature::Signature,
    transaction::VersionedTransaction,
};
use async_trait::async_trait;
use solana_transaction_status::TransactionStatus;

use crate::Result;

#[async_trait]
pub trait AsyncClient: Sync {
    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature>;

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>>;

    async fn get_latest_blockhash(&self) -> Result<Hash>;

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64>;

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64>;

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account>;

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>>;

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64>;
}
