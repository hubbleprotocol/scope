# Release 0.8.1

## What's Changed

* Fix overflow if no sample in past hour and twap ts by @oeble in <https://github.com/hubbleprotocol/scope/pull/183>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.8.0...release/v0.8.1>

## Post merge actions

* Update mapping

## Dev Commands

1. [x] Set `$CLUSTER` to devnet: `export CLUSTER=devnet`
2. [x] Set `$FEED_NAME` to something good like `hubble`: `export FEED_NAME=hubble`
3. [x] Check everything is correct with `make check-env`
4. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/devnet/owner.json -u d`
5. [x] `make build` and check that it actually builds
6. [x] `make deploy`
7. [x] Launch the bot (possible with `make crank`)
8. [x] Update the IDL `anchor idl upgrade --provider.cluster devnet --provider.wallet ./keys/devnet/owner.json --filepath target/idl/scope.json 3Vw8Ngkh1MVJTPHthmUbmU2XKtFEkjYvJzMqrv2rh9yX`
9. [x] Merge hubble infra PR to release the bot

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u <mainnet_rpc> HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.8.0.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u <mainnet_rpc> -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [x] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [x] Launch the bot (possible with `make crank`)
12. [x] Merge hubble infra PR to release the bot
