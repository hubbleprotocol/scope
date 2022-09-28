# How to create a switchboard feed with a scope price?

## TL;DR

[Create a feed](https://app.switchboard.xyz/build/tool) with a custom job based on the example in [scope-price-job-example.jsonc](./scope-price-job-example.jsonc). Adapt all fields to match your needs.

## Concept

Scope can perform complex computations onchain that might be hard or impossible to express with switchboard. The idea here is to setup a switchboard aggregator with a job that fetch a scope price.

## Limitations

Scope is very strict about price age and detection of stalled price. A scope price timestamp is always the oldest timestamp of all the used sources while switchboard use the "open round" as a timestamp thus reflect the moment of the price computation. The provided job example makes the job fail if the scope price is too old but the switchboard timestamp remains the "open round" timestamp.

When reading a switchboard feed based on scope price it is important to be aware that the price is actually older up to the limit setup in the feed. The user of the switchboard feed can only estimate the worst age even if the price has been refreshed by scope one slot before the switchboard feed has been updated.

## Step by step

### Prepare the job description

We will create a custom job based on the example in [scope-price-job-example.jsonc](./scope-price-job-example.jsonc). The following parts need to be adapted:

- `"programId"` l.27:  
  In the example is set to scope mainnet deployment. Change it if you use a custom scope deployment.
- `"accountAddress"` l.28:  
  Scope price feed pubkey on mainnet. Change if you don't want to use hubble's price feed. Note: All address must live on mainnet-beta. Switchboard jobs are executed off-chain and always read mainnet data even if the price feed is created on devnet.
- `index` in `path` l.33:  
  Must be set to the index of the price to fetch (`42` in the example).
- `@.unixTimestamp > (${CURRENT_TIMESTAMP} - 70` in `path` l.33:  
  The value `70` reflect oldest age of a scope price in second that would be accepted. Increase or reduce depending on how often the scope price or its sources are refreshed.
- `divideTask > scalar` l.38:  
  Must reflect the number of decimals of the scope price. In this example only constant are accepted which is fine for kToken prices like in the example (fixed 8 decimals).

### Test the job

To test the job run [`./switchboard-test.sh your-price-job.json`](./switchboard-test.sh) and check the result is valid.

### Create the feed

1. Go to [switchboard](https://app.switchboard.xyz/build/tool) to create a custom job.
2. Connect your wallet.
3. Press "Import" on the bottom left.
4. Give a name like "Scope" to the job. Check you have JSON selected and copy-paste the part between the `task` square brackets from your tested job to the "Job schema".
5. Press test and check the result.
6. Name your feed and proceed to checkout.
