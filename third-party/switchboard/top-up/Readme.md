# Tool to automatically top up switchboard feeds

This is a collection of helper scripts to check the current balance of switchboard feeds and ensure a minimum balance in all of them.

## Requirements

The following tools must be in path:

- Bash
- Python 3
- Solana cli in a recent version
- Switchbord cli in a recent version `npm i -g @switchboard-xyz/cli`

## Setup

The scripts expect a `.env` file placed in the same folder as this Readme. A `.env.example` is provided.

## Usage

```shell
./auto.sh
```

You can explore the content of the `utils` folder for manual operations.
