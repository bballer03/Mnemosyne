# syntax=docker/dockerfile:1.7

# NOTE: Pin base images to specific digests in production.
# Run `docker pull <image>` and `docker inspect --format='{{index .RepoDigests 0}}' <image>`
# to capture the current immutable reference.

# NOTE: Pin to a specific digest in production. Run:
#   docker pull rust:1.85-bookworm && docker inspect --format='{{index .RepoDigests 0}}' rust:1.85-bookworm
FROM rust:1.85-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml ./cli/Cargo.toml
COPY core/Cargo.toml ./core/Cargo.toml

RUN mkdir -p cli/src core/src \
    && printf 'fn main() {}\n' > cli/src/main.rs \
    && printf 'pub fn docker_build_stub() {}\n' > core/src/lib.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p mnemosyne-cli

COPY cli/src ./cli/src
COPY core/src ./core/src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p mnemosyne-cli \
    && cp /app/target/release/mnemosyne-cli /app/mnemosyne-cli

# NOTE: Pin to a specific digest in production. Run:
#   docker pull debian:bookworm-slim && docker inspect --format='{{index .RepoDigests 0}}' debian:bookworm-slim
FROM debian:bookworm-slim AS runtime

ARG VERSION=dev
ARG SOURCE=https://github.com/bballer03/mnemosyne
ARG DESCRIPTION=AI-powered JVM heap analysis tool
ARG LICENSES=Apache-2.0

LABEL org.opencontainers.image.source=$SOURCE \
      org.opencontainers.image.version=$VERSION \
      org.opencontainers.image.description=$DESCRIPTION \
      org.opencontainers.image.licenses=$LICENSES

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system mnemosyne \
    && useradd --system --gid mnemosyne --create-home --shell /usr/sbin/nologin mnemosyne \
    && mkdir -p /data \
    && chown mnemosyne:mnemosyne /data

COPY --from=builder /app/mnemosyne-cli /usr/local/bin/mnemosyne-cli

WORKDIR /data

USER mnemosyne

ENTRYPOINT ["mnemosyne-cli"]
CMD ["--help"]