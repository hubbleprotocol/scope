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

# TODO: not sure if it really works
ifneq (,$(wildcard ./.env))
	include ./.env
endif

CLUSTER ?= localnet
OWNER_KEYPAIR ?= ./keys/$(CLUSTER)/owner.json

ifeq ($(CLUSTER),localnet)
	URL = "http://127.0.0.1:8899"
endif
ifeq ($(CLUSTER),mainnet)
	URL = "https://twilight-misty-snow.solana-mainnet.quiknode.pro/1080f1a8952de8e09d402f2ce877698f832faea8/"
endif
ifeq ($(CLUSTER),mainnet-beta)
	URL = "https://twilight-misty-snow.solana-mainnet.quiknode.pro/1080f1a8952de8e09d402f2ce877698f832faea8/"
endif
ifeq ($(CLUSTER),devnet)
	URL = "https://wandering-restless-darkness.solana-devnet.quiknode.pro/8eca9fa5ccdf04e4a0f558cdd6420a6805038a1f/"
endif
ifeq ($(URL),)
# URL is still empty, CLUSTER is probably set to an URL directly
# TODO: is this logical?
	URL = $(CLUSTER)
endif

SCOPE_PROGRAM_KEYPAIR := keys/$(CLUSTER)/scope.json
FAKE_PYTH_PROGRAM_KEYPAIR := keys/$(CLUSTER)/pyth.json

SCOPE_PROGRAM_SO := target/deploy/scope.so
FAKE_PYTH_PROGRAM_SO := target/deploy/pyth.so

SCOPE_PROGRAM_ID != solana-keygen pubkey $(SCOPE_PROGRAM_KEYPAIR)
FAKE_PYTH_PROGRAM_ID != solana-keygen pubkey $(FAKE_PYTH_PROGRAM_KEYPAIR)
PROGRAM_DEPLOY_ACCOUNT != solana-keygen pubkey $(OWNER_KEYPAIR)

.PHONY: deploy build-client run listen deploy deploy-int airdrop test test-rust test-ts

build: $(SCOPE_PROGRAM_SO) $(FAKE_PYTH_PROGRAM_SO)

# Don't autodelete the keys, we want to keep them as much as possible 
.PRECIOUS: keys/$(CLUSTER)/%.json
keys/$(CLUSTER)/%.json:
>@ mkdir -p $(@D)
>@ solana-keygen new --no-bip39-passphrase -s -o $@

# Rebuild the .so if any rust file change
target/deploy/%.so: keys/$(CLUSTER)/%.json $(shell find programs -name "*.rs") $(shell find programs -name "Cargo.toml") Cargo.lock
>@ echo "*******Build $* *******"
>@ CLUSTER=$(CLUSTER) anchor build -p $*
#< Optional but just to ensure deploys without the makefile behave correctly 
>@ cp -f keys/$(CLUSTER)/$*.json target/deploy/$*-keypair.json 

deploy:
>@ PROGRAM_SO=$(SCOPE_PROGRAM_SO) PROGRAM_KEYPAIR=$(SCOPE_PROGRAM_KEYPAIR) $(MAKE) deploy-int
>@ PROGRAM_SO=$(FAKE_PYTH_PROGRAM_SO) PROGRAM_KEYPAIR=$(FAKE_PYTH_PROGRAM_KEYPAIR) $(MAKE) deploy-int

deploy-int: $(PROGRAM_SO) $(PROGRAM_KEYPAIR) $(OWNER_KEYPAIR)
>@ echo "*******Deploy $(PROGRAM_SO)*******"
>@ solana program deploy -u $(URL) --upgrade-authority $(OWNER_KEYPAIR) --program-id $(PROGRAM_KEYPAIR) $(PROGRAM_SO)

## Listen to on-chain logs
listen:
> solana logs ${SCOPE_PROGRAM_ID}

test: test-rust test-ts

test-rust:
> cargo test

test-ts:
> yarn run ts-mocha tests/**/*.ts

## Client side
build-client:
> npm run build

run:
> npm run start

airdrop: $(OWNER_KEYPAIR)
> solana airdrop 10 ${PROGRAM_DEPLOY_ACCOUNT} --url http://127.0.0.1:8899