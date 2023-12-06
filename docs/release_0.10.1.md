# Release 0.10.0

## What's Changed

* Release 0.10.0 by @oeble in <https://github.com/hubbleprotocol/scope/pull/204>
* add SHDW scope entry by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/205>
* ✨ OrbitLink get all accounts of given type by @oeble in <https://github.com/hubbleprotocol/scope/pull/194>
* shdw token per ktoken by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/206>
* Update deps by @oeble in <https://github.com/hubbleprotocol/scope/pull/207>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.10.0...release/v0.10.1>

## Post merge actions

N/A

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u $URL HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.10.0.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u $URL -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [N/A] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [N/A] Launch the bot (possible with `make crank`)
12. [N/A] Merge hubble infra PR to release the bot
