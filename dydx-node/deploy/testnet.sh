#!/bin/bash
# dYdX AptosBFT Testnet Management Script
#
# This script helps manage the local 4-validator testnet

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Print colored output
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
}

# Build the Docker images
build() {
    info "Building Docker images..."
    docker-compose build "$@"
    info "Build complete!"
}

# Start the testnet
start() {
    check_docker
    info "Starting 4-validator testnet..."
    docker-compose up -d
    info "Testnet started!"
    info ""
    info "Services:"
    echo "  - Validator 1: http://localhost:50052 (gRPC), http://localhost:9090 (metrics)"
    echo "  - Validator 2: http://localhost:50053 (gRPC), http://localhost:9091 (metrics)"
    echo "  - Validator 3: http://localhost:50054 (gRPC), http://localhost:9092 (metrics)"
    echo "  - Validator 4: http://localhost:50055 (gRPC), http://localhost:9093 (metrics)"
    echo "  - Prometheus:  http://localhost:9094"
    echo "  - Grafana:     http://localhost:3000 (admin/admin)"
}

# Stop the testnet
stop() {
    info "Stopping testnet..."
    docker-compose stop
    info "Testnet stopped!"
}

# Stop and remove all containers and volumes
clean() {
    warn "This will remove all containers and volumes. State will be lost!"
    read -p "Are you sure? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        info "Cleaning up..."
        docker-compose down -v
        info "Cleanup complete!"
    else
        info "Cleanup cancelled."
    fi
}

# View logs
logs() {
    local service="${1:-validator1}"
    info "Showing logs for $service..."
    docker-compose logs -f "$service"
}

# Show status
status() {
    info "Testnet status:"
    docker-compose ps
}

# Execute command in a validator container
exec_validator() {
    local validator="$1"
    shift
    info "Executing in $validator: $@"
    docker-compose exec "$validator" "$@"
}

# Test connectivity between validators
test_connectivity() {
    info "Testing P2P connectivity between validators..."
    echo ""

    for i in 1 2 3 4; do
        for j in 1 2 3 4; do
            if [ $i -ne $j ]; then
                echo -n "Testing validator$i -> validator$j:900$j ... "
                if docker-compose exec -T validator$i nc -zv validator$j 900$j 2>&1 | grep -q "succeeded"; then
                    echo -e "${GREEN}✓ OK${NC}"
                else
                    echo -e "${RED}✗ FAILED${NC}"
                fi
            fi
        done
    done
}

# Show usage
usage() {
    cat << EOF
Usage: $0 <command> [options]

Commands:
    build [service]    Build Docker images (all services or specific one)
    start              Start the testnet
    stop               Stop the testnet
    clean              Stop and remove all containers and volumes
    logs [service]     Show logs for a service (default: validator1)
    status             Show status of all services
    exec <validator> <cmd>  Execute command in a validator container
    test-connectivity  Test P2P connectivity between validators
    help               Show this help message

Examples:
    $0 build                    # Build all images
    $0 build validator1         # Build only validator1
    $0 start                    # Start the testnet
    $0 logs validator1          # Show validator1 logs
    $0 exec validator1 sh       # Open shell in validator1
    $0 test-connectivity        # Test network connectivity

For more information, see config/README.md
EOF
}

# Main script logic
case "${1:-help}" in
    build)
        build "${@:2}"
        ;;
    start)
        start
        ;;
    stop)
        stop
        ;;
    clean)
        clean
        ;;
    logs)
        logs "${2:-validator1}"
        ;;
    status)
        status
        ;;
    exec)
        if [ -z "${2:-}" ]; then
            error "Please specify a validator (e.g., validator1)"
            exit 1
        fi
        exec_validator "$2" "${@:3}"
        ;;
    test-connectivity)
        test_connectivity
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        error "Unknown command: $1"
        usage
        exit 1
        ;;
esac
