# syntax=docker/dockerfile:1

ARG RUST_IMAGE=rust:bookworm

# ------------------------------------------------------------------ frontend
FROM ${RUST_IMAGE} AS web-build
ARG NODE_VERSION=24.19.0
ARG TRUNK_VERSION=0.21.14

# wasm32 target + node (tailwind hook) + trunk binary.
# TARGETARCH: amd64 (x64) ou arm64 (aarch64) — Oracle Always Free = ARM.
ARG TARGETARCH
RUN rustup target add wasm32-unknown-unknown \
 && case "$TARGETARCH" in \
      arm64) NODEARCH=arm64; TRUNKARCH=aarch64 ;; \
      *)     NODEARCH=x64;   TRUNKARCH=x86_64  ;; \
    esac \
 && curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODEARCH}.tar.xz" \
      | tar -xJ -C /usr/local --strip-components=1 \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${TRUNKARCH}-unknown-linux-gnu.tar.gz" \
      | tar -xz -C /usr/local/bin \
 && node --version && trunk --version

WORKDIR /build
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/ ./

# trunk self-installs wasm-bindgen-cli + wasm-opt under /root/.cache/trunk.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    --mount=type=cache,target=/root/.cache/trunk \
    trunk build --release

# ------------------------------------------------------------------- server
FROM ${RUST_IMAGE} AS server-build

# reqwest native-tls → openssl-sys needs headers at build time.
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build
# Manifests first for dependency-layer caching (lockfile respected).
COPY Cargo.toml Cargo.lock ./
COPY crates/blindall-api/Cargo.toml       crates/blindall-api/Cargo.toml
COPY crates/blindall-core/Cargo.toml      crates/blindall-core/Cargo.toml
COPY crates/blindall-db/Cargo.toml        crates/blindall-db/Cargo.toml
COPY crates/blindall-game/Cargo.toml      crates/blindall-game/Cargo.toml
COPY crates/blindall-providers/Cargo.toml crates/blindall-providers/Cargo.toml
COPY crates/blindall-realtime/Cargo.toml  crates/blindall-realtime/Cargo.toml
COPY crates/blindall-redis/Cargo.toml     crates/blindall-redis/Cargo.toml
COPY crates/blindall-server/Cargo.toml    crates/blindall-server/Cargo.toml
COPY crates ./crates
# SQL migrations are embedded in the binary via sqlx::migrate! at compile time.
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release -p blindall-server \
 && mkdir -p /out && cp target/release/blindall-server /out/

# ------------------------------------------------------------------ runtime
FROM debian:bookworm-slim AS runtime

# libssl3 for reqwest/native-tls outbound HTTPS (providers, Brevo, Stripe).
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app blindall \
 && mkdir -p /app/storage && chown -R blindall:blindall /app

COPY --from=server-build /out/blindall-server /usr/local/bin/blindall-server
COPY --from=web-build    /build/dist          /app/web/dist

USER blindall
WORKDIR /app

ENV BIND=0.0.0.0:8080 \
    SPA_DIR=/app/web/dist \
    STORAGE_ROOT=/app/storage \
    STORAGE_PUBLIC_PREFIX=/storage \
    RUST_LOG=blindall=info,tower_http=info

EXPOSE 8080
VOLUME ["/app/storage"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD ["curl", "-fsS", "http://127.0.0.1:8080/healthz"]

ENTRYPOINT ["blindall-server"]
