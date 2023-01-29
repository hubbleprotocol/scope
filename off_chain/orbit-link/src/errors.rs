use anchor_client::solana_sdk::signer::SignerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[cfg(feature = "rpc-client")]
    #[error("Solana rpc client error: {0}")]
    SolanaRpcError(#[from] solana_client::client_error::ClientError),

    #[cfg(feature = "banks-client")]
    #[error("Solana banks client error: {0}")]
    SolanaBanksError(Box<solana_banks_client::BanksClientError>),

    #[error(transparent)]
    SignerError(#[from] SignerError),

    // TODO: replace with this once it's fixed: https://github.com/solana-labs/solana/issues/29858
    //#[error(transparent)]
    //TransactionCompileError(#[from] CompileError),
    #[error("Transaction compile error: {0}")]
    TransactionCompileError(String),

    #[error("No instruction to include in the transaction")]
    NoInstructions,
}

#[cfg(feature = "banks-client")]
impl From<solana_banks_client::BanksClientError> for ErrorKind {
    fn from(err: solana_banks_client::BanksClientError) -> Self {
        ErrorKind::SolanaBanksError(Box::new(err))
    }
}
