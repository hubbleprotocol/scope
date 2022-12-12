#!/usr/bin/env bash

parent_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$parent_path"
jq 'to_entries | map(.value) | .[]' ../../configs/mainnet/hubble.json | tail -n +2 | jq 'select(.oracle_type=="SplStake") | .oracle_mapping' | xargs -i bash -c 'spl-stake-pool update {}'
