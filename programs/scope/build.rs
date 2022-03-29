use std::env;

// This build file generate the public key to know the program id
fn main() {
    if cfg!(any(
        feature = "localnet",
        feature = "devnet",
        feature = "mainnet"
    )) {
        // A cluster feature has been manually set, just ignore env variables.
    } else {
        let cluster = env::var("CLUSTER").unwrap_or_else(|_| "mainnet".to_string());

        // Rerun if CLUSTER is changed
        println!("cargo:rerun-if-env-changed=CLUSTER");
        // Set feature according to current cluster
        if matches!(cluster.as_str(), "localnet" | "devnet") {
            println!("cargo:rustc-cfg=feature=\"{}\"", cluster);
        } else {
            // default to mainnet configuration
            println!("cargo:rustc-cfg=feature=\"mainnet\"");
        }
    }
}
