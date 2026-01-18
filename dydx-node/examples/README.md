# Example Configuration Files

This directory contains example configuration files for running the dYdX AptosBFT consensus node.

## Files

### `genesis.json`

Example genesis file for a 4-validator testnet. This file defines:

- **Chain ID**: `dydx-testnet-1`
- **Initial height**: 1
- **Consensus parameters**:
  - Max block size: 22MB
  - Max block gas: 81.5M
  - Block time: 500ms (sub-second finality!)
- **Validators**: 4 validators with equal voting power (100 each)
- **App state**: Cosmos SDK application state (accounts, governance, staking, etc.)

**IMPORTANT**: This is for **testing only**. For production:
1. Generate secure Ed25519 key pairs for each validator
2. Replace the example addresses and public keys
3. Set appropriate voting power distribution
4. Configure appropriate consensus parameters for your network

### `validator_key.json`

Example validator key file. This file contains:

- **address**: The validator's node ID (hex-encoded, 40 bytes)
- **private_key**: The validator's private signing key (hex-encoded, 64 bytes)
- **pub_key**: The validator's public key (ed25519, base64-encoded)

**SECURITY WARNING**: Never use the example private key in production! Always generate secure random keys.

## Generating Keys

For testing, you can generate random Ed25519 key pairs using the `dydx-adapter` utilities:

```bash
# Generate a new validator key
cargo run --bin keygen -- examples/validator_key.json
```

For production, use a secure hardware security module (HSM) or key management service (KMS).

## Usage

Run the node with the example files:

```bash
cargo run --bin dydxaptosbft -- \
  --genesis examples/genesis.json \
  --validator-key examples/validator_key.json \
  --data-dir /tmp/dydx-testnet \
  --log-level debug
```

## Validator Address Format

Validator addresses are 40-byte hex-encoded strings derived from the Ed25519 public key:

```
SHA256(public_key) -> truncate to 20 bytes -> hex encode
```

Example: `ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD`

## Voting Power

Voting power is an integer representing the validator's stake in the network. Common configurations:

- **Equal power**: All validators have equal voting power (e.g., 100 each)
- **Stake-weighted**: Voting power proportional to token stake
- **Supermajority**: One or few validators with >2/3 power (testing only)

For Byzantine fault tolerance, the sum of honest voting power must exceed 1/3 of total power.
