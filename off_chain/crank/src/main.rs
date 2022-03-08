use anchor_client::solana_sdk::signature::read_keypair_file;
use anchor_client::{solana_sdk::pubkey::Pubkey, Client, Cluster};

use scope_client::{ScopeClient, TokenConfList};
use solana_sdk::commitment_config::CommitmentConfig;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};

use tracing::{error, info, warn};

use anyhow::Result;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Connect to solana validator
    #[clap(long, env, parse(try_from_str), default_value = "localnet")]
    cluster: Cluster,

    /// Account keypair to pay for the transactions
    #[clap(long, env, parse(from_os_str))]
    keypair: PathBuf,

    /// Program Id
    #[clap(long, env, parse(try_from_str))]
    program_id: Pubkey,

    /// "Price feed" unique name to work with
    #[clap(long, env)]
    price_feed: String,

    /// Subcommand to execute
    #[clap(subcommand)]
    action: Actions,
}

#[derive(Debug, Subcommand)]
enum Actions {
    /// Download the remote oracle mapping in the provided mapping file
    #[clap(arg_required_else_help = true)]
    Download {
        /// Where to store the mapping
        #[clap(long, env, parse(from_os_str))]
        mapping: PathBuf,
    },
    /// Upload the provided oracle mapping to the chain.
    /// This requires initial program deploy account
    #[clap(arg_required_else_help = true)]
    Upload {
        /// Where is stored the mapping to upload
        #[clap(long, env, parse(from_os_str))]
        mapping: PathBuf,
    },
    /// Initialize the program accounts
    /// This requires initial program deploy account and enough funds
    #[clap(arg_required_else_help = true)]
    Init {
        /// Where is stored the mapping to use
        #[clap(long, env, parse(from_os_str))]
        mapping: PathBuf,
    },

    /// Automatically refresh the prices
    #[clap()]
    Crank {
        #[clap(long, env, default_value = "1000")]
        refresh_interval_ms: u64,
        /// Where to store the mapping
        #[clap(long, env, parse(from_os_str))]
        mapping: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    // TODO output JSON?
    tracing_subscriber::fmt::init();

    // Read keypair to sign transactions
    let payer = read_keypair_file(args.keypair).expect("Keypair file not found or invalid");

    let commitment = if let Actions::Crank { .. } = args.action {
        // For crank we don't want to wait for proper confirmation of the refresh transaction
        CommitmentConfig::processed()
    } else {
        CommitmentConfig::confirmed()
    };

    let client = Client::new_with_options(args.cluster, Rc::new(payer), commitment);

    let mut scope = ScopeClient::new(client, args.program_id, args.price_feed)?;

    match args.action {
        Actions::Download { mapping } => download(&mut scope, &mapping),
        Actions::Upload { mapping } => upload(&mut scope, &mapping),
        Actions::Init { mapping } => init(&mut scope, &mapping),
        Actions::Crank {
            refresh_interval_ms,
            mapping,
        } => crank(&mut scope, (&mapping).as_ref(), refresh_interval_ms),
    }
}

fn init(scope: &mut ScopeClient, mapping: &impl AsRef<Path>) -> Result<()> {
    let token_list = TokenConfList::read_from_file(&mapping)?;
    scope.set_local_mapping(&token_list)?;
    scope.init_program()
}

fn upload(scope: &mut ScopeClient, mapping: &impl AsRef<Path>) -> Result<()> {
    let token_list = TokenConfList::read_from_file(&mapping)?;
    scope.set_local_mapping(&token_list)?;
    scope.upload_oracle_mapping()
}

fn download(scope: &mut ScopeClient, mapping: &impl AsRef<Path>) -> Result<()> {
    scope.download_oracle_mapping()?;
    let token_list = scope.get_local_mapping()?;
    token_list.save_to_file(&mapping)
}

fn crank(
    scope: &mut ScopeClient,
    mapping_op: Option<impl AsRef<Path>>,
    refresh_interval_ms: u64,
) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = TokenConfList::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list)?;
        // TODO add check if local is correctly equal to remote mapping
    } else {
        scope.download_oracle_mapping()?;
    }
    let refresh_interval = Duration::from_millis(refresh_interval_ms);
    info!("Refresh interval set to {:?}", refresh_interval);
    loop {
        let start = Instant::now();
        if let Err(e) = scope.refresh_all_prices() {
            error!("Error while refreshing prices {:?}", e);
        }
        let elapsed = start.elapsed();
        if refresh_interval > elapsed {
            sleep(refresh_interval - elapsed);
        } else {
            warn!("last refresh duration was {:?}", elapsed)
        }
    }

    #[allow(unreachable_code)]
    {
        // no exit condition in crank operating mode
        unreachable!()
    }
}
