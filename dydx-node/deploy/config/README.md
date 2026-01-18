# dYdX AptosBFT 4-Validator Testnet Configuration

This directory contains the configuration files for running a local 4-validator testnet of the dYdX AptosBFT consensus node.

## Directory Structure

```
config/
├── genesis.json              # Shared genesis file (master copy)
├── validator1/
│   ├── genesis.json          # Validator 1's copy of genesis
│   └── validator_key.json    # Validator 1's private key
├── validator2/
│   ├── genesis.json          # Validator 2's copy of genesis
│   └── validator_key.json    # Validator 2's private key
├── validator3/
│   ├── genesis.json          # Validator 3's copy of genesis
│   └── validator_key.json    # Validator 3's private key
└── validator4/
    ├── genesis.json          # Validator 4's copy of genesis
    └── validator_key.json    # Validator 4's private key
```

## Validator Information

| Validator | Address (hex) | P2P Port | gRPC Port | Metrics Port |
|-----------|---------------|----------|-----------|--------------|
| validator1 | 0101...01 | 9001 | 50052 | 9090 |
| validator2 | 0202...02 | 9002 | 50053 | 9091 |
| validator3 | 0303...03 | 9003 | 50054 | 9092 |
| validator4 | 0404...04 | 9004 | 50055 | 9093 |

## Security Notice

**⚠️ WARNING**: These are TEST keys ONLY. Never use these keys in production!

- The validator addresses use simple repeated patterns (all 0x01, 0x02, etc.)
- The private keys are deterministic and predictable
- These keys are for local testing purposes only

## Starting the Testnet

From the `deploy` directory:

```bash
# Build the Docker images
docker-compose build

# Start all validators
docker-compose up -d

# View logs
docker-compose logs -f validator1

# Stop the testnet
docker-compose down

# Clean up volumes (reset state)
docker-compose down -v
```

## Verifying Consensus

After starting the testnet, you can verify consensus is working:

```bash
# Check validator 1 logs
docker logs dydx-validator1 -f

# Check metrics endpoints
curl http://localhost:9090/metrics
curl http://localhost:9091/metrics
curl http://localhost:9092/metrics
curl http://localhost:9093/metrics
```

Expected output:
- All validators should log "AptosBFT consensus engine initialized"
- P2P network configuration should be logged
- Validators should start proposing and voting on blocks

## Network Configuration

Each validator is configured to connect to all other validators:

- **validator1** connects to: validator2:9002, validator3:9003, validator4:9004
- **validator2** connects to: validator1:9001, validator3:9003, validator4:9004
- **validator3** connects to: validator1:9001, validator2:9002, validator4:9004
- **validator4** connects to: validator1:9001, validator2:9002, validator3:9003

This creates a fully-connected mesh topology for maximum resilience.

## Prometheus & Grafana

The testnet includes Prometheus and Grafana for metrics visualization:

- **Prometheus**: http://localhost:9094
- **Grafana**: http://localhost:3000 (admin/admin)

## Troubleshooting

### Validators don't start

```bash
# Check if ports are already in use
lsof -i :9001-9004
lsof -i :50052-50055

# Reset state
docker-compose down -v
docker-compose up -d
```

### No blocks being produced

```bash
# Check validator logs for errors
docker-compose logs validator1 | grep -i error

# Verify all validators are running
docker-compose ps
```

### Network connectivity issues

```bash
# Test connectivity between containers
docker exec dydx-validator1 nc -zv validator2 9002
docker exec dydx-validator1 nc -zv validator3 9003
docker exec dydx-validator1 nc -zv validator4 9004
```

## Next Steps

1. ✅ Create genesis configuration
2. ⏳ Run docker-compose testnet
3. ⏳ Verify consensus across validators
4. ⏳ Add network error handling
5. ⏳ Performance testing
