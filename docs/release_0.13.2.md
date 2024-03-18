# Release 0.13.2

## What's Changed

### Smart contract changes

* Use prev price on pyth feed when current one is not TRADING by @oeble in <https://github.com/hubbleprotocol/scope/pull/267>

### CLI changes

* Update CLI to update mapping only for spot/twap by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/249>
* Remove patched cargo chef install from docker build by @elliotkennedy in <https://github.com/hubbleprotocol/scope/pull/259>

### Config changes

* add WYNN and ARAB by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/247>
* add Jup entry by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/248>
* Add JUP/USD feeds from Pyth by @oeble in <https://github.com/hubbleprotocol/scope/pull/250>
* add ARAB-JUP entry by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/251>
* remove trailing comma by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/252>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/253>
* fix typo by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/254>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/255>
* update ponke oracle by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/256>
* add wealth token by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/257>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/260>
* fix typo in config by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/261>
* enable twap for catwifhat by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/262>
* add new tokens by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/263>
* add LIKE and DITH by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/264>
* add new kToken by @silviutroscot in <https://github.com/hubbleprotocol/scope/pull/265>
* Remove JSOL, add switchboard USDC by @oeble in <https://github.com/hubbleprotocol/scope/pull/266>

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.13.1...release/v0.13.2>

## Post merge actions

N/A

## Mainnet Commands

1. [x] Set `$CLUSTER` to mainnet: `export CLUSTER=mainnet`
2. [x] Set `$URL` to a good RPC
3. [x] Set `$FEED_NAME` to something good like `hubble`
4. [x] Check everything is correct with `make check-env`
5. [x] `make build` and check that it actually builds
6. [x] Dump old program in case of rollback: `solana program dump -u $MAINNET_RPC_URL HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.13.1.so`
7. [x] Put owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds: `solana balance keys/mainnet/owner.json -u m`
8. [x] Write buffer `solana program write-buffer target/deploy/scope.so -u $URL -k ./keys/$CLUSTER/owner.json`
9. [x] Make proposal on squads
10. [N/A] Update the IDL `anchor idl upgrade --provider.cluster mainnet --provider.wallet ./keys/mainnet/owner.json --filepath target/idl/scope.json HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ`
11. [x] Launch the bot (possible with `make crank`)
12. [N/A] Merge hubble infra PR to release the bot
