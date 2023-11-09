# OrbitLink

OrbitLink is a convenience layer above Solana's async RPC clients. OrbitLink supports Anchor-based programs.

## Concept

This client helps to abstract away:

- Lookup-tables creation and automatic usage.
- Compute-units management.
- Priority fee adjustment.
- Transaction retries.
- Support for banks-client, allowing direct connection to bpf-tests.
