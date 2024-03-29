# CLI HELP

### How to use
- Running the following line compiles the CLI and outputs all the available commands
`cargo run -p scope-cli -- help`
- Running the following line tells you what args the command expects
`cargo run -p scope-cli -- help <command-name>`

#### Examples
- Set admin-cached for a price_feed configuration
`cargo run -p scope-cli -- --keypair keys/localnet/owner.json --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble --cluster localnet set-admin-cached --admin-cached yV4XWzpBQZ6bamKWaAZTPhEk3WphNZsaVqQ2msAX4Cr` - to execute
`cargo run -p scope-cli -- --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble --cluster localnet --multisig yV4XWzpBQZ6bamKWaAZTPhEk3WphNZsaVqQ2msAX4Cr set-admin-cached --admin-cached yV4XWzpBQZ6bamKWaAZTPhEk3WphNZsaVqQ2msAX4Cr` - to get encoded message base58
- Approve admin-cached for a price_feed configuration
`cargo run -p scope-cli -- --keypair keys/localnet/owner.json --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble --cluster localnet approve-admin-cached` - to execute
`cargo run -p scope-cli -- --program-id HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ --price-feed hubble --cluster mainnet --multisig yV4XWzpBQZ6bamKWaAZTPhEk3WphNZsaVqQ2msAX4Cr approve-admin-cached` - to get encoded message base58 and simulation base64 - for multisig 
