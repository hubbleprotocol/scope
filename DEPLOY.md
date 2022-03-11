# Deploy procedure

 1. Set `$CLUSTER` to mainnet/devnet
 2. Set `$URL` to a good RPC
 3. Set `$FEED_NAME` to something good like `hubble`
 4. Put/generate owner keypair in `./keys/$CLUSTER/owner.json` and ensure you have enough funds
 5. `make build` and check that it actually builds
 6. Build scope-cli in release mode (`cargo build -p scope-cli --release`)
 7. Check the keys in `./keys/$CLUSTER` and save them
 8. `make deploy-scope` (we don't want to deploy fake-pyth)
 9. `make init` (initialize using scope-cli and oracle mapping in `./configs/$CLUSTER/$FEED_NAME.json`, configuration account seed is set to `$FEED_NAME`)
10. Launch the bot (possible with `make crank`)