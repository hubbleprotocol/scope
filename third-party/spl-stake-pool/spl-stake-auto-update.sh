#!/usr/bin/env bash
uname=$(uname);

parent_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$parent_path"

case "$uname" in
    (*Linux*) jq 'to_entries | map(.value) | .[]' ../../configs/mainnet/hubble.json | tail -n +2 | jq 'select(.oracle_type=="SplStake") | .oracle_mapping' | xargs -i bash -c 'spl-stake-pool update {}'; ;;
    (*Darwin*) jq 'to_entries | map(.value) | .[]' ../../configs/mainnet/hubble.json | tail -n +2 | jq 'select(.oracle_type=="SplStake") | .oracle_mapping' | gxargs -i bash -c 'spl-stake-pool update {}'; ;;
    (*) echo 'error: unsupported platform.'; exit 2; ;;
esac;

