# Release 0.13.0

## What's Changed

### Smart contract changes

* Dlmm prices by @oeble in <https://github.com/hubbleprotocol/scope/pull/243>
* Bot improvements by @oeble in <https://github.com/hubbleprotocol/scope/pull/237>
* Spl stake epoch check by @oeble in <https://github.com/hubbleprotocol/scope/pull/238>
* Release 0.12.0 by @oeble in <https://github.com/hubbleprotocol/scope/pull/232>

### Config changes

* add puff and upsidedowncat by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/240>
* fix twap source for mockJUP by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/242>
* add mockJUP token by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/241>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/239>
* add new tokens MASH, ONE, HONEY by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/235>
* Config updates: Scope Twap and EURC by @oeble in <https://github.com/hubbleprotocol/scope/pull/234>
* Upgrade helm service-base-chart -> 0.1.0 by @elliotkennedy in <https://github.com/hubbleprotocol/scope/pull/233>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.12.0...release/v0.13.0>

## Post merge actions

N/A

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u $MAINNET_RPC_URL HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.12.0.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u $URL -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [N/A] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [x] Launch the bot (possible with `make crank`)
12. [x] Merge hubble infra PR to release the bot
