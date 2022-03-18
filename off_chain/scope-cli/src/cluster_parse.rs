use anchor_client::Cluster;
use anyhow::{anyhow, Result};
use url::Url;

// TODO: remove all this once https://github.com/project-serum/anchor/pull/1362 is released

pub fn parse(s: &str) -> Result<Cluster> {
    match s.to_lowercase().as_str() {
        "t" | "testnet" => Ok(Cluster::Testnet),
        "m" | "mainnet" => Ok(Cluster::Mainnet),
        "d" | "devnet" => Ok(Cluster::Devnet),
        "l" | "localnet" => Ok(Cluster::Localnet),
        "g" | "debug" => Ok(Cluster::Debug),
        _ if s.starts_with("http") => {
            let http_url = s;

            // Taken from:
            // https://github.com/solana-labs/solana/blob/aea8f0df1610248d29d8ca3bc0d60e9fabc99e31/web3.js/src/util/url.ts

            let mut ws_url = Url::parse(http_url)?;
            if let Some(port) = ws_url.port() {
                ws_url.set_port(Some(port + 1))
                    .map_err(|_| anyhow!("Unable to set port"))?;
            }
            if ws_url.scheme() == "https" {
                ws_url.set_scheme("wss")
                    .map_err(|_| anyhow!("Unable to set scheme"))?;
            } else {
                ws_url.set_scheme("ws")
                    .map_err(|_| anyhow!("Unable to set scheme"))?;
            }


            Ok(Cluster::Custom(http_url.to_string(), ws_url.to_string()))
        }
        _ => Err(anyhow::Error::msg(
            "Cluster must be one of [localnet, testnet, mainnet, devnet] or be an http or https url\n",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cluster(name: &str, cluster: Cluster) {
        assert_eq!(parse(name).unwrap(), cluster);
    }

    #[test]
    fn test_cluster_parse() {
        test_cluster("testnet", Cluster::Testnet);
        test_cluster("mainnet", Cluster::Mainnet);
        test_cluster("devnet", Cluster::Devnet);
        test_cluster("localnet", Cluster::Localnet);
        test_cluster("debug", Cluster::Debug);
    }

    #[test]
    #[should_panic]
    fn test_cluster_bad_parse() {
        let bad_url = "httq://my_custom_url.test.net";
        parse(bad_url).unwrap();
    }

    #[test]
    fn test_http_port() {
        let url = "http://my-url.com:7000/";
        let cluster = parse(url).unwrap();
        assert_eq!(
            Cluster::Custom(url.to_string(), "ws://my-url.com:7001/".to_string()),
            cluster
        );
    }

    #[test]
    fn test_http_no_port() {
        let url = "http://my-url.com/";
        let cluster = parse(url).unwrap();
        assert_eq!(
            Cluster::Custom(url.to_string(), "ws://my-url.com/".to_string()),
            cluster
        );
    }

    #[test]
    fn test_https_port() {
        let url = "https://my-url.com:7000/";
        let cluster = parse(url).unwrap();
        assert_eq!(
            Cluster::Custom(url.to_string(), "wss://my-url.com:7001/".to_string()),
            cluster
        );
    }
    #[test]
    fn test_https_no_port() {
        let url = "https://my-url.com/";
        let cluster = parse(url).unwrap();
        assert_eq!(
            Cluster::Custom(url.to_string(), "wss://my-url.com/".to_string()),
            cluster
        );
    }

    #[test]
    fn test_upper_case() {
        let url = "http://my-url.com/FooBar";
        let cluster = parse(url).unwrap();
        assert_eq!(
            Cluster::Custom(url.to_string(), "ws://my-url.com/FooBar".to_string()),
            cluster
        );
    }
}
