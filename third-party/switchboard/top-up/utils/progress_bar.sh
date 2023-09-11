#!/usr/bin/env bash

# 1- Current progress
current=$1
# 2- Total
total=$2
# 3- Message
message=$3

# Add padding or tuncate message to an exact 30 characters
message=$(printf "%-30s" "$message")

# Print a progress bar using python to draw the bar
# Example: [##########          ] 50% | Message
python3 -c "import math; print('\r[' + '#' * math.floor($current / $total * 20) + ' ' * math.floor((1 - $current / $total) * 20) + '] ' + str($current) + '/' + str($total) + ' | ' + '$message', end='')"
