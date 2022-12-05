.PHONY: build-docker

build-docker:
	DOCKER_BUILDKIT=1 docker build --ssh default ../.. -f Dockerfile -t hubbleprotocol/scope-bot:latest
