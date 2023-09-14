# Release 0.4.0

## What's Changed

Smart contract API change - kTokens now take additional `GlobalConfig` and `CollateralInfos` accounts, whereas previously they were using a `ScopeChain` account, which has been removed from the Kamino program.

### Smart contract

* Calculate kToken price with Kamino `CollateralInfos` account rather than `ScopeChain` by @elliotkennedy in [151](https://github.com/hubbleprotocol/scope/pull/151)

### Other

* Add HNT token by @y2kappa in [134](https://github.com/hubbleprotocol/scope/pull/134)
* Add helium MOBILE and IOT prices by @oeble in [135](https://github.com/hubbleprotocol/scope/pull/135)
* Add NANA feeds by @oeble in [136](https://github.com/hubbleprotocol/scope/pull/136)
* Add STEP/USD feed by @oeble in [137](https://github.com/hubbleprotocol/scope/pull/137)
* Restore wstETH by @oeble in [138](https://github.com/hubbleprotocol/scope/pull/138)
* Add Forge price feed by @oeble in [139](https://github.com/hubbleprotocol/scope/pull/139)
* add COCO switchboard feed by @silviutroscot in [140](https://github.com/hubbleprotocol/scope/pull/140)
* add STYLE token oracle by @silviutroscot in [142](https://github.com/hubbleprotocol/scope/pull/142)
* add Style TWAP + Chai  by @silviutroscot in [143](https://github.com/hubbleprotocol/scope/pull/143)
* add feeds for t and blze tokens by @silviutroscot in [144](https://github.com/hubbleprotocol/scope/pull/144)
* add EUROE by @silviutroscot in [145](https://github.com/hubbleprotocol/scope/pull/145)
* Update wsteth to switchboard by @oeble in [147](https://github.com/hubbleprotocol/scope/pull/147)
* Move to docker build to bullseye slim by @oeble in [148](https://github.com/hubbleprotocol/scope/pull/148)
* Add a placeholder for the deprecated `OracleType.YiToken` by @elliotkennedy in [149](https://github.com/hubbleprotocol/scope/pull/149)
* 🎉 Add tools to manage switchboard feeds balance. by @oeble in [150](https://github.com/hubbleprotocol/scope/pull/150)

**Full Changelog**: <https://github.com/hubbleprotocol/scope/compare/release/v0.3.1...release/v0.4.0>

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
7. [x] Dump old program in case of rollback: `solana program dump -u <mainnet_rpc> HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ scope-0.3.1.so` 
8. [x] Launch the bot (possible with `make crank`)
9. [x] Merge hubble infra PR to release the bot
