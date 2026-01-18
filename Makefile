# Makefile for dYdX AptosBFT Consensus Node
#
# This Makefile provides convenient targets for building, testing, and running
# the AptosBFT consensus node for dYdX v4.

.PHONY: all build test clean lint docker-build docker-up docker-down docker-logs help

# Default target
all: build

# Binary name
BINARY_NAME=dydxaptosbft
BINARY_PATH=target/release/$(BINARY_NAME)

# Rust compiler flags
RUSTFLAGS?= -C opt-level=3

# Cargo flags
CARGO_FLAGS?=--release

# =============================================================================
# Build Targets
# =============================================================================

build: test
	@echo "Building AptosBFT consensus node..."
	cargo build $(CARGO_FLAGS)
	@echo "✓ Build completed: $(BINARY_PATH)"

build-linux:
	@echo "Building AptosBFT consensus node (Linux target)..."
	cargo build $(CARGO_FLAGS) --target x86_64-unknown-linux-gnu
	@echo "✓ Build completed: $(BINARY_PATH)"

# =============================================================================
# Test Targets
# =============================================================================

test:
	@echo "Running AptosBFT tests..."
	cargo test --workspace

test-unit:
	@echo "Running unit tests only..."
	cargo test --lib --bins

test-integration:
	@echo "Running integration tests..."
	cargo test --test '*integration*'

# =============================================================================
# Linting Targets
# =============================================================================

lint:
	@echo "Running Rust linter..."
	cargo clippy --all-targets --all-features

lint-fix:
	@echo "Running Rust linter with auto-fix..."
	cargo clippy --all-targets --all-features --fix

format:
	@echo "Formatting Rust code..."
	cargo fmt

format-check:
	@echo "Checking Rust code formatting..."
	cargo fmt --check

# =============================================================================
# Clean Targets
# =============================================================================

clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@echo "✓ Clean completed"

clean-all: clean
	@echo "Cleaning all artifacts..."
	cargo clean && \
	rm -rf target/ Dockerfile.target && \
	rm -rf Dockerfile.* && \
	rm -rf docker-compose.override.yml && \
	cargo clean
	@echo "✓ Full clean completed"

# =============================================================================
# Docker Targets
# =============================================================================

docker-build:
	@echo "Building AptosBFT Docker image..."
	docker build -t dydxprotocol/dydx-aptosbft:latest .
	@echo "✓ Docker image built successfully"

docker-up:
	@echo "Starting AptosBFT development environment..."
	docker-compose -f docker/docker-compose.aptosbft.yml up -d
	@echo "✓ Development environment started"
	@echo "Logs: docker-compose -f docker/docker-compose.aptosbft.yml logs -f"

docker-down:
	@echo "Stopping AptosBFT development environment..."
	docker-compose -f docker/docker-compose.aptosbft.yml down
	@echo "✓ Development environment stopped"

docker-restart:
	@echo "Restarting AptosBFT development environment..."
	docker-compose -f docker/docker-compose.aptosbft.yml restart
	@echo "✓ Development environment restarted"

docker-logs:
	@echo "Showing AptosBFT logs..."
	docker-compose -f docker/docker-compose.aptosbft.yml logs -f

docker-ps:
	@echo "Showing AptosBFT containers..."
	docker-compose -f docker/docker-compose.aptosbft.yml ps

# =============================================================================
# Utility Targets
# =============================================================================

help:
	@echo "dYdX AptosBFT Consensus Node - Build System"
	@echo ""
	@echo "Build Targets:"
	@echo "  make              - Build AptosBFT binary"
	@echo "  make build-linux    - Build for Linux target"
	@echo ""
	@echo "Test Targets:"
	@echo "  make test           - Run all tests"
	@echo "  make test-unit      - Run unit tests only"
	@echo "  make test-integration - Run integration tests"
	@echo ""
	@echo "Lint Targets:"
	@echo "  make lint           - Run linter (check code quality)"
	@echo "  make lint-fix        - Run linter with auto-fix"
	@echo "  make format         - Format code"
	@echo "  make format-check    - Check code formatting"
	@echo ""
	@echo "Clean Targets:"
	@	@echo "  make clean           - Clean build artifacts"
	@	@echo "  make clean-all       - Clean all artifacts including Docker"
	@	@echo ""
	@echo "Docker Targets:"
	@echo "  make docker-build     - Build Docker image"
	@echo "  make docker-up        - Start development environment"
	@echo "  make docker-down      - Stop development environment"
	@echo "  make docker-restart   - Restart development environment"
	@echo "  make docker-logs      - Show logs from all services"
	@echo "  make docker-ps        - Show status of all containers"
	@echo ""
	@echo "Utility Targets:"
	@echo "  make help            - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make build && make test        # Build and run tests"
	@echo "  make docker-build && make docker-up  # Build and start containers"
	@echo "  make lint-fix && make format       # Fix lint issues and format code"
	@echo ""

# =============================================================================
# Installation Targets
# =============================================================================

install: build
	@echo "Installing AptosBFT binary to /usr/local/bin..."
	sudo cp -f $(BINARY_PATH) /usr/local/bin/
	@echo "✓ Installed $(BINARY_NAME) to /usr/local/bin"

uninstall:
	@echo "Uninstalling AptosBFT binary from /usr/local/bin..."
	sudo rm -f /usr/local/bin/$(BINARY_NAME)
	@echo "✓ Uninstalled $(BINARY_NAME) from /usr/local/bin"

# =============================================================================
# Development Targets
# =============================================================================

dev:
	@echo "Starting AptosBFT development environment..."
	@echo "Press Ctrl+C to stop"
	docker-compose -f docker/docker-compose.aptosbft.yml up
	docker-compose -f docker/docker-compose.aptosbft.yml logs -f

dev-logs:
	@echo "Showing logs from AptosBFT development environment..."
	docker-compose -f docker/docker-compose.aptosbft.yml logs -f aptosbft

dev-test:
	@echo "Running tests in development environment..."
	docker-compose -f docker/docker-compose.aptosbft.yml exec aptosbft cargo test

# =============================================================================
# CI/CD Targets
# =============================================================================

ci: lint format-check test
	@echo "Running CI pipeline..."
	$(MAKE) lint-check
	@echo "  ✓ Code formatting check passed"
	@echo ""
	@echo "  Running tests..."
	@echo "  ✓ All tests passed"
	@echo ""
	@echo "All validation checks passed!"

ci-docker: docker-build docker-test
	@echo "Running Docker CI pipeline..."
	$(MAKE) docker-build
	@echo "  ✓ Docker image built"
	@echo "  Running tests in Docker..."
	docker-compose -f docker/docker-compose.aptosbft.yml up -d
	docker-compose -f docker/docker-compose.aptosbft.yml exec aptosbft cargo test
	docker-compose -f docker/docker-compose.aptosbft.yml down
	@echo "  ✓ Tests passed"
	@echo ""
	@echo "✓ Docker CI pipeline passed"

docker-test:
	@echo "Running tests in Docker..."
	docker-compose -f docker/docker-compose.aptosbft.yml up -d
	docker-compose -f docker/docker-compose.aptosbft.yml exec aptosbft cargo test
	docker-compose -f docker/docker-compose.aptosbft.yml down

# =============================================================================
# Release Targets
# =============================================================================

release: clean build-linux docker-build
	@echo "Creating release artifacts..."
	@echo "✓ Binary built: $(BINARY_PATH)"
	@echo "✓ Docker image created: dydxprotocol/dydx-aptosbft:latest"
	@echo ""
	@echo "To tag and push:"
	@echo "  docker tag dydxprotocol/dydx-aptosbft:latest dydxprotocol/dydx-aptosbft:vX.Y.Z"
	@echo "  docker push dydxprotocol/dydx-aptosbft:latest"
	@echo "  docker push dydxprotocol/dydx-aptosbft:vX.Y.Z"

# =============================================================================
# Validation Targets
# =============================================================================

check: format-check test
	@echo "Running all validation checks..."
	$(MAKE) format-check
	@echo "  ✓ Code formatting check passed"
	@echo ""
	@echo "  Running tests..."
	$(MAKE) test
	@echo "  ✓ All tests passed"
	@echo ""
	@echo "All validation checks passed!"

# =============================================================================
# Information Targets
# =============================================================================

info:
	@echo "AptosBFT Build System Information"
	@echo ""
	@echo "Rust Version:"
	rustc --version || echo "  rustc not found"
	@echo ""
	@echo "Cargo Version:"
	cargo --version || echo "  cargo not found"
	@echo ""
	@echo "Binary Location:"
	@echo "  $(BINARY_PATH)"
	@	@ls -lh $(BINARY_PATH) 2>/dev/null || echo "  (binary not yet built)"
	@echo ""
	@echo "Build Information:"
	@echo "  Profile: release"
	@echo "  Optimizations: opt-level=3"
	@echo "  Codegen-units: 1"
	@echo ""
	@echo "Docker Image:"
	@echo "  Name: dydxprotocol/dydx-aptosbft:latest"
	@echo "  Size: $(shell docker images dydxprotocol/dydx-aptosbft:latest --format '{{.Size}}' 2>/dev/null || echo "  (image not yet built)"
	@echo ""
	@echo "Configuration:"
	@echo "  Config File: aptosbft/config/aptosbft.toml.example"
	@echo "  Data Dir: /var/lib/dydxaptosbft"
	@echo "  Ports: 50052 (gRPC), 7000 (P2P), 9090 (metrics)"

show-deps:
	@echo "Showing Rust dependency tree..."
	cargo tree

# =============================================================================
# Misc
# ==============================================================================

.DEFAULT_GOAL := build

.PHONY: check
check: format-check test
