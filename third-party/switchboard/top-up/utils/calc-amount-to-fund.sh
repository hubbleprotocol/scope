#!/usr/bin/env bash

# Move current directory to the directory of this script
cd "$(dirname "${BASH_SOURCE[0]}")"

# If ADMIN_KEYPAIR is not set or RPC_URL is not set, source ../.env
if [ -z ${ADMIN_KEYPAIR+x} ] || [ -z ${RPC_URL+x} ]; then
    source ../.env
fi

# Param 1 is the target fund amount (float)
target_fund=$1
# Parm 2 is the current fund (float)
current_fund=$2

# The amount to fund (target - current) is rounded up to the nearest 0.1 SOL with min value of 0
# Calculate and print using python to avoid any floating point issues

python3 -c "import math; print(max(0, math.ceil(($target_fund - $current_fund) * 10) / 10))"
