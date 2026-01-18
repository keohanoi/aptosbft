# Multi-stage Dockerfile for dYdX AptosBFT Consensus Node
#
# This Dockerfile builds the AptosBFT consensus node for dYdX v4 in a multi-stage
# build to minimize the final image size. The Rust code is compiled in the builder stage
# and only the binary is copied to the final runtime image.
#
# Usage:
#   docker build -t dydxprotocol/dydx-aptosbft:latest .
#   docker run -p 50052:50052 dydxprotocol/dydx-aptosbft:latest

# =============================================================================
# Stage 1: Builder - Compile AptosBFT binary
# =============================================================================
FROM rust:1.82 as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /aptosbft

# Copy Cargo.toml and Cargo.lock (if exists) first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Build dummy project to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src/main.rs && \
    rm -rf target/release/deps && \
    rm -rf target/release/.fingerprint /target/release/build

# Copy the actual source code
COPY . .

# Build AptosBFT binary in release mode
RUN cargo build --release && \
    strip /aptosbft/target/release/dydxaptosbft || true

# =============================================================================
# Stage 2: Runtime - Minimal Linux image
# =============================================================================
FROM debian:bookworm-slim as runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash aptosbft

# Set working directory
WORKDIR /aptosbft

# Copy compiled binary from builder
COPY --from=builder /aptosbft/target/release/dydxaptosbft /usr/local/bin/dydxaptosbft

# Create data directory
RUN mkdir -p /var/lib/dydxaptosbft && \
    chown -R aptosbft:aptosbft /var/lib/dydxaptosbft

# Switch to non-root user
USER aptosbft

# Expose ports
# 50052: gRPC server port (for AptosBFT bridge)
# 7000: P2P network port (for AptosBFT consensus)
# 9090: Metrics endpoint (optional)
EXPOSE 50052 7000 9090

# Set environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    APTOSBFT_CONFIG=/etc/aptosbft/config.toml \
    APTOSBFT_DATA_DIR=/var/lib/dydxaptosbft

# Health check - check if gRPC server is responding
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:50051/ || exit 1

# Run the AptosBFT node
CMD ["dydxaptosbft"]
