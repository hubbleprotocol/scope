FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef

RUN apt-get update \
    && apt-get install -y git unzip build-essential autoconf libtool libssl-dev

# Authorize SSH Host
RUN mkdir -p -m 0600 ~/.ssh && \
    ssh-keyscan github.com >> ~/.ssh/known_hosts

# Allow cargo to fetch with git cli
RUN mkdir -p /root/.cargo \
    && echo -e "[net]\ngit-fetch-with-cli = true" > /root/.cargo/config

WORKDIR scope

FROM chef AS prepare

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build

ENV CLUSTER=mainnet

COPY --from=prepare /scope/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN --mount=type=ssh cargo chef cook --release --bin scope --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release --bin scope --locked

FROM debian:bullseye-slim AS runtime

COPY --from=build /scope/target/release/scope ./
COPY ./off_chain/docker-bot/docker-entrypoint.sh .

ENTRYPOINT ["./docker-entrypoint.sh"]

# use scratch to dump binary from
FROM scratch AS release-bin

COPY --from=runtime /scope .
