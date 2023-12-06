# Release 0.10.0

## What's Changed

* add tests for twap reset by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/200>
* Increase the twap smoothing factor precision. by @oeble in <https://github.com/hubbleprotocol/scope/pull/201>
* Add clmm prices by @oeble in <https://github.com/hubbleprotocol/scope/pull/203>
* Change ktokens to token x give price in token instead of lamports by @oeble in <https://github.com/hubbleprotocol/scope/pull/202>
* Update scope admin by @mihalex98 in <https://github.com/hubbleprotocol/scope/pull/196>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.9.1...release/v0.10.0>

## Post merge actions

N/A

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u $URL HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.9.1.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u $URL -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [x] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [ ] Launch the bot (possible with `make crank`)
12. [ ] Merge hubble infra PR to release the bot
