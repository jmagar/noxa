# noxa — Multi-stage Docker build
# Produces 2 binaries: noxa (CLI) and noxa-mcp (MCP server)

# ---------------------------------------------------------------------------
# Stage 1: Build all binaries in release mode
# ---------------------------------------------------------------------------
FROM rust:1.93-bookworm AS builder

# Build dependencies: cmake + clang for BoringSSL (wreq), pkg-config for linking
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy manifests + lock first for better layer caching.
# If only source changes, cargo doesn't re-download deps.
COPY Cargo.toml Cargo.lock ./
COPY crates/noxa-core/Cargo.toml crates/noxa-core/Cargo.toml
COPY crates/noxa-fetch/Cargo.toml crates/noxa-fetch/Cargo.toml
COPY crates/noxa-llm/Cargo.toml crates/noxa-llm/Cargo.toml
COPY crates/noxa-pdf/Cargo.toml crates/noxa-pdf/Cargo.toml
COPY crates/noxa-mcp/Cargo.toml crates/noxa-mcp/Cargo.toml
COPY crates/noxa-cli/Cargo.toml crates/noxa-cli/Cargo.toml

# Copy .cargo config if present (optional build flags)
COPY .cargo .cargo

# Create dummy source files so cargo can resolve deps and cache them.
RUN mkdir -p crates/noxa-core/src && echo "" > crates/noxa-core/src/lib.rs \
    && mkdir -p crates/noxa-fetch/src && echo "" > crates/noxa-fetch/src/lib.rs \
    && mkdir -p crates/noxa-llm/src && echo "" > crates/noxa-llm/src/lib.rs \
    && mkdir -p crates/noxa-pdf/src && echo "" > crates/noxa-pdf/src/lib.rs \
    && mkdir -p crates/noxa-mcp/src && echo "fn main() {}" > crates/noxa-mcp/src/main.rs \
    && mkdir -p crates/noxa-cli/src && echo "fn main() {}" > crates/noxa-cli/src/main.rs

# Pre-build dependencies (this layer is cached until Cargo.toml/lock changes)
RUN cargo build --release 2>/dev/null || true

# Now copy real source and rebuild. Only the final binaries recompile.
COPY crates crates
RUN touch crates/*/src/*.rs \
    && cargo build --release

# ---------------------------------------------------------------------------
# Stage 2: Minimal runtime image
# ---------------------------------------------------------------------------
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy both binaries
COPY --from=builder /build/target/release/noxa /usr/local/bin/noxa
COPY --from=builder /build/target/release/noxa-mcp /usr/local/bin/noxa-mcp

# Default: run the CLI
CMD ["noxa"]
