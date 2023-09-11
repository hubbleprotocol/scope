#!/usr/bin/env bash

# If not all parameters are provided or first param is "help" print a help message
if [ $# -ne 4 ] || [ $1 = "help" ]; then
    echo "Usage: ./total-amount-to-fund.sh <target_fund> <feeds_file> <feeds_name_file> <feeds_balance_file>"
    echo "Example: ./total-amount-to-fund.sh 2 feeds.txt feeds_name.txt feeds_balance.txt"
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
# 3. Store the result in a temporary file
# 4. Sum the total amount to fund using python

# Create unique temporary file
temporary_file=$(mktemp)

# Loop over the feeds, feeds_name and feeds_balance files
paste $feeds_file $feeds_name_file $feeds_balance_file | while IFS="$(printf '\t')" read -r feed feed_name feed_balance; do
    # Calculate the amount to fund
    amount_to_fund=$(./calc-amount-to-fund.sh $target_fund $feed_balance)

    echo "Feed: $feed, Name: $feed_name, Balance: $feed_balance, Amount to fund: $amount_to_fund"

    # Store the result in a temporary file
    echo $amount_to_fund >> $temporary_file
done

# Sum the total amount to fund using python
total=$(python3 -c "import math; print(sum([float(line) for line in open('$temporary_file')]))")
admin_balance=$(solana balance "$ADMIN_KEYPAIR" -u "$RPC_URL" | sed -n 's/ SOL//p')

# If total > admin_balance + 5 SOL, print a warning
if [ $(python3 -c "print(1 if $total > $admin_balance else 0)") -eq 1 ]; then
    echo -e "\033[1;31mERROR:\033[0m Total amount to fund ($total SOL) is greater than admin balance ($admin_balance SOL)"
elif [ $(python3 -c "print(1 if $total > $admin_balance - 5 else 0)") -eq 1 ]; then
    echo -e "\033[1;33mWARNING:\033[0m Total amount to fund ($total SOL) is very close to admin balance ($admin_balance SOL)"
else
    echo "Total amount to fund: $total SOL (admin balance: $admin_balance SOL))"
fi


# Delete the temporary file
rm $temporary_file