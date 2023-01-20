use anchor_client::solana_sdk::compute_budget::ComputeBudgetInstruction;
use anchor_client::solana_sdk::message::Message;
use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signer::Signer;
use anchor_client::solana_sdk::transaction::VersionedTransaction;
use anchor_client::{
    anchor_lang::{InstructionData, ToAccountMetas},
    solana_sdk::instruction::Instruction,
};

use crate::{errors, OrbitLink, Result};

use base64::engine::{general_purpose::STANDARD as BS64, Engine};

pub const DEFAULT_IX_BUDGET: u32 = 200_000;

#[derive(Clone)]
pub struct TxBuilder<'link, T, S>
where
    T: crate::async_client::AsyncClient,
    S: Signer,
{
    instructions: Vec<Instruction>,
    total_budget: u32,
    link: &'link OrbitLink<T, S>,
}

impl<'link, T, S> TxBuilder<'link, T, S>
where
    T: crate::async_client::AsyncClient,
    S: Signer,
    errors::ErrorKind: From<<T as crate::async_client::AsyncClient>::Error>,
{
    pub fn new(link: &'link OrbitLink<T, S>) -> Self {
        TxBuilder {
            instructions: vec![],
            total_budget: 0,
            link,
        }
    }

    pub fn add_ix_with_budget(mut self, instruction: Instruction, budget: u32) -> Self {
        self.instructions.push(instruction);
        self.total_budget += budget;
        self
    }

    pub fn add_ixs_with_budget(
        mut self,
        instructions: impl IntoIterator<Item = (Instruction, u32)>,
    ) -> Self {
        self.instructions
            .extend(instructions.into_iter().map(|(instruction, budget)| {
                self.total_budget += budget;
                instruction
            }));
        self
    }

    pub fn add_anchor_ix_with_budget(
        mut self,
        program_id: &Pubkey,
        accounts: impl ToAccountMetas,
        args: impl InstructionData,
        budget: u32,
    ) -> Self {
        self.instructions.push(Instruction {
            program_id: *program_id,
            data: args.data(),
            accounts: accounts.to_account_metas(None),
        });
        self.total_budget += budget;
        self
    }

    pub fn add_ix(self, instruction: Instruction) -> Self {
        self.add_ix_with_budget(instruction, DEFAULT_IX_BUDGET)
    }

    pub fn add_ixs(self, instructions: impl IntoIterator<Item = Instruction>) -> Self {
        let budgeted_instructions = instructions
            .into_iter()
            .map(|instruction| (instruction, DEFAULT_IX_BUDGET));
        self.add_ixs_with_budget(budgeted_instructions)
    }

    pub fn add_anchor_ix(
        self,
        program_id: &Pubkey,
        accounts: impl ToAccountMetas,
        args: impl InstructionData,
    ) -> Self {
        self.add_anchor_ix_with_budget(program_id, accounts, args, DEFAULT_IX_BUDGET)
    }

    pub async fn build(self, extra_signers: &[&dyn Signer]) -> Result<VersionedTransaction> {
        self.link.create_tx(&self.instructions, extra_signers).await
    }

    fn get_budget_ix(&self) -> Option<Instruction> {
        if self.total_budget > 200_000 || self.instructions.len() > 1 {
            Some(ComputeBudgetInstruction::set_compute_unit_limit(
                self.total_budget,
            ))
        } else {
            // No need for an extra compute budget instruction
            None
        }
    }

    pub async fn build_with_budget(
        self,
        extra_signers: &[&dyn Signer],
    ) -> Result<VersionedTransaction> {
        if self.instructions.is_empty() {
            return Err(errors::ErrorKind::NoInstructions);
        }
        let mut instructions = Vec::with_capacity(self.instructions.len() + 1);
        if let Some(ix_budget) = self.get_budget_ix() {
            instructions.push(ix_budget);
        }
        instructions.extend(self.instructions);
        self.link.create_tx(&instructions, extra_signers).await
    }

    pub async fn build_with_budget_and_fee(
        self,
        extra_signers: &[&dyn Signer],
    ) -> Result<VersionedTransaction> {
        if self.instructions.is_empty() {
            return Err(errors::ErrorKind::NoInstructions);
        }
        let mut instructions = Vec::with_capacity(self.instructions.len() + 2);
        if let Some(ix_budget) = self.get_budget_ix() {
            instructions.push(ix_budget);
        }
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
            self.link.client.get_recommended_micro_lamport_fee().await?,
        ));
        instructions.extend(self.instructions);
        self.link.create_tx(&instructions, extra_signers).await
    }

    pub fn build_raw_msg(&self) -> Vec<u8> {
        let msg = Message::new(&self.instructions, Some(&self.link.payer.pubkey()));
        msg.serialize()
    }

    pub fn to_base64(&self) -> String {
        let raw_msg = self.build_raw_msg();
        BS64.encode(raw_msg)
    }

    pub fn to_base58(&self) -> String {
        let raw_msg = self.build_raw_msg();
        bs58::encode(raw_msg).into_string()
    }
}
