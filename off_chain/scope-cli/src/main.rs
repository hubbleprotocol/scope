use std::{
    ops::Neg,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anchor_client::{
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        clock, commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
        signer::Signer,
    },
    Cluster,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use orbit_link::{async_client::AsyncClient, OrbitLink};
use scope_client::{utils::get_clock, ScopeClient, ScopeConfig};
use tokio::time::sleep;
use tracing::{error, info, trace, warn};

mod web;

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

    /// Set flag to activate json log output
    #[clap(long, env = "JSON_LOGS")]
    json: bool,

    /// Print timestamps in logs (not needed on grafana)
    #[clap(long, env)]
    log_timestamps: bool,

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
        /// Period in seconds to print all prices
        #[clap(long, env, default_value = "60")]
        print_period_s: u64,
        /// Time in seconds to wait before repeating alert if the price is still too old
        #[clap(long, env, default_value = "30")]
        old_price_alert_snooze_time_s: u64,
        /// Number of slots above max age before alerting for a price being too old
        #[clap(long, env, default_value = "50")]
        alert_old_price_after_slots: clock::Slot,
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

    /// Resets the value of a TWAP to the current price for that asset.
    /// This requires initial program deploy account
    #[clap()]
    ResetTwap {
        /// The index of the token for which we want to reset the TWAP
        #[clap(long, env)]
        token: u16,
    },

    /// Sets the admin cached for the config
    /// This requires admin keypair
    #[clap()]
    SetAdminCached {
        /// The pubkey of the admin_cached
        #[clap(long, env, parse(try_from_str))]
        admin_cached: Pubkey,
        /// If set returns a base58 encoded string instead of executing tx signing with provided keypair
        #[clap(long, env)]
        multisig: bool,
        /// Wether it should return tx in base64 - useful for simulations
        #[clap(long, env)]
        base64: bool,
    },

    /// Approves the admin cached for the config
    /// This requires adminc_cached keypair
    #[clap()]
    ApproveAdminCached {
        /// The pubkey of the admin_cached - to become admin
        #[clap(long, env, parse(try_from_str))]
        admin_cached: Pubkey,
        /// If set returns a base58 encoded string instead of executing tx signing with provided keypair
        #[clap(long, env)]
        multisig: bool,
        /// Wether it should return tx in base64 - useful for simulations
        #[clap(long, env)]
        base64: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = Args::parse();

    // Skip logging if only printing pubkeys
    if !matches!(args.action, Actions::GetPubkeys { .. }) {
        if args.json {
            tracing_subscriber::fmt().json().without_time().init();
        } else if args.log_timestamps {
            tracing_subscriber::fmt().compact().init();
        } else {
            tracing_subscriber::fmt().compact().without_time().init();
        }

        info!("Starting with args {:#?}", args);
    }

    // Read keypair to sign transactions
    let payer = read_keypair_file(args.keypair).expect("Keypair file not found or invalid");

    let commitment = if let Actions::Crank { .. } = args.action {
        // For crank we don't want to wait for proper confirmation of the refresh transaction
        CommitmentConfig::processed()
    } else {
        CommitmentConfig::confirmed()
    };

    let rpc_client = RpcClient::new_with_commitment(args.cluster.url().to_string(), commitment);
    // TODO: use lookup tables
    let client = OrbitLink::new(rpc_client, payer, None, commitment);

    if let Actions::Init { mapping } = args.action {
        init(client, &args.program_id, &args.price_feed, &mapping).await
    } else {
        let mut scope = ScopeClient::new(client, args.program_id, &args.price_feed).await?;

        match args.action {
            Actions::Download { mapping } => download(&mut scope, &mapping).await,
            Actions::Upload { mapping } => upload(&mut scope, &mapping).await,
            Actions::Init { .. } => unreachable!(),
            Actions::Show { mapping } => show(&mut scope, &mapping).await,
            Actions::Crank {
                refresh_interval_slot,
                mapping,
                server,
                server_port,
                print_period_s,
                old_price_alert_snooze_time_s,
                alert_old_price_after_slots,
                old_price_is_error,
            } => {
                let _server_handle = if server {
                    Some(web::server::thread_start(server_port).await)
                } else {
                    None
                };
                crank(
                    &mut scope,
                    (mapping).as_ref(),
                    refresh_interval_slot,
                    print_period_s,
                    old_price_alert_snooze_time_s,
                    alert_old_price_after_slots,
                    old_price_is_error,
                )
                .await
            }
            Actions::GetPubkeys { mapping } => get_pubkeys(&mut scope, &mapping).await,
            Actions::ResetTwap { token } => reset_twap(&scope, token).await,
            Actions::SetAdminCached {
                admin_cached,
                multisig,
                base64,
            } => set_admin_cached(&mut scope, admin_cached, multisig, base64).await,
            Actions::ApproveAdminCached {
                admin_cached,
                multisig,
                base64,
            } => approve_admin_cached(&mut scope, admin_cached, multisig, base64).await,
        }
    }
}

async fn init<T: AsyncClient, S: Signer>(
    client: OrbitLink<T, S>,
    program_id: &Pubkey,
    price_feed: &str,
    mapping_op: &Option<impl AsRef<Path>>,
) -> Result<()> {
    let mut scope = ScopeClient::new_init_program(client, program_id, price_feed).await?;

    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list).await?;
        scope.upload_oracle_mapping().await?;
    }

    Ok(())
}

async fn upload<T: AsyncClient, S: Signer>(
    scope: &mut ScopeClient<T, S>,
    mapping: &impl AsRef<Path>,
) -> Result<()> {
    let token_list = ScopeConfig::read_from_file(&mapping)?;
    scope.set_local_mapping(&token_list).await?;
    scope.upload_oracle_mapping().await
}

async fn download<T: AsyncClient, S: Signer>(
    scope: &mut ScopeClient<T, S>,
    mapping: &impl AsRef<Path>,
) -> Result<()> {
    scope.download_oracle_mapping(0).await?;
    let token_list = scope.get_local_mapping()?;
    token_list.save_to_file(mapping)
}

async fn show<T: AsyncClient, S: Signer>(
    scope: &mut ScopeClient<T, S>,
    mapping_op: &Option<impl AsRef<Path>>,
) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list).await?;
    } else {
        scope.download_oracle_mapping(0).await?;
    }

    let current_slot = get_clock(scope.get_rpc()).await?.slot;

    info!(current_slot);

    scope.log_prices(current_slot).await
}

async fn get_pubkeys<T: AsyncClient, S: Signer>(
    scope: &mut ScopeClient<T, S>,
    mapping_op: &Option<impl AsRef<Path>>,
) -> Result<()> {
    if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        scope.set_local_mapping(&token_list).await?;
    } else {
        scope.download_oracle_mapping(0).await?;
    }

    scope.print_pubkeys().await
}

async fn crank<T: AsyncClient, S: Signer>(
    scope: &mut ScopeClient<T, S>,
    mapping_op: Option<impl AsRef<Path>>,
    refresh_interval_slot: clock::Slot,
    print_period_s: u64,
    old_price_alert_snooze_time_s: u64,
    alert_old_price_after_slots: clock::Slot,
    old_price_is_error: bool,
) -> Result<()> {
    let auto_refresh_mapping = if let Some(mapping) = mapping_op {
        let token_list = ScopeConfig::read_from_file(&mapping)?;
        info!(
            "Default refresh interval set to {:?} slots",
            token_list.default_max_age
        );
        scope.set_local_mapping(&token_list).await?;

        false
    } else {
        info!(
            "Default refresh interval set to {:?} slots",
            refresh_interval_slot
        );
        scope.download_oracle_mapping(refresh_interval_slot).await?;

        true
    };

    let current_slot = get_clock(scope.get_rpc()).await.unwrap_or_default().slot;
    scope.log_prices(current_slot).await.unwrap();

    let alert_threshold: i64 = (alert_old_price_after_slots as i64).neg();
    let alert_snooze_time = Duration::from_secs(old_price_alert_snooze_time_s);
    let error_log = format!(
        "Some prices are older than max age by more than {alert_old_price_after_slots} slots."
    );
    let mut last_alert = Instant::now();

    let print_period = Duration::from_secs(print_period_s);
    let mut last_print = Instant::now();

    const MAPPING_REFRESH_PERIOD: Duration = Duration::from_secs(60);
    let mut last_mapping_refresh = Instant::now();

    loop {
        let start = Instant::now();

        if last_mapping_refresh.elapsed() > MAPPING_REFRESH_PERIOD {
            if auto_refresh_mapping {
                if let Err(res) = scope.download_oracle_mapping(refresh_interval_slot).await {
                    warn!("Error while refreshing mapping {:?}", res)
                }
            }
            last_mapping_refresh = Instant::now();
        }

        if last_print.elapsed() > print_period {
            let current_slot = get_clock(scope.get_rpc()).await.unwrap_or_default().slot;
            info!(current_slot);
            let _ = scope.log_prices(current_slot).await;
            last_print = Instant::now();
        }

        if let Err(e) = scope.refresh_old_prices().await {
            warn!("Error while refreshing prices {:?}", e);
        }

        let elapsed = start.elapsed();
        trace!("last refresh duration was {:?}", elapsed);

        let shortest_ttl = scope.get_prices_shortest_ttl().await.unwrap_or_default();
        trace!(shortest_ttl);

        if alert_threshold > shortest_ttl && last_alert.elapsed() > alert_snooze_time {
            last_alert = Instant::now();
            if old_price_is_error {
                error!(%error_log, old_prices=?scope.get_expired_prices().await.unwrap_or_default());
            } else {
                warn!(%error_log, old_prices=?scope.get_expired_prices().await.unwrap_or_default());
            }
        }

        let sleep_ms_from_slots = if shortest_ttl > 0 {
            // Time to sleep if we consider slot age
            (shortest_ttl as u64) * clock::DEFAULT_MS_PER_SLOT
        } else {
            // Avoid spamming the network with requests, sleep at least 1 slot
            clock::DEFAULT_MS_PER_SLOT
        };
        trace!(sleep_ms_from_slots);
        sleep(Duration::from_millis(sleep_ms_from_slots)).await;
    }
}

async fn reset_twap<T: AsyncClient, S: Signer>(
    scope: &ScopeClient<T, S>,
    token: u16,
) -> Result<()> {
    scope.reset_twap_price(token).await
}

async fn set_admin_cached<T: AsyncClient, S: Signer>(
    scope: &ScopeClient<T, S>,
    admin_cached: Pubkey,
    multisig: bool,
    base64: bool,
) -> Result<()> {
    scope
        .ix_set_admin_cached(admin_cached, multisig, base64)
        .await
}

async fn approve_admin_cached<T: AsyncClient, S: Signer>(
    scope: &ScopeClient<T, S>,
    admin_cached: Pubkey,
    multisig: bool,
    base64: bool,
) -> Result<()> {
    scope
        .ix_approve_admin_cached(admin_cached, multisig, base64)
        .await
}
