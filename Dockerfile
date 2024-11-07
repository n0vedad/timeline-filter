# syntax=docker/dockerfile:1.4
FROM rust:1-bookworm AS build

RUN cargo install sqlx-cli@0.8.2 --no-default-features --features sqlite
RUN cargo install sccache --version ^0.8
ENV RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache

RUN USER=root cargo new --bin supercell
RUN mkdir -p /app/
WORKDIR /app/

ARG GIT_HASH
ENV GIT_HASH=$GIT_HASH

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    <<EOF
set -e
cargo build --locked --release --bin supercell --target-dir .
EOF

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source="https://github.com/astrenoxcoop/supercell"
LABEL org.opencontainers.image.description="A lightweight and configurable atproto feed generator."
LABEL org.opencontainers.image.licenses=MIT

RUN set -x \
    && apt-get update \
    && apt-get install ca-certificates -y

RUN groupadd -g 1508 -r supercell && useradd -u 1509 -r -g supercell -d /var/lib/supercell -m  supercell

ENV RUST_LOG=info
ENV RUST_BACKTRACE=full

COPY --from=build /app/release/supercell /var/lib/supercell/

RUN chown -R supercell:supercell /var/lib/supercell

WORKDIR /var/lib/supercell

USER supercell
ENTRYPOINT ["sh", "-c", "/var/lib/supercell/supercell"]
