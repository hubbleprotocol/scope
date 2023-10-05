# Release 0.4.1

## What's Changed

Adds support for Raydium ktoken prices.

### Smart contract

* Add yvaults as an optional dependency to scope and re-use functions for kToken price calcs by @elliotkennedy in https://github.com/hubbleprotocol/scope/pull/158

### Other

* ✨ Add multiple kTokens by @oeble in https://github.com/hubbleprotocol/scope/pull/153
* Remove ctokens from conf by @oeble in https://github.com/hubbleprotocol/scope/pull/154
* :wrench: Add kjitoSOL-USDC config by @andreihrs in https://github.com/hubbleprotocol/scope/pull/155
* 🩹 Fix clone mainnet tool by @oeble in https://github.com/hubbleprotocol/scope/pull/156
* Copy scope types to a separate lib by @elliotkennedy in https://github.com/hubbleprotocol/scope/pull/157
* Add LST by @oeble in https://github.com/hubbleprotocol/scope/pull/159

**Full Changelog**: https://github.com/hubbleprotocol/scope/compare/release/v0.4.0...release/v0.4.1

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

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u <mainnet_rpc> -k <payer>`
7. [x] Dump old program in case of rollback: `solana program dump -u <mainnet_rpc> HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.4.0.so` 
8. [x] Launch the bot (possible with `make crank`)
9. [x] Merge hubble infra PR to release the bot
