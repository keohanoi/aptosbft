# dYdX AptosBFT Deployment Guide

This directory contains production deployment configurations and scripts for running the dYdX AptosBFT consensus node.

## Directory Structure

```
deploy/
├── Dockerfile              # Production container image
├── docker-compose.yml      # Local testnet with 4 validators
├── deploy.sh              # Deployment automation script
├── prometheus/
│   └── prometheus.yml     # Prometheus monitoring configuration
├── grafana/
│   ├── dashboards/        # Grafana dashboard provisioning
│   └── datasources/       # Grafana datasource configuration
└── README.md             # This file
```

## Quick Start

### Local Testnet (Docker Compose)

Start a 4-validator local testnet:

```bash
# Start the testnet
./deploy/deploy.sh testnet-start

# View logs
./deploy/deploy.sh testnet-logs

# Stop the testnet
./deploy/deploy.sh testnet-stop
```

Access points:
- **Validator 1**: http://localhost:9090/metrics
- **Validator 2**: http://localhost:9091/metrics
- **Validator 3**: http://localhost:9092/metrics
- **Validator 4**: http://localhost:9093/metrics
- **Prometheus**: http://localhost:9094
- **Grafana**: http://localhost:3000 (admin/admin)

### Production Deployment (Docker)

```bash
# Build the image
docker build -f deploy/Dockerfile -t dydxaptosbft:latest .

# Set environment variables
export NODE_NAME=dydx-validator-1
export CHAIN_ID=dydx-mainnet-1
export DATA_DIR=/var/lib/dydxaptosbft
export CONFIG_DIR=/etc/dydxaptosbft

# Run the deployment script
./deploy/deploy.sh deploy

# Check health
./deploy/deploy.sh health
```

### Production Deployment (systemd)

```bash
# Set deployment mode
export DEPLOYMENT_MODE=systemd

# Run as root
sudo ./deploy/deploy.sh deploy

# Check status
sudo systemctl status dydxaptosbft
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NODE_NAME` | `dydx-validator` | Container/service name |
| `CHAIN_ID` | `dydx-mainnet-1` | Blockchain identifier |
| `DATA_DIR` | `/var/lib/dydxaptosbft` | Data storage directory |
| `CONFIG_DIR` | `/etc/dydxaptosbft` | Configuration directory |
| `LOG_DIR` | `/var/log/dydxaptosbft` | Log directory |
| `DEPLOYMENT_MODE` | `docker` | Deployment mode: `docker` or `systemd` |
| `DOCKER_IMAGE` | `dydxaptosbft:latest` | Docker image name |

### Required Configuration Files

Before deploying, ensure you have:

1. **genesis.json** - Genesis file with validator set
   ```bash
   cp examples/genesis.json $CONFIG_DIR/genesis.json
   ```

2. **validator_key.json** - Validator signing key
   ```bash
   # Generate a secure key for production
   # DO NOT use the example key in production!
   cp examples/validator_key.json $CONFIG_DIR/validator_key.json
   ```

## Ports

| Port | Protocol | Description |
|------|----------|-------------|
| 50052 | TCP | gRPC server (consensus communication) |
| 9090 | TCP | Prometheus metrics endpoint |

## Monitoring

### Prometheus Metrics

The node exposes Prometheus metrics at `http://localhost:9090/metrics`:

- `dydx_consensus_state` - Current consensus state
- `dydx_consensus_current_epoch` - Current epoch
- `dydx_consensus_current_round` - Current round
- `dydx_consensus_blocks_proposed_total` - Total blocks proposed
- `dydx_consensus_blocks_committed_total` - Total blocks committed
- `dydx_consensus_peers_connected` - Connected peer count
- `dydx_consensus_mempool_size` - Mempool transaction count

### Health Check

```bash
curl http://localhost:9090/health
```

Response:
```json
{
  "status": "healthy",
  "consensus_state": 1,
  "epoch": 100,
  "round": 5,
  "peers_connected": 3,
  "mempool_size": 150,
  "blocks_proposed": 100,
  "blocks_committed": 99
}
```

## Docker Compose Services

The provided docker-compose.yml includes:

1. **4 Validator Nodes** - Full AptosBFT consensus nodes
2. **Prometheus** - Metrics collection and storage
3. **Grafana** - Metrics visualization and dashboards

### Scaling

To add more validators:

```yaml
  validator5:
    # ... same configuration as other validators
    ports:
      - "50056:50052"
      - "9094:9090"
```

## Systemd Service

The deployment script creates a systemd service at:

```
/etc/systemd/system/dydxaptosbft.service
```

Manage with:

```bash
sudo systemctl start dydxaptosbft
sudo systemctl stop dydxaptosbft
sudo systemctl restart dydxaptosbft
sudo systemctl status dydxaptosbft
sudo journalctl -u dydxaptosbft -f
```

## Security Considerations

### Production Checklist

1. **Generate secure keys** - Never use example keys in production
2. **Use firewall rules** - Restrict gRPC access to known peers
3. **Enable TLS** - Use TLS for gRPC communication in production
4. **Monitor logs** - Set up log aggregation (ELK, Loki, etc.)
5. **Backup data directory** - Regular backups of `$DATA_DIR`
6. **Update regularly** - Keep the binary updated with security patches

### Firewall Example

```bash
# Allow gRPC from known peers
ufw allow from 10.0.0.0/8 to any port 50052 proto tcp

# Allow metrics from localhost
ufw allow from 127.0.0.1 to any port 9090 proto tcp
```

## Troubleshooting

### Container won't start

```bash
# Check logs
docker logs dydx-validator

# Check configuration
docker run --rm -v $CONFIG_DIR:/config dydxaptosbft:latest \
  --genesis /config/genesis.json --help
```

### Out of sync

```bash
# Check current round/epoch
curl http://localhost:9090/health | jq .

# Reset state (WARNING: loses all data)
rm -rf $DATA_DIR/*
./deploy/deploy.sh deploy
```

### High memory usage

The node uses memory for:
- Consensus state
- Mempool
- Network connections

Adjust via cargo build flags or system limits:

```bash
# Set memory limit (Docker)
docker run --memory=4g dydxaptosbft:latest

# Set memory limit (systemd)
# Add to systemd service:
# MemoryLimit=4G
```

## Development

### Building the Docker Image

```bash
docker build -f deploy/Dockerfile -t dydxaptosbft:latest .
```

### Testing with Custom Configuration

```bash
docker run --rm -v $(pwd)/examples:/config:ro \
  dydxaptosbft:latest \
  --genesis /config/genesis.json \
  --validator-key /config/validator_key.json \
  --help
```

## Additional Resources

- [Main README](../README.md) - Project documentation
- [Examples](../examples/README.md) - Example configuration files
- [IBC Headers](../dydx-adapter/src/ibc_headers.rs) - IBC compatibility implementation
