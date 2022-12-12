# SPL Stake pool update

Every epoch the SPL stake pool need to be refreshed with the rewards from all validators of the pool. If not done, the price will remain stalled. Most of the small pools don't do it automatically, to attempt to do it, anyone can run the provided script [`./spl-stake-auto-update.sh`](./spl-stake-auto-update.sh).

## Pre-requirement

The provided script requires to have a valid solana config (`solana config get`) targeting mainnet.
