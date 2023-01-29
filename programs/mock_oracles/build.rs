use std::{env, fs, path::PathBuf, str::FromStr};

use anyhow::Result;

const PUBKEY_RS_FILENAME: &str = "pubkey.rs";

// This build file generate the public key to know the program id
fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR").unwrap();
    let cluster = env::var("CLUSTER").unwrap_or_else(|_| "localnet".to_string());
    let keypair_json_filename = format!("{}.json", env::var("CARGO_PKG_NAME").unwrap());
    let keypair_path = PathBuf::from_str("../../keys")?
        .join(cluster)
        .join(keypair_json_filename);

    let pubkey_path = PathBuf::from_str(&out_dir)?.join(PUBKEY_RS_FILENAME);

    // Rerun if CLUSTER is changed
    println!("cargo:rerun-if-env-changed=CLUSTER");
    // Rerun if private key change
    println!(
        "cargo:rerun-if-changed={}",
        keypair_path.as_os_str().to_string_lossy()
    );

    let keypair_json = fs::read(keypair_path)?;

    let keypair: Vec<u8> = serde_json::from_slice(&keypair_json)?;
    let pubkey = &keypair[32..64];

    let pubkey_json = serde_json::to_string(pubkey)?;

    fs::write(pubkey_path, pubkey_json)?;

    Ok(())
}
