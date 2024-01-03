# Release 0.10.3

## What's Changed

* ScopeTwap now track the number of samples by @oeble in <https://github.com/hubbleprotocol/scope/pull/213>
* Followup comments by @mihalex98 in <https://github.com/hubbleprotocol/scope/pull/214>
* Get recent prioritization fees in OrbitLink by @oeble in <https://github.com/hubbleprotocol/scope/pull/219>

### Config and CLI changes

* Release notes for version 0.10.2 by @oeble in <https://github.com/hubbleprotocol/scope/pull/210>
* use pyth for JTO price by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/212>
* Add twap to latest price by @oeble in <https://github.com/hubbleprotocol/scope/pull/211>
* Add JITOSOL and LST by @alittlezz in <https://github.com/hubbleprotocol/scope/pull/215>
* add new entry  by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/216>
* support new tokens: CROWN, MEAN, BLOCK, DFL, GP, GUAC by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/218>
* add usdy by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/220>
* add eur oracle by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/222>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/223>
* fix feed name for POLIS by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/224>
* add ldo by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/225>
* add hbb twap + update hbb max age by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/227>
* Update configuration admin to multisig  by @mihalex98 in <https://github.com/hubbleprotocol/scope/pull/217>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.10.2...release/v0.11.0>

## Post merge actions

N/A

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u $URL HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.10.2.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u $URL -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [x] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [x] Launch the bot (possible with `make crank`)
12. [N/A] Merge hubble infra PR to release the bot
