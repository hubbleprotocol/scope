# Release 0.3.1

## What's Changed

Main change is the introduction of protections against price refresh in flash loans and CPIs.

### Smart contract

* Introduce security tests by @oeble in [#125](https://github.com/hubbleprotocol/scope/pull/125)
* 🔐 Forbid CPI and flash op on refresh prices by @oeble in [#132](https://github.com/hubbleprotocol/scope/pull/132)

### Other

* Release 0.3.0 by @silviutroscot in [#123](https://github.com/hubbleprotocol/scope/pull/123)
* Add BTC/USD, RLB/USD, STSOL/SOL by @y2kappa in [#124](https://github.com/hubbleprotocol/scope/pull/124)
* CGNTSOL Oracle + make Makefile commands OS agnostic by @silviutroscot in [#126](https://github.com/hubbleprotocol/scope/pull/126)
* Add get_slot_with_commitment function to orbit-link async trait by @elliotkennedy in [#127](https://github.com/hubbleprotocol/scope/pull/127)
* Add HXRO price feed by @oeble in [#128](https://github.com/hubbleprotocol/scope/pull/128)
* Update SPL stake on Mac by @silviutroscot in [#129](https://github.com/hubbleprotocol/scope/pull/129)
* Add simulate_transaction to AsyncClient by @elliotkennedy in [#130](https://github.com/hubbleprotocol/scope/pull/130)
* Add MNDE Twap, Remove lower bounds by @oeble in [#131](https://github.com/hubbleprotocol/scope/pull/131)

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.3.0...release/v0.3.1>

## Post merge actions

* N/A

## Dev Commands

1. [x] Set `$CLUSTER` to devnet: `export CLUSTER=devnet`
2. [x] Set `$FEED_NAME` to something good like `hubble`: `export FEED_NAME=hubble`
3. [x] Check everything is correct with `make check-env`
4. [x] Put/generate owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/devnet/owner.json -u d`
5. [x] `make build` and check that it actually builds
6. [x] `make deploy-scope` (we don't want to deploy fake-pyth)
7. [x] Launch the bot (possible with `make crank`)
8. [x] Merge hubble infra PR to release the bot

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet/devnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u <mainnet_rpc> -k <payer>`
7. [x] Launch the bot (possible with `make crank`)
8. [x] Merge hubble infra PR to release the bot
