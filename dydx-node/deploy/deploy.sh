#!/bin/bash
# dYdX AptosBFT Deployment Script
#
# This script deploys the dYdX AptosBFT consensus node to a production environment.
# It supports both Docker and systemd deployment modes.

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
NODE_NAME="${NODE_NAME:-dydx-validator}"
CHAIN_ID="${CHAIN_ID:-dydx-mainnet-1}"
DATA_DIR="${DATA_DIR:-/var/lib/dydxaptosbft}"
CONFIG_DIR="${CONFIG_DIR:-/etc/dydxaptosbft}"
LOG_DIR="${LOG_DIR:-/var/log/dydxaptosbft}"
DOCKER_IMAGE="${DOCKER_IMAGE:-dydxaptosbft:latest}"
DEPLOYMENT_MODE="${DEPLOYMENT_MODE:-docker}"

# Functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

check_dependencies() {
    if [ "$DEPLOYMENT_MODE" = "docker" ]; then
        if ! command -v docker &> /dev/null; then
            log_error "docker is not installed"
            exit 1
        fi
        if ! command -v docker-compose &> /dev/null; then
            log_error "docker-compose is not installed"
            exit 1
        fi
    fi
}

create_directories() {
    log_info "Creating directories..."
    mkdir -p "$DATA_DIR" "$CONFIG_DIR" "$LOG_DIR"
    chown -R dydx:dydx "$DATA_DIR" "$CONFIG_DIR" "$LOG_DIR" 2>/dev/null || true
}

setup_systemd() {
    log_info "Setting up systemd service..."

    cat > /etc/systemd/system/dydxaptosbft.service <<EOF
[Unit]
Description=dYdX AptosBFT Consensus Node
After=network.target

[Service]
Type=simple
User=dydx
Group=dydx
WorkingDirectory=$DATA_DIR
ExecStart=/usr/local/bin/dydxaptosbft \\
    --genesis $CONFIG_DIR/genesis.json \\
    --validator-key $CONFIG_DIR/validator_key.json \\
    --data-dir $DATA_DIR \\
    --log-level info \\
    --metrics-bind-address 0.0.0.0:9090
Restart=always
RestartSec=5
StandardOutput=append:$LOG_DIR/output.log
StandardError=append:$LOG_DIR/error.log

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log_info "Systemd service created: dydxaptosbft.service"
}

deploy_docker() {
    log_info "Deploying with Docker..."
    create_directories

    # Copy configuration files
    if [ -f "$CONFIG_DIR/genesis.json" ]; then
        log_info "Using existing genesis.json"
    else
        log_warn "genesis.json not found, copying example"
        cp examples/genesis.json "$CONFIG_DIR/genesis.json"
    fi

    if [ -f "$CONFIG_DIR/validator_key.json" ]; then
        log_info "Using existing validator_key.json"
    else
        log_error "validator_key.json not found, please generate one"
        exit 1
    fi

    # Run container
    docker run -d \
        --name "$NODE_NAME" \
        --restart unless-stopped \
        -p 50052:50052 \
        -p 9090:9090 \
        -v "$DATA_DIR:/var/lib/dydxaptosbft" \
        -v "$CONFIG_DIR/genesis.json:/etc/dydxaptosbft/genesis.json:ro" \
        -v "$CONFIG_DIR/validator_key.json:/etc/dydxaptosbft/validator_key.json:ro" \
        -e DYDX_GRPC_BIND_ADDRESS=0.0.0.0:50052 \
        -e DYDX_METRICS_BIND_ADDRESS=0.0.0.0:9090 \
        -e DYDX_LOG_LEVEL=info \
        "$DOCKER_IMAGE"

    log_info "Docker container started: $NODE_NAME"
}

deploy_systemd() {
    log_info "Deploying with systemd..."
    check_root
    create_directories
    setup_systemd

    # Build and install binary
    log_info "Building binary..."
    cargo build --release --bin dydxaptosbft
    cp target/release/dydxaptosbft /usr/local/bin/

    # Create user if not exists
    if ! id dydx &>/dev/null; then
        useradd -r -s /bin/false -d $DATA_DIR dydx
    fi

    # Check configuration
    if [ ! -f "$CONFIG_DIR/genesis.json" ]; then
        log_error "genesis.json not found in $CONFIG_DIR"
        exit 1
    fi

    if [ ! -f "$CONFIG_DIR/validator_key.json" ]; then
        log_error "validator_key.json not found in $CONFIG_DIR"
        exit 1
    fi

    # Enable and start service
    systemctl enable dydxaptosbft
    systemctl start dydxaptosbft

    log_info "Systemd service started: dydxaptosbft"
}

start_testnet() {
    log_info "Starting local testnet with docker-compose..."
    docker-compose -f deploy/docker-compose.yml up -d
    log_info "Testnet started"
    log_info "Validator 1: http://localhost:9090/metrics"
    log_info "Validator 2: http://localhost:9091/metrics"
    log_info "Validator 3: http://localhost:9092/metrics"
    log_info "Validator 4: http://localhost:9093/metrics"
    log_info "Prometheus: http://localhost:9094"
    log_info "Grafana: http://localhost:3000 (admin/admin)"
}

stop_testnet() {
    log_info "Stopping local testnet..."
    docker-compose -f deploy/docker-compose.yml down
    log_info "Testnet stopped"
}

# Main script
case "${1:-deploy}" in
    deploy)
        check_dependencies
        if [ "$DEPLOYMENT_MODE" = "docker" ]; then
            deploy_docker
        else
            deploy_systemd
        fi
        ;;
    testnet-start)
        start_testnet
        ;;
    testnet-stop)
        stop_testnet
        ;;
    testnet-logs)
        docker-compose -f deploy/docker-compose.yml logs -f
        ;;
    status)
        if [ "$DEPLOYMENT_MODE" = "docker" ]; then
            docker ps | grep "$NODE_NAME"
            curl -s http://localhost:9090/health | jq .
        else
            systemctl status dydxaptosbft
        fi
        ;;
    health)
        curl -s http://localhost:9090/health | jq .
        ;;
    *)
        echo "Usage: $0 {deploy|testnet-start|testnet-stop|testnet-logs|status|health}"
        echo ""
        echo "Environment variables:"
        echo "  NODE_NAME         - Container/service name (default: dydx-validator)"
        echo "  CHAIN_ID          - Chain ID (default: dydx-mainnet-1)"
        echo "  DATA_DIR          - Data directory (default: /var/lib/dydxaptosbft)"
        echo "  CONFIG_DIR        - Config directory (default: /etc/dydxaptosbft)"
        echo "  DEPLOYMENT_MODE   - Deployment mode: docker|systemd (default: docker)"
        echo "  DOCKER_IMAGE      - Docker image (default: dydxaptosbft:latest)"
        exit 1
        ;;
esac
