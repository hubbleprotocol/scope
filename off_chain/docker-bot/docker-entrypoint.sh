#!/usr/bin/env bash
set -e

# Need env vars:
# VALIDATOR_RPC_URL
# PROGRAM_ID
# KEYPAIR (tx payer)
# REFRESH_INTERVAL_SLOT (optional default to 30)
# JSON_LOGS
# PRICE_FEED

exec ./scope --cluster "$VALIDATOR_RPC_URL" crank $@
