#!/usr/bin/env bash

# Move current directory to the directory of this script
cd "$(dirname "${BASH_SOURCE[0]}")"

# If ADMIN_KEYPAIR is not set or RPC_URL is not set, source .env
if [ -z ${ADMIN_KEYPAIR+x} ] || [ -z ${RPC_URL+x} ]; then
    source .env
fi

# Function to if user want to continue or abort script
# Default to continue (yes)
function continue_or_abort {
    read -p "Continue? [Y/n] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Nn]$ ]]; then
        echo "Aborting..."
        exit 1
    fi
}

ADMIN_PUBKEY=`solana-keygen pubkey "$ADMIN_KEYPAIR"`

echo "This script will top up all switchboard feeds owned by $ADMIN_KEYPAIR ($ADMIN_PUBKEY) to a target amount per feed."

# Ask for target amount per feed, default to 2, re-ask if invalid float number is provided
while true; do
    read -p "Enter target amount per feed (default: 2): " target_fund
    target_fund=${target_fund:-2}
    if [[ $target_fund =~ ^[+-]?[0-9]+\.?[0-9]*$ ]]; then
        break
    else
        echo "Invalid number, please try again."
    fi
done

# Create temporary files for feeds, feeds name and feeds balance
feeds_file=$(mktemp)
feeds_name_file=$(mktemp)
feeds_balance_file=$(mktemp)

# List all feeds
./utils/list-all-feeds.sh > $feeds_file

total_feeds=$(wc -l < $feeds_file)

# Get feeds name with progress bar
echo "Getting feeds name..."
count=1
for feed in $(cat $feeds_file); do
    ./utils/progress_bar.sh $((count++)) $total_feeds "Feed: $feed"
    sb solana aggregator print $feed --mainnetBeta -u "$RPC_URL" | grep name | head -n1 | sed -n 's/^name[[:space:]]*//p' >> $feeds_name_file
done
echo " "

# Get feeds balance with progress bar
echo "Getting feeds balance..."
count=1
for feed in $(cat $feeds_file); do
    ./utils/progress_bar.sh $((count++)) $total_feeds "Feed: $feed"
    ./utils/get-feed-balance.sh $feed >> $feeds_balance_file
done
echo " "


# Calculate total amount to fund
./utils/total-amount-to-fund.sh $target_fund $feeds_file $feeds_name_file $feeds_balance_file

continue_or_abort

# Fund feeds
./utils/auto-fund.sh $target_fund $feeds_file $feeds_name_file $feeds_balance_file

rm $feeds_file
rm $feeds_name_file 
rm $feeds_balance_file