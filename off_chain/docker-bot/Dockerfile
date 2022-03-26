FROM amazonlinux AS base

RUN yum -y update
RUN yum -y install libudev-devel

FROM base AS build

ARG TOOLCHAIN=stable

RUN yum -y install git unzip build-essential autoconf libtool openssl-devel

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $TOOLCHAIN
ENV PATH=$PATH:/root/.cargo/bin

# Allow cargo to fetch with git cli
RUN echo -e "[net]\ngit-fetch-with-cli = true" > /root/.cargo/config

COPY / /scope

# Just simulate a valid scope program
WORKDIR /scope

WORKDIR /scope/off_chain/scope-cli
RUN CLUSTER=mainnet cargo install --root /bot --path . --locked

FROM base AS release

COPY --from=build /bot/bin/scope ./
COPY ./off_chain/docker-bot/docker-entrypoint.sh .

ENTRYPOINT ["./docker-entrypoint.sh"]

# use scratch to dump binary from
FROM scratch AS release-bin

COPY --from=release /scope .
