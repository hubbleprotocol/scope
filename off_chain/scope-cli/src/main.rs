use anchor_client::solana_sdk::signature::read_keypair_file;
use anchor_client::{solana_sdk::pubkey::Pubkey, Client, Cluster};

use anchor_client::solana_sdk::clock;
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use scope_client::utils::get_clock;
use scope_client::{ScopeClient, ScopeConfig};
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};

use tracing::{error, info, trace, warn};

use anyhow::Result;

mod cluster_parse;
mod web;

use cluster_parse::parse;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Connect to solana validator
    #[clap(long, env, parse(try_from_str=parse), default_value = "localnet")]
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

    /// Set flag to activate json log output
    #[clap(long, env = "JSON_LOGS")]
    json: bool,

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
    #[clap()]
    Init {
        /// Where is stored the mapping to use
        #[clap(long, env, parse(from_os_str))]
        mapping: Option<PathBuf>,
    },

    /// Display the all prices from the oracle
    #[clap()]
    Show {
        /// Optional configuration file to provide association between
        /// entries number and a price name.
        /// If provided only the prices listed in configuration file are displayed
        #[clap(long, env, parse(from_os_str))]
        mapping: Option<PathBuf>,
    },

    /// Automatically refresh the prices
    #[clap()]
    Crank {
        /// Age of price in slot before triggering a refresh
        #[clap(long, env, default_value = "30")]
        refresh_interval_slot: clock::Slot,
        /// Where to store the mapping
        #[clap(long, env, parse(from_os_str))]
        mapping: Option<PathBuf>,
        /// Activate the health webserver for Kubernetes
        #[clap(long, env)]
        server: bool,
        /// Embedded webserver port
        /// Only valid if --server is also used
        #[clap(long, env, default_value = "8080")]
        server_port: u16,
        /// Number of tx retries before logging an error for prices being too old
        #[clap(long, env, default_value = "10")]
        num_retries_before_error: usize,
        /// Log old prices as errors when prices are still too old after all retries
        #[clap(long, env)]
        old_price_is_error: bool,
    },

    /// Get a list of all pubkeys that are needed for price refreshed according to the configuration.
    /// This includes the extra pubkeys that are not directly referenced by the configuration.
    #[clap()]
    GetPubkeys {
        /// Where is stored the mapping to use
        /// This must be provided to get entries that are not yet in the onchain oracle mapping.
        #[clap(long, env, parse(from_os_str))]
        mapping: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    if args.json {
        tracing_subscriber::fmt().json().init();
    } else {
        tracing_subscriber::fmt::init();
    }
    // Read keypair to sign transactions
    let payer = read_keypair_file(args.keypair).expect("Keypair file not found or invalid");

    let commitment = if let Actions::Crank { .. } = args.action {
        // For crank we don't want to wait for proper confirmation of the refresh transaction
        CommitmentConfig::confirmed()
    } else {
        CommitmentConfig::finalized()
    };

    let client = Client::new_with_options(args.cluster, Rc::new(payer), commitment);

    if let Actions::Init { mapping } = args.action {
        init(&client, &args.program_id, &args.price_feed, &mapping)
    } else {
        let mut scope = ScopeClient::new(client, args.program_id, &args.price_feed)?;

        match args.action {
            Actions::Download { mapping } => download(&mut scope, &mapping),
            Actions::Upload { mapping } => upload(&mut scope, &mapping),
            Actions::Init { .. } => unreachable!(),
            Actions::Show { mapping } => show(&mut scope, &mapping),
            Actions::Crank {
                refresh_interval_slot,
                mapping,
                server,
                server_port,
                num_retries_before_error,
                old_price_is_error,
            } => {
                if server {
                    web::server::thread_start(server_port)?;
                }
                crank(
                    &mut scope,
                    (mapping).as_ref(),
                    refresh_interval_slot,
                    num_retries_before_error,
                    old_price_is_error,
                )
            }
            Actions::GetPubkeys { mapping } => get_pubkeys(&mut scope, &mapping),
        }
    }
}

fn init(
    client: &Client,
    program_id: &Pubkey,
    price_feed: &str,
    mapping_op: &Option<impl AsRef<Path>>,
) -> Result<()> {
    let mut scope = ScopeClient::new_init_program(client, program_id, price_feed)?;

    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list)?;
        scope.upload_oracle_mapping()?;
    }

    Ok(())
}

fn upload(scope: &mut ScopeClient, mapping: &impl AsRef<Path>) -> Result<()> {
    let token_list = ScopeConfig::read_from_file(&mapping)?;
    scope.set_local_mapping(&token_list)?;
    scope.upload_oracle_mapping()
}

fn download(scope: &mut ScopeClient, mapping: &impl AsRef<Path>) -> Result<()> {
    scope.download_oracle_mapping(0)?;
    let token_list = scope.get_local_mapping()?;
    token_list.save_to_file(mapping)
}

fn show(scope: &mut ScopeClient, mapping_op: &Option<impl AsRef<Path>>) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list)?;
    } else {
        scope.download_oracle_mapping(0)?;
    }

    let current_slot = get_clock(&scope.get_rpc())?.slot;

    info!(current_slot);

    scope.log_prices(current_slot)
}

fn get_pubkeys(scope: &mut ScopeClient, mapping_op: &Option<impl AsRef<Path>>) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list)?;
    } else {
        scope.download_oracle_mapping(0)?;
    }

    scope.print_pubkeys()
}

fn crank(
    scope: &mut ScopeClient,
    mapping_op: Option<impl AsRef<Path>>,
    refresh_interval_slot: clock::Slot,
    init_num_retries_before_err: usize,
    old_price_is_error: bool,
) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        info!(
            "Default refresh interval set to {:?} slots",
            token_list.default_max_age
        );
        scope.set_local_mapping(&token_list)?;
        // TODO add check if local is correctly equal to remote mapping
    } else {
        info!(
            "Default refresh interval set to {:?} slots",
            refresh_interval_slot
        );
        scope.download_oracle_mapping(refresh_interval_slot)?;
    }
    let mut num_retries_before_err: usize = init_num_retries_before_err;
    let error_log = format!(
        "Some prices are still too old after {init_num_retries_before_err} refresh attempts"
    );
    loop {
        let start = Instant::now();

        if let Err(e) = scope.refresh_expired_prices() {
            warn!("Error while refreshing prices {:?}", e);
        }

        let elapsed = start.elapsed();
        trace!("last refresh duration was {:?}", elapsed);

        let current_slot = get_clock(&scope.get_rpc())?.slot;

        info!(current_slot);

        let _ = scope.log_prices(current_slot);

        let shortest_ttl = scope.get_prices_shortest_ttl()?;
        trace!(shortest_ttl);

        if shortest_ttl > 0 {
            num_retries_before_err = init_num_retries_before_err;
            // Time to sleep if we consider slot age
            let sleep_ms_from_slots = shortest_ttl * clock::DEFAULT_MS_PER_SLOT;
            trace!(sleep_ms_from_slots);
            sleep(Duration::from_millis(sleep_ms_from_slots));
        } else {
            num_retries_before_err -= 1;
        }

        if num_retries_before_err == 0 {
            if old_price_is_error {
                error!(%error_log, old_prices=?scope.get_expired_prices().unwrap_or_default());
            } else {
                warn!(%error_log, old_prices=?scope.get_expired_prices().unwrap_or_default());
            }
            num_retries_before_err = init_num_retries_before_err;
        }
    }

    #[allow(unreachable_code)]
    {
        // no exit condition in crank operating mode
        unreachable!()
    }
}
