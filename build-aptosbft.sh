#!/bin/bash
# Build script for dYdX AptosBFT Consensus Node
#
# Usage:
#   ./build-aptosbft.sh [clean]
#
# The script builds the AptosBFT node binary in release mode.
# If the "clean" argument is provided, it will clean the build artifacts first.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
APTOSBFT_DIR="$PROJECT_DIR/aptosbft"

# Log function
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
    exit 1
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if Rust is installed
check_rust() {
    if ! command -v rustc &> /dev/null; then
        error "Rust toolchain (rustc) is not installed. Please install Rust from https://rustup.rs/"
    fi

    local rust_version=$(rustc --version 2>/dev/null)
    log "Found Rust version: $rust_version"
}

# Clean build artifacts if requested
clean_build() {
    log "Cleaning build artifacts in $APTOSBFT_DIR..."
    cd "$APTOSBFT_DIR"

    # Clean Rust build artifacts
    cargo clean 2>/dev/null || true

    # Remove target directory
    rm -rf target 2>/dev/null || true

    log "Build artifacts cleaned successfully"
}

# Build the AptosBFT binary
build_aptosbft() {
    log "Building AptosBFT consensus node..."

    cd "$APTOSBFT_DIR"

    # Run cargo build in release mode
    cargo build --release "$@"

    if [ $? -eq 0 ]; then
        log "✓ Build completed successfully!"
        log "Binary location: $APTOSBFT_DIR/target/release/dydxaptosbft"

        # Show binary size
        if [ -f "$APTOSBFT_DIR/target/release/dydxaptosbft" ]; then
            local size=$(du -h "$APTOSBFT_DIR/target/release/dydxaptosbft" | cut -f1)
            log "Binary size: $size"
        fi
    else
        error "Build failed. Run with 'bash -x build-aptosbft.sh' to see detailed errors."
    fi
}

# Run tests
run_tests() {
    log "Running AptosBFT tests..."

    cd "$APTOSBFT_DIR"

    cargo test --workspace "$@"

    if [ $? -eq 0 ]; then
        log "✓ All tests passed!"
    else
        error "Tests failed. Please check the output above."
    fi
}

# Main execution
main() {
    log "dYdX AptosBFT Build Script"
    log "Project directory: $PROJECT_DIR"

    # Check Rust toolchain
    check_rust

    # Parse arguments
    CLEAN_BUILD=false
    RUN_TESTS=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            clean)
                CLEAN_BUILD=true
                shift
                ;;
            test)
                RUN_TESTS=true
                shift
                ;;
            *)
                error "Unknown argument: $1"
                ;;
        esac
    done

    # Clean if requested
    if [ "$CLEAN_BUILD" = true ]; then
        clean_build
    fi

    # Run tests if requested
    if [ "$RUN_TESTS" = true ]; then
        run_tests
    fi

    # Build the binary
    build_aptosbft

    log "Build process completed successfully!"
    log ""
    log "Next steps:"
    log "  1. Test the binary: ./aptosbft/target/release/dydxaptosbft --help"
    log "  2. Run with Docker: docker-compose -f docker/docker-compose.aptosbft.yml up"
    log "  3. Run locally: RUST_LOG=debug ./aptosbft/target/release/dydxaptosbft"
}

# Run main
main "$@"
