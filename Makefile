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
   SCOPE_PROGRAM_DEPLOY_TARGET := keys/$(CLUSTER)/scope.json
   FAKE_ORACLES_PROGRAM_DEPLOY_TARGET := keys/$(CLUSTER)/mock_oracles.json
   SCOPE_PROGRAM_ID != solana-keygen pubkey $(SCOPE_PROGRAM_DEPLOY_TARGET)
   FAKE_ORACLES_PROGRAM_ID != solana-keygen pubkey $(FAKE_ORACLES_PROGRAM_DEPLOY_TARGET)
   deploy: $(SCOPE_PROGRAM_DEPLOY_TARGET) $(FAKE_ORACLES_PROGRAM_DEPLOY_TARGET) airdrop
endif
ifeq ($(CLUSTER),mainnet)
   SWITCHBOARD_BASE_URL ?= https://switchboard.xyz/explorer/3/
   URL ?= "https://solana-mainnet.rpc.extrnode.com"
   SCOPE_PROGRAM_ID ?= "HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ"
endif
ifeq ($(CLUSTER),devnet)
   SWITCHBOARD_BASE_URL ?= https://switchboard.xyz/explorer/2/
   URL ?= "https://api.devnet.solana.com"
   SCOPE_PROGRAM_ID ?= "3Vw8Ngkh1MVJTPHthmUbmU2XKtFEkjYvJzMqrv2rh9yX"
endif
ifndef URL
   $(error Cluster is set to an unknown value: $(CLUSTER))
endif

SCOPE_PROGRAM_SO := target/deploy/scope.so
FAKE_ORACLES_PROGRAM_SO := target/deploy/mock_oracles.so
SCOPE_CLI := target/debug/scope

PROGRAM_DEPLOY_ACCOUNT != solana-keygen pubkey $(OWNER_KEYPAIR)

PROGRAM_SO ?= $(SCOPE_PROGRAM_SO)
SCOPE_PROGRAM_DEPLOY_TARGET ?= $(SCOPE_PROGRAM_ID)
PROGRAM_DEPLOY_TARGET ?= $(SCOPE_PROGRAM_DEPLOY_TARGET)

.PHONY: deploy run listen deploy deploy-int airdrop test test-rust test-ts init check-env format print-switchboard-links

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
>@ if [ $(CLUSTER) = "mainnet" ]; then\
      PROGRAM_SO=$(SCOPE_PROGRAM_SO) $(MAKE) write-buffer;\
   else\
      CLUSTER=$(CLUSTER) URL=$(URL) PROGRAM_SO=$(SCOPE_PROGRAM_SO) PROGRAM_DEPLOY_TARGET=$(SCOPE_PROGRAM_DEPLOY_TARGET) $(MAKE) deploy-int;\
   fi

deploy:
>@ if [ $(CLUSTER) = "localnet" ]; then\
      PROGRAM_SO=$(FAKE_ORACLES_PROGRAM_SO) PROGRAM_DEPLOY_TARGET=$(FAKE_ORACLES_PROGRAM_DEPLOY_TARGET) $(MAKE) deploy-int;\
   fi
>@ if [ $(CLUSTER) = "mainnet" ]; then\
      PROGRAM_SO=$(SCOPE_PROGRAM_SO) $(MAKE) write-buffer;\
   else\
      CLUSTER=$(CLUSTER) URL=$(URL) $(MAKE) deploy-scope;\
   fi

deploy-int: $(PROGRAM_SO) $(OWNER_KEYPAIR)
>@ if [ $(CLUSTER) = "mainnet" ]; then echo "mainnet shall be behind multisig, use `make write-buffer` instead" && exit 1; fi
>@ echo "*******Deploy $(PROGRAM_SO) to $(URL)*******"
>@ if [ $(shell uname -s) = "Darwin" ]; then \
      PROGRAM_SIZE=$(shell stat -f '%z' "$(PROGRAM_SO)" 2> /dev/null);\
   else \
      PROGRAM_SIZE=$(shell stat -c '%s' "$(PROGRAM_SO)");\
   fi
>@ PROGRAM_SIZE=$$(( PROGRAM_SIZE * 4 ))
>@ echo "Program allocated size: $$PROGRAM_SIZE"
>@ solana program deploy -v \
    -u $(URL) \
    --program-id $(PROGRAM_DEPLOY_TARGET) \
    --keypair $(OWNER_KEYPAIR) \
    --upgrade-authority $(OWNER_KEYPAIR) \
    --max-len $$PROGRAM_SIZE \
    $(PROGRAM_SO)

write-buffer: $(PROGRAM_SO)
>@ echo ""
>@ echo "********************************************************************************"
>@ echo "*******Write $(PROGRAM_SO) to a buffer account*******"
>@ echo "Scope program will be written to a buffer account using the default solana configuration. Please check that the following parameters are correct."
>@ echo "Your keypair must be set with an absolute path"
>@ solana config get
>@ echo "Your current balance is $(shell solana balance)"
>@ echo -n "Do you wish to continue? [y/N] " && read ans && [ $${ans:-N} = y ]
>@ solana program write-buffer $(PROGRAM_SO)

## Listen to on-chain logs
listen:
> solana logs -u $(URL) ${SCOPE_PROGRAM_ID}

test-validator:
> solana-test-validator -r

print-pubkeys: $(SCOPE_CLI)
>@ ./target/debug/scope --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) get-pubkeys --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

clone-mainnet-to-local-validator: $(SCOPE_CLI)
>@ export ORACLE_PUBKEYS="${shell CLUSTER=mainnet make -s print-pubkeys}"
> solana-test-validator -r --url "https://solana-mainnet.rpc.extrnode.com" --clone $$ORACLE_PUBKEYS

clone-devnet-to-local-validator:
>@ export ORACLE_PUBKEYS="${shell CLUSTER=devnet make -s print-pubkeys}"
> solana-test-validator -r --url "https://rpc.ankr.com/solana_devnet" --clone $$ORACLE_PUBKEYS

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
> RUST_BACKTRACE=1 RUST_LOG="scope_client=trace,scope=trace" cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) init --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

update-mapping: $(SCOPE_CLI)
> RUST_BACKTRACE=1 RUST_LOG="scope_client=trace,scope=trace" cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) upload --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json

crank: $(SCOPE_CLI)
> if [ -f ./configs/$(CLUSTER)/$(FEED_NAME).json ]; then\
       cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) --log-timestamps crank --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json;\
   else\
       cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) --log-timestamps crank;\
   fi

get-prices: $(SCOPE_CLI)
>@ if [ -f ./configs/$(CLUSTER)/$(FEED_NAME).json ]; then\
       cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) show --mapping ./configs/$(CLUSTER)/$(FEED_NAME).json;\
   else\
       cargo run -p scope-cli -- --cluster $(URL) --keypair $(OWNER_KEYPAIR) --program-id $(SCOPE_PROGRAM_ID) --price-feed $(FEED_NAME) show;\
   fi

format:
> prettier --write "./**/*.ts"
> cargo fmt

print-switchboard-links:
>@ jq 'to_entries | map(.value) | .[]' ./configs/$(CLUSTER)/$(FEED_NAME).json | tail -n +2 | jq 'select(.oracle_type=="SwitchboardV2")' | jq '.oracle_mapping' | xargs -I % echo "$(SWITCHBOARD_BASE_URL)%"
