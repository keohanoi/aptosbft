# dYdX AptosBFT Consensus Node - Production Dockerfile
#
# This Dockerfile builds a minimal production image for running the dYdX AptosBFT
# consensus node. It uses a multi-stage build to keep the final image small.

# Stage 1: Build stage
FROM rust:1.85-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    git \
    protobuf-dev \
    protoc \
    libssl3 \
    libcrypto3

# Set working directory
WORKDIR /build

# Copy entire workspace
COPY . .

# Build the binary in release mode
RUN cargo build --release --bin dydxaptosbft

# Stage 2: Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc

# Create non-root user
RUN addgroup -g 1000 dydx && \
    adduser -D -u 1000 -G dydx dydx

# Set up directories
RUN mkdir -p /var/lib/dydxaptosbft /etc/dydxaptosbft /var/log/dydxaptosbft && \
    chown -R dydx:dydx /var/lib/dydxaptosbft /etc/dydxaptosbft /var/log/dydxaptosbft

# Copy binary from builder
COPY --from=builder /build/target/release/dydxaptosbft /usr/local/bin/

# Copy configuration files (optional - users can mount their own)
COPY dydx-node/examples/genesis.json /etc/dydxaptosbft/genesis.json.example
COPY dydx-node/examples/validator_key.json /etc/dydxaptosbft/validator_key.json.example

# Set ownership
RUN chown -R dydx:dydx /usr/local/bin/dydxaptosbft /etc/dydxaptosbft

# Switch to non-root user
USER dydx

# Expose ports
# 50052: gRPC server for consensus communication
# 9001-9004: P2P network ports for validator communication
# 9090: Prometheus metrics endpoint
EXPOSE 50052 9001 9002 9003 9004 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9090/health || exit 1

# Set default environment variables
ENV DYDX_GRPC_BIND_ADDRESS=0.0.0.0:50052 \
    DYDX_METRICS_BIND_ADDRESS=0.0.0.0:9090 \
    DYDX_DATA_DIR=/var/lib/dydxaptosbft \
    DYDX_LOG_LEVEL=info

# Set working directory
WORKDIR /var/lib/dydxaptosbft

# Default command
ENTRYPOINT ["/usr/local/bin/dydxaptosbft"]

# Default arguments (can be overridden)
CMD ["--genesis", "/etc/dydxaptosbft/genesis.json", \
     "--validator-key", "/etc/dydxaptosbft/validator_key.json", \
     "--data-dir", "/var/lib/dydxaptosbft"]
