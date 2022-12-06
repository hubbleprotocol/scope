.PHONY: build-docker

build-docker:
	DOCKER_BUILDKIT=1 docker build --ssh default --target runtime ../.. -f Dockerfile -t hubbleprotocol/scope-bot:latest
