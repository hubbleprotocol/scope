ifneq (,$(wildcard ./.env))
	include ./.env
endif

ORACLE_PROGRAM_ID=A9DXGTCMLJsX7kMfwJ2aBiAFACPmUsxv6TRxcEohL4CD
FAKE_PYTH_PROGRAM_ID=3URDD3Eutw6SufPBzNm2dbwqwvQjRUFCtqkKVsjk3uSE
# TODO: Or 4sZs4ybFfqttgsssLZRZ7659KLg3RJL5gDTFdo7ApsAC ? How to get this right on first deployement?
PROGRAM_DEPLOY_ACCOUNT=$(shell eval solana-keygen pubkey ./keys/${CLUSTER}/owner.json)

.PHONY: build deploy build-client run listen deploy-new

build:
	anchor build -p oracle
	anchor build -p pyth

# Only use this when you want to deploy the program at a new address (or for the first time)
# otherwise use the "deploy" to deploy to the old address
deploy: airdrop
	anchor deploy -p oracle --provider.wallet ./keys/${CLUSTER}/owner.json
	anchor deploy -p pyth --provider.wallet ./keys/${CLUSTER}/owner.json

# Use these whenever you already have a program id
upgrade: airdrop
	anchor upgrade ./target/deploy/oracle.so --program-id $(ORACLE_PROGRAM_ID) --provider.wallet ./keys/${CLUSTER}/owner.json
	anchor upgrade ./target/deploy/oracle.so --program-id $(FAKE_PYTH_PROGRAM_ID) --provider.wallet ./keys/${CLUSTER}/owner.json

## Listen to on-chain logs
listen:
	solana logs $(ORACLE_PROGRAM_ID)


## Client side
build-client:
	npm run build

run:
	npm run start

airdrop:
	solana airdrop 10 ${PROGRAM_DEPLOY_ACCOUNT} --url http://127.0.0.1:8899