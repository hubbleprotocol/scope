#!/usr/bin/env bash

# Move current directory to the directory of this script
cd "$(dirname "${BASH_SOURCE[0]}")"

# If ADMIN_KEYPAIR is not set or RPC_URL is not set, source ../.env
if [ -z ${ADMIN_KEYPAIR+x} ] || [ -z ${RPC_URL+x} ]; then
    source ../.env
fi

# Param 1 is the feed address
FEED=$1

sb solana aggregator print $FEED --mainnetBeta -u "$RPC_URL" | grep balance | awk '{print $2}'