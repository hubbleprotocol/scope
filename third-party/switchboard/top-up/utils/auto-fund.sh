#!/usr/bin/env bash

# If not all parameters are provided or first param is "help" print a help message
if [ $# -ne 4 ] || [ $1 = "help" ]; then
    echo "Usage: ./auto-fund.sh <target_fund> <feeds_file> <feeds_name_file> <feeds_balance_file>"
    echo "Example: ./auto-fund.sh 2 all-feeds all-feeds-names all-feeds-balances"
    exit 1
fi

# Function that check if a file exists and return its absolute path
function check_file {
    if [ ! -f "$1" ]; then
        echo "File $1 does not exist"
        exit 1
    fi
    echo $(realpath $1)
}

# Param 1 is the target fund amount per feed (float)
target_fund=$1
# Param 2 is the file containing the list of feeds
feeds_file=$(check_file $2)
# Param 3 is the file containing the list of feeds name
feeds_name_file=$(check_file $3)
# Param 4 is the file containing the list of feeds balance
feeds_balance_file=$(check_file $4)

# Move current directory to the directory of this script
cd "$(dirname "${BASH_SOURCE[0]}")"

# If ADMIN_KEYPAIR is not set or RPC_URL is not set, source ../.env
if [ -z ${ADMIN_KEYPAIR+x} ] || [ -z ${RPC_URL+x} ]; then
    source ../.env
fi

# For each feed
# 1. Get the current feed balance
# 2. Calculate the amount to fund
# 3. Fund the feed

total_feeds=$(wc -l < $feeds_file)

# Loop over the feeds, feeds_name and feeds_balance files
count=1
paste $feeds_file $feeds_name_file $feeds_balance_file | while IFS="$(printf '\t')" read -r feed feed_name feed_balance; do
    # Calculate the amount to fund
    amount_to_fund=$(./calc-amount-to-fund.sh $target_fund $feed_balance)
    
    # Skip if amount to fund is 0
    if [ $amount_to_fund = "0" ]; then
        echo "[$count/$total_feeds] Skipping feed $feed, Name: $feed_name, Balance: $feed_balance"
        ((count++))
        continue
    fi

    echo "[$count/$total_feeds] Feed: $feed, Name: $feed_name, Balance: $feed_balance, Amount to fund: $amount_to_fund"

    sb solana aggregator fund $feed --amount "$amount_to_fund" --keypair "$ADMIN_KEYPAIR" --mainnetBeta -u "$RPC_URL"
    # Print separator
    echo "------------------------------------------------------------------------------------------------------------------------"
    ((count++))
done

