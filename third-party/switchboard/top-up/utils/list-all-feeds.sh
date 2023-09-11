#!/usr/bin/env bash

# Move current directory to the directory of this script
cd "$(dirname "${BASH_SOURCE[0]}")"

# If ADMIN_KEYPAIR is not set or RPC_URL is not set, source ../.env
if [ -z ${ADMIN_KEYPAIR+x} ] || [ -z ${RPC_URL+x} ]; then
    source ../.env
fi

# List all feeds
# Read owner from keypair
owner=`solana-keygen pubkey "$ADMIN_KEYPAIR"`
sb solana aggregator list $owner --mainnetBeta -u "$RPC_URL"