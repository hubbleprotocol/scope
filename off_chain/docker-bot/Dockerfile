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
RUN mkdir -p keys/localnet
RUN echo "[136,209,142,151,46,179,22,31,181,123,45,32,9,176,77,44,34,235,236,219,127,117,84,100,128,11,119,178,212,10,72,91,24,182,101,20,142,227,3,67,30,101,120,181,199,162,221,168,118,163,228,210,0,51,111,6,30,93,175,34,94,52,16,162]" > keys/localnet/scope.json

WORKDIR /scope/off_chain/scope-cli
RUN cargo install --root /bot --path .

FROM base AS release

COPY --from=build /bot/bin/scope ./
COPY ./off_chain/docker-bot/docker-entrypoint.sh .

ENTRYPOINT ["./docker-entrypoint.sh"]

# use scratch to dump binary from
FROM scratch AS release-bin

COPY --from=release /scope .
