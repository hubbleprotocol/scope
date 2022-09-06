# Makefile with attempt to make it more reliable
# please read https://tech.davis-hansson.com/p/make/
SHELL := bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

define DEPENDABLE_VAR

.PHONY: phony
$1: phony
>@ if [[ `cat $1 2>&1` != '$($1)' ]]; then \
     echo -n $($1) > $1 ; \
   fi

endef

# TODO: not sure if it really works
ifneq (,$(wildcard ./.env))
	include ./.env
endif

CLUSTER ?= localnet
OWNER_KEYPAIR ?= ./keys/$(CLUSTER)/owner.json
FEED_NAME ?= hubble

#declare CLUSTER to be dependable
$(eval $(call DEPENDABLE_VAR,CLUSTER))

ifeq ($(CLUSTER),localnet)
	URL ?= "http://127.0.0.1:8899"
endif
ifeq ($(CLUSTER),mainnet)
	URL ?= "https://solana-api.projectserum.com"
endif
ifeq ($(CLUSTER),mainnet-beta)
	URL ?= "https://api.mainnet-beta.solana.com"
endif
ifeq ($(CLUSTER),devnet)
	URL ?= "https://api.devnet.solana.com"
endif
ifeq ($(URL),)
# URL is still empty, CLUSTER is probably set to an URL directly
# TODO: is this logical?
	URL = $(CLUSTER)
endif

SCOPE_PROGRAM_KEYPAIR := keys/$(CLUSTER)/scope.json
FAKE_ORACLES_PROGRAM_KEYPAIR := keys/$(CLUSTER)/mock_oracles.json

SCOPE_PROGRAM_SO := target/deploy/scope.so
FAKE_ORACLES_PROGRAM_SO := target/deploy/mock_oracles.so
SCOPE_CLI := target/debug/scope

SCOPE_PROGRAM_ID != solana-keygen pubkey $(SCOPE_PROGRAM_KEYPAIR)
FAKE_ORACLES_PROGRAM_ID != solana-keygen pubkey $(FAKE_ORACLES_PROGRAM_KEYPAIR)
PROGRAM_DEPLOY_ACCOUNT != solana-keygen pubkey $(OWNER_KEYPAIR)

.PHONY: deploy run listen deploy deploy-int airdrop test test-rust test-ts init check-env format

check-env:
>@ echo "CLUSTER=$(CLUSTER)" 
>@ echo "URL=$(URL)" 
>@ echo "FEED_NAME=$(FEED_NAME)"

build: $(SCOPE_PROGRAM_SO) $(FAKE_ORACLES_PROGRAM_SO) $(SCOPE_CLI)

$(SCOPE_CLI): $(shell find programs -name "*.rs") $(shell find off_chain -name "*.rs") $(shell find off_chain -name "Cargo.toml") Cargo.lock
> cargo build -p scope-cli

# Don't autodelete the keys, we want to keep them as much as possible 
.PRECIOUS: keys/$(CLUSTER)/%.json
keys/$(CLUSTER)/%.json:
>@ mkdir -p $(@D)
>@ solana-keygen new --no-bip39-passphrase -s -o $@

# Rebuild the .so if any rust file change
target/deploy/%.so: keys/$(CLUSTER)/%.json $(shell find programs -name "*.rs") $(shell find programs -name "Cargo.toml") Cargo.lock CLUSTER
>@ echo "*******Build $* *******"
>@ CLUSTER=$(CLUSTER) anchor build -p $*
>@ cp -f keys/$(CLUSTER)/$*.json target/deploy/$*-keypair.json #< Optional but just to ensure deploys without the makefile behave correctly 

deploy-scope:
>@ PROGRAM_SO=$(SCOPE_PROGRAM_SO) PROGRAM_KEYPAIR=$(SCOPE_PROGRAM_KEYPAIR) $(MAKE) deploy-int

deploy:
>@ PROGRAM_SO=$(SCOPE_PROGRAM_SO) PROGRAM_KEYPAIR=$(SCOPE_PROGRAM_KEYPAIR) $(MAKE) deploy-int
>@ if [ $(CLUSTER) = "localnet" ]; then\
	   # Deploy fake oracles (mock_oracles, Switchboard V1 and Switchboard V2) only on localnet\
       PROGRAM_SO=$(FAKE_ORACLES_PROGRAM_SO) PROGRAM_KEYPAIR=$(FAKE_ORACLES_PROGRAM_KEYPAIR) $(MAKE) deploy-int;\
   fi

deploy-int: $(PROGRAM_SO) $(PROGRAM_KEYPAIR) $(OWNER_KEYPAIR)
>@ echo "*******Deploy $(PROGRAM_SO) to $(URL)*******"
>@ if [ $(shell uname -s) = "Darwin" ]; then \
      PROGRAM_SIZE=$(shell stat -f '%z' "$(PROGRAM_SO)");\
   else \
      PROGRAM_SIZE=$(shell stat -c%s "$(PROGRAM_SO)"); \
   fi
>@ PROGRAM_SIZE=$$(( PROGRAM_SIZE * 4 ))
>@ echo "Program allocated size: $$PROGRAM_SIZE"
>@ solana program deploy -v \
    -u $(URL) \
    --program-id $(PROGRAM_KEYPAIR) \
    --keypair $(OWNER_KEYPAIR) \
    --upgrade-authority $(OWNER_KEYPAIR) \
    --max-len $$PROGRAM_SIZE \
    $(PROGRAM_SO)

## Listen to on-chain logs
listen:
> solana logs -u $(URL) ${SCOPE_PROGRAM_ID}

test-validator:
> solana-test-validator -r --url mainnet-beta --clone \
                         EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc \
                         CGczF9uYdSVXmSr9swMafhF1ktHsi6ygcgTHWL71XNZ9 \
                         53bbgS6eK2iBL4iKv8C3tzCLwtoidyssCmosV2ESTXAs \
                         --account JAa3gQySiTi8tH3dpkvgztJWHQC1vGXr5m6SQ9LEM55T tests/deps/solustscope.json

print-pubkeys: $(SCOPE_CLI)
>@ ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) get-pubkeys --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

clone-mainnet-to-local-validator: $(SCOPE_CLI)
>@ export ORACLE_PUBKEYS="${shell CLUSTER=mainnet make print-pubkeys}"
> solana-test-validator -r --url mainnet-beta --clone $$ORACLE_PUBKEYS

clone-devnet-to-local-validator:
>@ export ORACLE_PUBKEYS="${shell CLUSTER=devnet make print-pubkeys}"
> solana-test-validator -r --url devnet --clone $$ORACLE_PUBKEYS

test: test-rust test-ts

test-rust:
> cargo test

test-ts: $(SCOPE_CLI)
> yarn run ts-mocha -t 1000000 tests/test_*.ts

# airdrop done this way to stay in devnet limits
airdrop: $(OWNER_KEYPAIR)
>@ if [ $(CLUSTER) = "localnet" ]; then\
      solana airdrop 50 ${PROGRAM_DEPLOY_ACCOUNT} --url $(URL);\
   fi
>@ if [ $(CLUSTER) = "devnet" ]; then\
       for number in `seq 0 10`; do solana airdrop 2 ${PROGRAM_DEPLOY_ACCOUNT} --url $(URL); sleep 10; done;\
   fi
>@ if [ $(CLUSTER) = "mainnet" ] || [ $(CLUSTER) = "mainnet-beta" ]; then\
       echo "No airdrop on mainnet";\
   fi

init: $(SCOPE_CLI)
> RUST_BACKTRACE=1 RUST_LOG="scope_client=trace,scope=trace" ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) init --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

update-mapping: $(SCOPE_CLI)
> RUST_BACKTRACE=1 RUST_LOG="scope_client=trace,scope=trace" ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) upload --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

crank: $(SCOPE_CLI)
> if [ -f ./configs/$(CLUSTER)/$(FEED_NAME).json ]; then\
       ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) crank --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json;\
   else\
       ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) crank;\
   fi

get-prices: $(SCOPE_CLI)
>@ if [ -f ./configs/$(CLUSTER)/$(FEED_NAME).json ]; then\
       ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) show --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json;\
   else\
       ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) show;\
   fi

format:
> prettier --write "./**/*.ts"
> cargo fmt