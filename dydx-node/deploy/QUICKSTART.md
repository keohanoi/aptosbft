# dYdX AptosBFT Testnet - Quick Start Guide

This guide will help you quickly set up and run a local 4-validator testnet for the dYdX AptosBFT consensus.

## Prerequisites

- Docker Desktop (Mac/Windows) or Docker Engine + Docker Compose (Linux)
- At least 4GB RAM available for Docker
- ~2GB disk space for Docker images

## Quick Start (5 minutes)

```bash
# 1. Navigate to the deploy directory
cd aptosbft/dydx-node/deploy

# 2. Build the Docker images (first time takes 15-30 minutes)
./testnet.sh build

# 3. Start the testnet
./testnet.sh start

# 4. View logs
./testnet.sh logs validator1

# 5. Check status
./testnet.sh status
```

## Verifying Success

Once all validators are running, you should see:

```
[INFO] AptosBFT consensus engine initialized
[INFO] P2P network configured at 0.0.0.0:9001
[INFO] Consensus runtime started
```

### Access Points

| Service | URL | Notes |
|---------|-----|-------|
| Validator 1 gRPC | http://localhost:50052 | Main consensus endpoint |
| Validator 1 P2P | localhost:9001 | Peer-to-peer networking |
| Validator 1 Metrics | http://localhost:9090/metrics | Prometheus metrics |
| Prometheus | http://localhost:9094 | Metrics aggregation |
| Grafana | http://localhost:3000 | Visualization (admin/admin) |

## Testing Consensus

### 1. Check All Validators are Running

```bash
./testnet.sh status
```

Expected output: All 4 validators showing "Up" status.

### 2. Test P2P Connectivity

```bash
./testnet.sh test-connectivity
```

Expected output: All peer connections showing ✓ OK.

### 3. View Validator Logs

```bash
# View validator 1 logs
./testnet.sh logs validator1

# View all validator logs
docker-compose logs -f validator1 validator2 validator3 validator4
```

### 4. Check Metrics

```bash
# Check validator 1 metrics
curl http://localhost:9090/metrics | grep consensus

# Check all validators
for port in 9090 9091 9092 9093; do
    echo "Validator on port $port:"
    curl -s http://localhost:$port/metrics | grep -E "consensus_|aptosbft_" | head -20
    echo ""
done
```

## Common Commands

```bash
# Stop the testnet
./testnet.sh stop

# Restart the testnet
./testnet.sh stop && ./testnet.sh start

# View logs for specific validator
./testnet.sh logs validator2

# Execute command in container
./testnet.sh exec validator1 /bin/sh

# Clean up everything (removes state)
./testnet.sh clean

# Rebuild after code changes
./testnet.sh build validator1
docker-compose up -d validator1
```

## Troubleshooting

### Port Already in Use

```bash
# Check what's using the ports
lsof -i :9001-9004
lsof -i :50052-50055

# Stop the testnet first
./testnet.sh stop
```

### Containers Won't Start

```bash
# Check Docker is running
docker info

# Check for errors
docker-compose logs validator1

# Reset everything
./testnet.sh clean
./testnet.sh build
./testnet.sh start
```

### Low Memory Warning

Docker may warn about low memory. Increase Docker's memory limit:
- Docker Desktop: Settings → Resources → Memory → 4GB+

### Build Failures

```bash
# Clean build
docker-compose down -v
docker system prune -a
./testnet.sh build
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Docker Network                          │
│                     (dydx-testnet)                           │
│                                                               │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐        │
│  │ Validator1 │◄──►│ Validator2 │◄──►│ Validator3 │◄──┐    │
│  │  :9001     │    │  :9002     │    │  :9003     │    │    │
│  └────────────┘    └────────────┘    └────────────┘    │    │
│         ▲                                  │             │    │
│         └──────────────────────────────────┴──────────┘    │
│                          │                                  │
│                   ┌────────────┐                             │
│                   │ Validator4 │                             │
│                   │  :9004     │                             │
│                   └────────────┘                             │
│                                                               │
│  ┌────────────┐    ┌────────────┐                           │
│  │ Prometheus │◄──►│  Grafana   │                           │
│  │  :9094     │    │  :3000     │                           │
│  └────────────┘    └────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

## Next Steps

1. **Test Block Production**: Watch logs to see blocks being proposed and committed
2. **View Metrics**: Open Grafana to see consensus metrics visualized
3. **Test Failure Scenarios**: Stop a validator and watch consensus continue
4. **Performance Testing**: Increase transaction load and measure throughput

## Development Workflow

For active development:

```bash
# 1. Make code changes in aptosbft/dydx-adapter or aptosbft/dydx-node

# 2. Rebuild affected validator(s)
cd aptosbft/dydx-node/deploy
./testnet.sh build validator1

# 3. Restart affected validator(s)
docker-compose up -d --force-recreate validator1

# 4. Watch logs
./testnet.sh logs validator1
```

## Additional Resources

- [Config README](config/README.md) - Detailed configuration information
- [Integration Plan](../../../AptosBFT%20Integration%20with%20dYdX%20v4.md) - Technical design
- [Implementation Progress](../../../aptosbft/IMPLEMENTATION_PROGRESS.md) - Development status
