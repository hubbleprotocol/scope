use std::env;

// This build file generate the public key to know the program id
fn main() {
    let cluster = env::var("CLUSTER").unwrap_or_else(|_| "mainnet".to_string());

    // Rerun if CLUSTER is changed
    println!("cargo:rerun-if-env-changed=CLUSTER");
    // Set feature according to current cluster
    if matches!(cluster.as_str(), "localnet" | "devnet") {
        println!("cargo:rustc-cfg=feature=\"{}\"", cluster);
    } // default to mainnet configuration
}
