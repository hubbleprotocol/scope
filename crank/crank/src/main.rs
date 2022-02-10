use anyhow::Result;
use pyth_client::{cast, Price};
use serum_common::client::rpc::send_txn;
use serum_common::client::Cluster;
use solana_client::rpc_client::RpcClient;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::Signer;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::Keypair,
    transaction::Transaction,
};
use solana_shadow::{BlockchainShadow, SyncOptions, Pubkey};
use std::{str::FromStr, sync::Arc, time::Duration};
use tracing_subscriber::EnvFilter;

fn configure_logging() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_writer(std::io::stdout)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    configure_logging();

    // https://pyth.network/developers/accounts/
    let ethusd: Pubkey = "JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB".parse()?;
    let btcusd: Pubkey = "GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU".parse()?;
    let solusd: Pubkey = "H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG".parse()?;
    let rayusd: Pubkey = "AnLf8tVYCM816gmBjiy8n53eXKKEDydT5piYjjQDPgTB".parse()?;
    let fttusd: Pubkey = "8JPJJkmDScpcNmBRKGZuPuG2GYAveQgP3t5gFuMymwvF".parse()?;
    let srmusd: Pubkey = "3NBReDRTLKMQEKiLD5tGcx4kXbTf88b7f2xLS9UuGjym".parse()?;

    let shadow = BlockchainShadow::new_for_accounts(
        &vec![ethusd, btcusd, solusd, rayusd, fttusd, srmusd],
        SyncOptions::default(),
    )
    .await?;

    let crank = HubbleClient::new(Cluster::Localnet);

    // iterate over the offline shadow of the account
    // everytime any account is accessed, then its contents
    // will reflect the latest version on-chain.
    for i in 0.. {
        // access the most recent snapshot of an account
        let ethacc = shadow.get_account(&ethusd).unwrap();
        let eth_price = cast::<Price>(&ethacc.data).agg.price as u64;

        let btcacc = shadow.get_account(&btcusd).unwrap();
        let btc_price = cast::<Price>(&btcacc.data).agg.price as u64;

        let solacc = shadow.get_account(&solusd).unwrap();
        let sol_price = cast::<Price>(&solacc.data).agg.price as u64;

        let rayacc = shadow.get_account(&rayusd).unwrap();
        let ray_price = cast::<Price>(&rayacc.data).agg.price as u64;

        let fttacc = shadow.get_account(&fttusd).unwrap();
        let ftt_price = cast::<Price>(&fttacc.data).agg.price as u64;

        let srmacc = shadow.get_account(&srmusd).unwrap();
        let srm_price = cast::<Price>(&srmacc.data).agg.price as u64;

        crank.batch_update_oracle(
            sol_price, btc_price, eth_price, ray_price, ftt_price, srm_price,
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    shadow.worker().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use serum_common::client::{rpc::send_txn, Cluster};
    use solana_client::rpc_client::RpcClient;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::Transaction,
    };

    fn admin() -> Keypair {
        Keypair::from_bytes(&[
            241, 101, 13, 165, 53, 150, 114, 216, 162, 246, 157, 94, 156, 209, 145, 37, 186, 13,
            219, 120, 66, 196, 128, 253, 177, 46, 0, 70, 68, 211, 238, 83, 155, 17, 157, 105, 115,
            161, 0, 60, 146, 250, 19, 171, 63, 222, 211, 135, 37, 102, 222, 216, 142, 131, 67, 196,
            185, 182, 202, 219, 55, 24, 135, 90,
        ])
        .unwrap()
    }
    fn oracle() -> Keypair {
        Keypair::from_bytes(&[
            228, 145, 142, 148, 186, 81, 142, 99, 218, 213, 177, 170, 2, 88, 105, 73, 191, 249,
            131, 50, 136, 95, 195, 217, 106, 172, 49, 101, 67, 95, 184, 13, 211, 107, 228, 233,
            150, 139, 146, 59, 204, 32, 172, 1, 114, 196, 100, 148, 12, 59, 221, 40, 77, 201, 32,
            221, 178, 142, 27, 96, 239, 193, 170, 27,
        ])
        .unwrap()
    }

    #[test]
    fn crank_update_price() {
        let cluster = Cluster::Localnet;
        let client = RpcClient::new(cluster.url().to_string());
        let client = Arc::new(client);

        let admin = admin();
        let oracle = oracle();
        let clock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();

        let (recent_hash, _fee_calc) = client.get_recent_blockhash().unwrap();
        println!("Sending request ...");
        let mut account_metas = Vec::with_capacity(3);
        account_metas.push(AccountMeta::new(admin.pubkey(), true));
        account_metas.push(AccountMeta::new(oracle.pubkey(), false));
        account_metas.push(AccountMeta::new_readonly(clock, false));

        let mut instruction_data = vec![];
        let update_instruction_sighash = [219, 200, 88, 176, 158, 63, 253, 127];
        let update_instruction_token = borsh::to_vec(&(3 as u8)).unwrap();
        let update_instruction_price = borsh::to_vec(&(20 as u64)).unwrap();
        instruction_data.extend_from_slice(&update_instruction_sighash);
        instruction_data.extend_from_slice(&update_instruction_token);
        instruction_data.extend_from_slice(&update_instruction_price);

        let instruction = Instruction {
            program_id: Pubkey::from_str("6jnS9rvUGxu4TpwwuCeF12Ar9Cqk2vKbufqc6Hnharnz").unwrap(),
            accounts: account_metas,
            data: instruction_data,
        };

        let txn = Transaction::new_signed_with_payer(
            std::slice::from_ref(&instruction),
            Some(&admin.pubkey()),
            &[&admin],
            recent_hash,
        );

        let signature = send_txn(&client, &txn, false).unwrap();
        println!("Signature {}", signature);
    }

    #[test]
    fn crank_update_batch_price() {
        let cluster = Cluster::Localnet;
        let client = RpcClient::new(cluster.url().to_string());
        let client = Arc::new(client);

        let admin = admin();
        let oracle = oracle();
        let clock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();

        let (recent_hash, _fee_calc) = client.get_recent_blockhash().unwrap();
        println!("Sending request ...");
        let mut account_metas = Vec::with_capacity(3);
        account_metas.push(AccountMeta::new(admin.pubkey(), true));
        account_metas.push(AccountMeta::new(oracle.pubkey(), false));
        account_metas.push(AccountMeta::new_readonly(clock, false));

        let mut instruction_data = vec![];
        let sol_price: u64 = 21;
        let btc_price: u64 = 22;
        let eth_price: u64 = 23;
        let ray_price: u64 = 24;
        let ftt_price: u64 = 25;
        let srm_price: u64 = 26;

        let update_instruction_sighash = [57, 189, 226, 20, 239, 33, 98, 191];
        let update_instruction_sol_price = borsh::to_vec(&sol_price).unwrap();
        let update_instruction_btc_price = borsh::to_vec(&btc_price).unwrap();
        let update_instruction_eth_price = borsh::to_vec(&eth_price).unwrap();
        let update_instruction_ray_price = borsh::to_vec(&ray_price).unwrap();
        let update_instruction_ftt_price = borsh::to_vec(&ftt_price).unwrap();
        let update_instruction_srm_price = borsh::to_vec(&srm_price).unwrap();

        instruction_data.extend_from_slice(&update_instruction_sighash);
        instruction_data.extend_from_slice(&update_instruction_sol_price);
        instruction_data.extend_from_slice(&update_instruction_btc_price);
        instruction_data.extend_from_slice(&update_instruction_eth_price);
        instruction_data.extend_from_slice(&update_instruction_ray_price);
        instruction_data.extend_from_slice(&update_instruction_ftt_price);
        instruction_data.extend_from_slice(&update_instruction_srm_price);

        let instruction = Instruction {
            program_id: Pubkey::from_str("6jnS9rvUGxu4TpwwuCeF12Ar9Cqk2vKbufqc6Hnharnz").unwrap(),
            accounts: account_metas,
            data: instruction_data,
        };

        let txn = Transaction::new_signed_with_payer(
            std::slice::from_ref(&instruction),
            Some(&admin.pubkey()),
            &[&admin],
            recent_hash,
        );

        let signature = send_txn(&client, &txn, false).unwrap();
        println!("Signature {}", signature);
    }

    #[test]
    fn crank_deposit_collateral() {
        let cluster = Cluster::Localnet;
        let client = RpcClient::new(cluster.url().to_string());
        let client = Arc::new(client);

        let (recent_hash, _fee_calc) = client.get_recent_blockhash().unwrap();
        println!("Sending request ...");

        // #[account(mut, signer)]
        // pub owner: AccountInfo<'info>,
        // #[account(mut)]
        // pub borrowing_market_state: ProgramAccount<'info, BorrowingMarketState>,
        // pub borrowing_vaults: ProgramAccount<'info, BorrowingVaults>,
        // #[account(mut)]
        // pub user_metadata: ProgramAccount<'info, UserMetadata>,
        // #[account(mut)]
        // pub user_positions: Loader<'info, UserPositions>,
        // #[account(mut)]
        // pub collateral_from: AccountInfo<'info>,
        // #[account(mut)]
        // pub collateral_to: AccountInfo<'info>,
        // pub collateral_token_mint: AccountInfo<'info>,
        // pub token_program: AccountInfo<'info>,
        // pub system_program: AccountInfo<'info>,

        let owner = Keypair::from_base58_string("2adUJYFVwgdMGEjCB7w73QMcogJhufzVYMFqqXfuVCFA89M5i2sF8mdhRD8pj3DEoaAHbsMP29UGi4SvhLkfNGVw");
        println!("Owner {}", owner.pubkey());
        let borrowing_market_state =
            Pubkey::from_str("9CxdNQSSYAAYnnRP9dBwzGugtsRhZpv6jF4HM2iZEbgd").unwrap();
        let borrowing_vaults =
            Pubkey::from_str("9nz2rrzrpyMAewVNuwwxb9irFLBWqmjkyXXq8XGm9PBw").unwrap();
        let user_metadata =
            Pubkey::from_str("GPamWzpnwYwoqgmB5opXtuEvf3HqsuX4fLcKBtrEJai5").unwrap();
        let user_positions =
            Pubkey::from_str("2Sh1QzzkSW9scs5nQGSdDnvcytk7W4zapcEnMz3tsUcR").unwrap();
        let collateral_from = owner.pubkey().clone();
        let colalteral_to =
            Pubkey::from_str("6iqdgjWdKfTKwcRJqRyRAr3bUfnDzgeKGCRUVe7bbj7E").unwrap();
        let collateral_token_mint =
            Pubkey::from_str("9nz2rrzrpyMAewVNuwwxb9irFLBWqmjkyXXq8XGm9PBw").unwrap();
        let token_program =
            Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        let system_program = Pubkey::from_str("11111111111111111111111111111111").unwrap();

        let mut account_metas = vec![];
        account_metas.push(AccountMeta::new(owner.pubkey(), true));
        account_metas.push(AccountMeta::new(borrowing_market_state, false));
        account_metas.push(AccountMeta::new_readonly(borrowing_vaults, false));
        account_metas.push(AccountMeta::new_readonly(user_metadata, false));
        account_metas.push(AccountMeta::new(user_positions, false));
        account_metas.push(AccountMeta::new(collateral_from, false));
        account_metas.push(AccountMeta::new(colalteral_to, false));
        account_metas.push(AccountMeta::new_readonly(collateral_token_mint, false));
        account_metas.push(AccountMeta::new_readonly(token_program, false));
        account_metas.push(AccountMeta::new_readonly(system_program, false));

        let mut instruction_data = vec![];
        let update_instruction_sighash = [156, 131, 142, 116, 146, 247, 162, 120];
        let update_instruction_amount_in_lamports = borsh::to_vec(&(3 as u64)).unwrap();
        let update_instruction_collateral = borsh::to_vec(&(20 as u8)).unwrap();
        instruction_data.extend_from_slice(&update_instruction_sighash);
        instruction_data.extend_from_slice(&update_instruction_amount_in_lamports);
        instruction_data.extend_from_slice(&update_instruction_collateral);

        let instruction = Instruction {
            program_id: Pubkey::from_str("8v1DhJaewvhbhDmptNrkYig7YFcExsRKteR3cYjLw2iy").unwrap(),
            accounts: account_metas,
            data: instruction_data,
        };

        let txn = Transaction::new_signed_with_payer(
            std::slice::from_ref(&instruction),
            Some(&owner.pubkey()),
            &[&owner],
            recent_hash,
        );

        let signature = send_txn(&client, &txn, false).unwrap();
        println!("Signature {}", signature);
    }
}

// fn read_keypair_file(s: &str) -> Result<Keypair> {
//     solana_sdk::signature::read_keypair_file(s)
//         .map_err(|_| format_err!("failed to read keypair from {}", s))
// }

struct HubbleClient {
    admin: Keypair,
    account_metas: Vec<AccountMeta>,
    program_id: Pubkey,
    client: Arc<RpcClient>,
}

impl HubbleClient {
    fn admin() -> Keypair {
        Keypair::from_bytes(&[
            241, 101, 13, 165, 53, 150, 114, 216, 162, 246, 157, 94, 156, 209, 145, 37, 186, 13,
            219, 120, 66, 196, 128, 253, 177, 46, 0, 70, 68, 211, 238, 83, 155, 17, 157, 105, 115,
            161, 0, 60, 146, 250, 19, 171, 63, 222, 211, 135, 37, 102, 222, 216, 142, 131, 67, 196,
            185, 182, 202, 219, 55, 24, 135, 90,
        ])
        .unwrap()
    }
    fn oracle() -> Keypair {
        Keypair::from_bytes(&[
            228, 145, 142, 148, 186, 81, 142, 99, 218, 213, 177, 170, 2, 88, 105, 73, 191, 249,
            131, 50, 136, 95, 195, 217, 106, 172, 49, 101, 67, 95, 184, 13, 211, 107, 228, 233,
            150, 139, 146, 59, 204, 32, 172, 1, 114, 196, 100, 148, 12, 59, 221, 40, 77, 201, 32,
            221, 178, 142, 27, 96, 239, 193, 170, 27,
        ])
        .unwrap()
    }

    pub fn new(cluster: Cluster) -> Self {
        let admin = HubbleClient::admin();
        let oracle = HubbleClient::oracle();
        let clock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();
        let hubble_program =
            Pubkey::from_str("6jnS9rvUGxu4TpwwuCeF12Ar9Cqk2vKbufqc6Hnharnz").unwrap();
        Self {
            admin: HubbleClient::admin(),
            account_metas: vec![
                AccountMeta::new(admin.pubkey(), true),
                AccountMeta::new(oracle.pubkey(), false),
                AccountMeta::new_readonly(clock, false),
            ],
            program_id: hubble_program,
            client: Arc::new(RpcClient::new(cluster.url().to_string())),
        }
    }

    pub fn batch_update_oracle(
        &self,
        sol_price: u64,
        btc_price: u64,
        eth_price: u64,
        ray_price: u64,
        ftt_price: u64,
        srm_price: u64,
    ) {
        let update_instruction_sighash = [57, 189, 226, 20, 239, 33, 98, 191];
        let update_instruction_sol_price = borsh::to_vec(&sol_price).unwrap();
        let update_instruction_btc_price = borsh::to_vec(&btc_price).unwrap();
        let update_instruction_eth_price = borsh::to_vec(&eth_price).unwrap();
        let update_instruction_ray_price = borsh::to_vec(&ray_price).unwrap();
        let update_instruction_ftt_price = borsh::to_vec(&ftt_price).unwrap();
        let update_instruction_srm_price = borsh::to_vec(&srm_price).unwrap();

        let mut instruction_data = vec![];
        instruction_data.extend_from_slice(&update_instruction_sighash);
        instruction_data.extend_from_slice(&update_instruction_sol_price);
        instruction_data.extend_from_slice(&update_instruction_btc_price);
        instruction_data.extend_from_slice(&update_instruction_eth_price);
        instruction_data.extend_from_slice(&update_instruction_ray_price);
        instruction_data.extend_from_slice(&update_instruction_ftt_price);
        instruction_data.extend_from_slice(&update_instruction_srm_price);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: self.account_metas.clone(),
            data: instruction_data,
        };

        let (recent_hash, _fee_calc) = self.client.get_recent_blockhash().unwrap();
        let txn = Transaction::new_signed_with_payer(
            std::slice::from_ref(&instruction),
            Some(&self.admin.pubkey()),
            &[&self.admin],
            recent_hash,
        );

        let lamports_before = self
            .client
            .get_account(&self.admin.pubkey())
            .unwrap()
            .lamports;

        let signature = send_txn(&self.client, &txn, false).unwrap();

        let lamports_after = self
            .client
            .get_account(&self.admin.pubkey())
            .unwrap()
            .lamports;

        let sol_price_factor = 100_000_000.0;
        let sol_usd_price = sol_price as f64 / sol_price_factor;
        let lamports_usd_price = sol_usd_price / (LAMPORTS_PER_SOL as f64);
        let cost =
            (lamports_before.checked_sub(lamports_after).unwrap_or(0) as f64) * lamports_usd_price;

        println!(
            "[{:?}] Batch updated before={} after={} cost=${} SOL={} BTC={} ETH={} RAY={} FTT={} SRM={} sig={}..{}",
            chrono::offset::Utc::now(),
            lamports_before,
            lamports_after,
            cost,
            sol_price as f64 / sol_price_factor,
            btc_price as f64 / sol_price_factor,
            eth_price as f64 / sol_price_factor,
            ray_price as f64 / sol_price_factor,
            ftt_price as f64 / sol_price_factor,
            srm_price as f64 / sol_price_factor,
            &signature.to_string()[..5],
            &signature.to_string()[signature.to_string().len() - 5..]
        );
    }
}
