# Scope

_Scope sees all prices in one glance._

[![Integration tests](https://github.com/hubbleprotocol/scope/actions/workflows/ts-integration.yaml/badge.svg)](https://github.com/hubbleprotocol/scope/actions/workflows/ts-integration.yaml)

Scope is a price oracle aggregator living on the Solana network. It copies data from multiple on-chain oracles' accounts into one "price feed".

Scope pre-validate the prices with a preset of rules and perform the update only if they meet the criteria.

The repository contains two software:

- [`scope`](./programs/scope/) on-chain program.
- [`scope-cli`](./off_chain/scope-cli/) that provides administration commands and a bot feature to trigger the price feed update.

## Limitations

- The association between a price at a given index in the price feed and the token pair associated with this price need is not stored on-chain. The label might indicate this association.
- A price feed is currently limited to 512 prices.
- If you do not have access to the Kamino source code, Scope can still be built. See [Building without Kamino ktokens](#building-without-kamino-ktokens) for more details.

## Future updates/ideas

- Allow extensible price feed (when resizable account feature is available in Solana mainnet)

### Running the bot

- For your price feed named "my-feed"

```shell
make build
export CLUSTER=mainnet
export URL=<url>
export FEED="my-feed"
make crank
# Expand to something similar to:
# RUST_BACKTRACE=1 cargo run -p scope-cli -- --keypair <keypair.json> --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble crank
```

### Building without Kamino kTokens

If you do not have access to the Kamino source code, you can still build scope without the default `yvaults` feature:

- Replace the `yvaults` dependency in `./programs/scope/Cargo.toml` with the `yvaults_stub` package:

```toml
[dependencies]
# Comment out the git repo
#yvaults = { git = "ssh://git@github.com/hubbleprotocol/yvault.git", features = ["no-entrypoint", "cpi", "mainnet"], optional = true }

# Add this line
yvaults = { path = "../yvaults_stub", package = "yvaults_stub", optional = true }
```

- Build scope with the following command:

```sh
anchor build -p scope -- --no-default-features --features mainnet
```

- Build the CLI:

```sh
cargo build -p scope-cli --no-default-features --features rpc-client
```
