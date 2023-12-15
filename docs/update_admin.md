## Steps

- [x] set admin cached
`cargo run -p scope-cli -- --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --keypair ./keys/mainnet/owner.json --price-feed hubble --cluster $MAINNET_RPC_URL  set-admin-cached --admin-cached E35i5qn7872eEmBt15e5VGhziUBzCTm43XCSWvDoQNNv`

- [x] approve admin cached multisig
`cargo run -p scope-cli -- --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble --cluster $MAINNET_RPC_URL --multisig E35i5qn7872eEmBt15e5VGhziUBzCTm43XCSWvDoQNNv approve-admin-cached | tee ixns.in && squads-kamino-cli create-transaction-from-instructions-file --cluster $MAINNET_RPC_URL --ledger-pubkey 5zY3XWg7hX5tSwGq1az5CVGRDfvHRK66xoZ6vvMrJ2y8 --multisig E35i5qn7872eEmBt15e5VGhziUBzCTm43XCSWvDoQNNv --file-path ./ixns.in --title "Scope-019: Update configuration admin"`

- [x] update mapping eth/usd max_age: 100 - to test
`make update-mapping CLUSTER=mainnet | tee ixns.in && squads-kamino-cli create-transaction-from-instructions-file --cluster $MAINNET_RPC_URL --ledger-pubkey 5zY3XWg7hX5tSwGq1az5CVGRDfvHRK66xoZ6vvMrJ2y8 --multisig E35i5qn7872eEmBt15e5VGhziUBzCTm43XCSWvDoQNNv --file-path ./ixns.in --title "Scope-020: Update scope max age"`