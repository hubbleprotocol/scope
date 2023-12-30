use async_trait::async_trait;
use solana_account_decoder::UiAccountEncoding;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use tracing::{debug, trace};

use super::*;
use crate::Result;

#[async_trait]
impl AsyncClient for RpcClient {
    async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<RpcSimulateTransactionResult> {
        <RpcClient>::simulate_transaction(self, transaction)
            .await
            .map_err(Into::into)
            .map(|response| response.value)
    }

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

    async fn get_program_accounts_with_size_and_discriminator(
        &self,
        program_id: &Pubkey,
        size: u64,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>> {
        let memcmp = RpcFilterType::Memcmp(Memcmp::new(
            0,
            MemcmpEncodedBytes::Bytes(discriminator.to_vec()),
        ));
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::DataSize(size), memcmp]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };

        <RpcClient>::get_program_accounts_with_config(self, program_id, config)
            .await
            .map_err(Into::into)
    }

    async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<Slot> {
        <RpcClient>::get_slot_with_commitment(self, commitment)
            .await
            .map_err(Into::into)
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64> {
        let fees = self.get_recent_prioritization_fees(&[]).await?;
        trace!("Recent fees: {:#?}", fees);
        let fee = fees
            .into_iter()
            .fold(0, |acc, x| u64::max(acc, x.prioritization_fee));

        debug!("Selected fee: {}", fee);

        Ok(fee)
    }
}
