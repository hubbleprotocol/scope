#!/bin/env python3

# This script goes through all switchboard feeds and update their configuration
# If the feed request 6 oracles or more it set the requested oracles to 5 (oracleRequestBatchSize)
# If the feed request a minimum oracles answers above 5 it set the minimum requested to 4 (minOracleResults)
# If the feed refresh period (minUpdateDelaySeconds) is lower than 60s it set it to 60s and the priority fee bump (priorityFeeBumpPeriod) to 65s

import json
import os
import sys


# Function to get the list of feeds from sb cli
def getFeeds():
    # Get the owner pubkey
    # owner=`solana-keygen pubkey "$ADMIN_KEYPAIR"`
    owner = os.popen('solana-keygen pubkey "$ADMIN_KEYPAIR"').read().strip()

    cmd = "sb solana aggregator list " + owner + ' --mainnetBeta -u "$RPC_URL"'
    print("Getting feeds with command: " + cmd)
    output = os.popen(cmd).read()
    # Outputs is a pubkey per line
    feeds = output.splitlines()
    feeds = [feed for feed in feeds if feed]  # remove empty strings
    return feeds


# Function to get the current configuration of a feed from sb cli
def getFeedConfig(feed):
    cmd = "sb solana aggregator print " + feed + ' --mainnetBeta -u "$RPC_URL"'
    output = os.popen(cmd).read()
    # Parse the output
    # We need config with oracleRequestBatchSize, minOracleResults, minUpdateDelaySeconds, priorityFeeBumpPeriod
    config = {}
    for line in output.splitlines():
        if "oracleRequestBatchSize" in line:
            config["oracleRequestBatchSize"] = int(line.split()[1])
        if "minOracleResults" in line:
            config["minOracleResults"] = int(line.split()[1])
        if "minUpdateDelaySeconds" in line:
            config["minUpdateDelaySeconds"] = int(line.split()[1])
        if "priorityFeeBumpPeriod" in line:
            config["priorityFeeBumpPeriod"] = int(line.split()[1])
        # If the name is not set we get it from the output (name can contain spaces)
        if "name" in line and "name" not in config:
            config["name"] = line.split(None, 1)[1]

    return config


# Function to update the configuration of a feed with sb cli according to earlier criteria
def updateFeedConfig(feed, config):
    cmd = (
        "sb solana aggregator set "
        + feed
        + ' --mainnetBeta -u "$RPC_URL" -k "$ADMIN_KEYPAIR"'
    )
    execute = False
    if config["oracleRequestBatchSize"] != config["minOracleResults"] + 1:
        cmd += " --batchSize=" + str(config["minOracleResults"] + 1)
        execute = True
    # if config["minOracleResults"] > 4:
    #     cmd += " --minOracles=4"
    #     execute = True
    # if config["minUpdateDelaySeconds"] < 60:
    #     cmd += " --updateInterval=60"
    #     execute = True
    if config["priorityFeeBumpPeriod"] != config["minUpdateDelaySeconds"] + 10:
        cmd += " --priorityFeeBumpPeriod=" + str(config["minUpdateDelaySeconds"] + 10)
        execute = True

    if execute:
        print(
            "Updating feed " + config["name"] + " (" + feed + ") with command: \n" + cmd
        )
        output = os.popen(cmd).read()
        print(output)


# main
if __name__ == "__main__":
    # Move current directory to the directory of this script
    os.chdir(os.path.dirname(os.path.abspath(__file__)))

    # Ensure ADMIN_KEYPAIR is set and RPC_URL is set
    if not os.environ.get("ADMIN_KEYPAIR"):
        print("ADMIN_KEYPAIR is not set source .env file")
        sys.exit(1)
    if not os.environ.get("RPC_URL"):
        print("RPC_URL is not set source .env file")
        sys.exit(1)

    # Get the list of feeds
    feeds = getFeeds()

    # For each feed get the current configuration and update it if needed
    for feed in feeds:
        config = getFeedConfig(feed)
        updateFeedConfig(feed, config)

    sys.exit(0)
