# Release 0.3.0


## What's Changed
* Add back FTT and SRM by @oeble in https://github.com/hubbleprotocol/scope/pull/108
* Add BONK-USD by @oeble in https://github.com/hubbleprotocol/scope/pull/109
* Add dynamic precision by @y2kappa in https://github.com/hubbleprotocol/scope/pull/110
* Add CEXs to BONK by @oeble in https://github.com/hubbleprotocol/scope/pull/111
* Add SAMO, bSOL, laineSOL and multiple perf improvement. by @oeble in https://github.com/hubbleprotocol/scope/pull/112
* Add HADES price by @oeble in https://github.com/hubbleprotocol/scope/pull/113
* Add MEXC to HADES price sources by @oeble in https://github.com/hubbleprotocol/scope/pull/114
* Make scope client async by @oeble in https://github.com/hubbleprotocol/scope/pull/115
* 👷 Simplify CI workflow with new solana setup action by @oeble in https://github.com/hubbleprotocol/scope/pull/118
* 🔊 Rework crank logs by @oeble in https://github.com/hubbleprotocol/scope/pull/117
* ⚡️ Reduce the number of CU spent on error prints. by @oeble in https://github.com/hubbleprotocol/scope/pull/122
* Print price age without color for parsing by @oeble in https://github.com/hubbleprotocol/scope/pull/119


**Full Changelog**: https://github.com/hubbleprotocol/scope/compare/release/v0.2.1...release/v0.3.0

## Post merge actions

* N/A

## Dev Commands

1. [x] Set `$CLUSTER` to devnet: `export CLUSTER=devnet`
2. [N/A] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`: `export FEED_NAME=hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] Put/generate owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/devnet/owner.json -u d`
6. [x] `make build` and check that it actually builds
7. [x] Build scope-cli in release mode (`cargo build -p scope-cli --release`)
8. [x] `make deploy-scope` (we don't want to deploy fake-pyth)
9. [x] Launch the bot (possible with `make crank`)


## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet/devnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
6. [x] `make build` and check that it actually builds
7. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u <mainnet_rpc> -k <payer>`
8.[x] Launch the bot (possible with `make crank`)
