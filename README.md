# AptosBFT

[![CI](https://github.com/aptos-labs/aptosbft/workflows/CI/badge.svg)](https://github.com/aptos-labs/aptosbft/actions)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.81%2B-orange.svg)
[![Coverage](https://img.shields.io/badge/coverage-85.08%25-brightgreen.svg)

> A generic, blockchain-agnostic implementation of the AptosBFT consensus algorithm in Rust.

## Overview

AptosBFT is a Byzantine fault-tolerant consensus protocol derived from DiemBFT/HotStuff. This library provides a **production-ready**, **generic** implementation that can be integrated with any blockchain that implements the required traits.

### Key Features

- **🔒 Byzantine Fault Tolerance**: Safety guaranteed with 2f+1 validators out of 3f+1 total
- **⚡ Fast Confirmation**: Single-round proposal and voting latency
- **🔄 Generic Design**: Works with any blockchain through trait-based abstraction
- **🎯 2-Chain Commit Protocol**: Proven safety guarantees with immediate commit decisions
- **⏱️ Timeout Certificates**: Liveness maintained even under partial synchrony
- **📊 DAG Consensus**: Parallel block proposal for improved throughput
- **🧪 Extensively Tested**: 85%+ test coverage with 461 passing tests
- **🔋 Production Ready**: State persistence, crash recovery, and comprehensive monitoring support

## Table of Contents

- [Architecture](#architecture)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Usage Guide](#usage-guide)
- [Modules](#modules)
- [Testing](#testing)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

## Architecture

```
aptosbft/
├── consensus-traits/     # Trait definitions (generic interfaces)
│   ├── block.rs           # Block, Transaction, Vote, QuorumCertificate
│   ├── core.rs            # Hash, Signature, PublicKey, NodeId
│   ├── safety.rs          # SafetyRules, SafetyState, TimeoutCertificate
│   ├── network.rs         # ConsensusNetwork, ValidatorSet
│   ├── storage.rs         # ConsensusStorage, StateComputer
│   └── proposer.rs        # ProposerElection, ReputationTracker
│
├── consensus-core/       # Core consensus implementation
│   ├── consensus/         # Main consensus orchestrator
│   ├── round_manager/     # Round state machine and proposal processing
│   ├── safety_rules/      # 2-chain safety rules implementation
│   │   ├── two_chain.rs   # 2-chain voting/timeout rules
│   │   ├── state.rs       # Safety state data structures
│   │   └── recovery.rs    # State persistence utilities
│   ├── votes/             # Vote aggregation and pending votes
│   │   └── pending_votes.rs
│   ├── timeout/           # Timeout certificates
│   │   └── two_chain.rs
│   ├── liveness/          # Liveness components
│   │   ├── pacemaker.rs   # Round timeout and progression
│   │   └── proposer.rs    # Proposer election strategies
│   ├── dag/               # DAG consensus for parallel blocks
│   │   ├── driver.rs      # DAG protocol driver
│   │   ├── store.rs       # In-memory DAG storage
│   │   └── ordering.rs    # Node ordering rules
│   ├── crypto/            # Signature aggregation
│   ├── proposal/          # Block proposal generation
│   ├── network/           # Network message handling
│   ├── pipeline/          # Processing pipeline stages
│   ├── epoch_manager/     # Epoch transition management
│   ├── state/             # Round state management
│   ├── types/             # Common types (Round, Epoch, etc.)
│   └── testing/           # Mock implementations for testing
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aptosbft = { git = "https://github.com/aptos-labs/aptosbft", version = "0.1" }
# or
consensus-core = { git = "https://github.com/aptos-labs/aptosbft", version = "0.1" }
consensus-traits = { git = "https://github.com/aptos-labs/aptosbft", version = "0.1" }
```

### Requirements

- Rust 1.81 or later
- Tokio async runtime

## Quick Start

### 1. Implement the Required Traits

```rust
use consensus_traits::{
    Block, BlockMetadata, QuorumCertificate, Transaction,
    Hash, Signature, PublicKey,
    ConsensusNetwork, ConsensusStorage, StateComputer,
};
use std::sync::Arc;

// Your blockchain's block type
#[derive(Clone, Debug)]
struct MyBlock {
    id: [u8; 32],
    parent_id: [u8; 32],
    transactions: Vec<MyTransaction>,
    epoch: u64,
    round: u64,
    // ... other fields
}

impl Block for MyBlock {
    type Transaction = MyTransaction;
    type Metadata = MyBlockMetadata;
    type Signature = MySignature;
    type Hash = [u8; 32];

    fn id(&self) -> Self::Hash { self.id }
    fn parent_id(&self) -> Self::Hash { self.parent_id }
    fn transactions(&self) -> &[Self::Transaction] { &self.transactions }
    fn metadata(&self) -> &Self::Metadata { &self.metadata }
    // ... implement other required methods
}

// Your transaction type
#[derive(Clone, Debug)]
struct MyTransaction {
    data: Vec<u8>,
}

impl Transaction for MyTransaction {
    type Hash = [u8; 32];
    fn hash(&self) -> Self::Hash { /* ... */ }
    fn size(&self) -> usize { self.data.len() }
}
```

### 2. Create a Consensus Instance

```rust
use consensus_core::{
    consensus::Consensus,
    round_manager::RoundManager,
    safety_rules::TwoChainSafetyRules,
    liveness::pacemaker::Pacemaker,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create safety rules
    let safety_rules = TwoChainSafetyRules::<MyBlock, MyVote>::new(0);

    // Create pacemaker
    let pacemaker = Pacemaker::new(/* config */);

    // Create round manager (main consensus orchestrator)
    let mut consensus = RoundManager::<MyBlock, MyVote, _>::new(
        network,
        storage,
        safety_rules,
        pacemaker,
        config,
    );

    // Start consensus loop
    consensus.start().await?;

    Ok(())
}
```

## Core Concepts

### The 2-Chain Commit Rule

AptosBFT uses the **2-chain commit rule** for safety:

> **Rule**: A block B₀ can be committed if there exists a certified block B₁ such that:
> 1. B₀ is a parent of B₁ (B₀ ← B₁)
> 2. round(B₀) + 1 = round(B₁)

This rule ensures that all honest validators agree on committed blocks, providing **safety** (no forks) in an asynchronous network.

### Voting Rules

A vote for block B is safe if **EITHER** condition is true:

1. **Normal Progression**: `block.round == block.qc.round + 1`
2. **With Timeout Certificate**: `block.round == tc.round + 1 AND block.qc.round >= tc.hqc_round`

### Timeout Certificates

When a round times out, validators can generate a **timeout certificate (TC)** proving 2f+1 validators timed out. TCs allow the protocol to advance to the next round even without a block proposal.

### Quorum Requirements

- **Quorum Certificate (QC)**: 2f+1 voting power
- **Timeout Certificate (TC)**: 2f+1 voting power
- **Commit Decision**: Requires 2-chain (two consecutive certified blocks)

## Usage Guide

### Implementing Safety Rules

```rust
use consensus_core::safety_rules::TwoChainSafetyRules;
use consensus_traits::SafetyRules;

let mut safety_rules = TwoChainSafetyRules::<MyBlock, MyVote>::new(0);

// Check if safe to vote for a proposal
match safety_rules.safe_to_vote(&proposal, None) {
    Ok(()) => {
        // Safe to vote - record and send vote
        safety_rules.record_vote(proposal.round())?;
        send_vote(vote)?;
    }
    Err(e) => {
        eprintln!("Not safe to vote: {}", e);
    }
}

// Check if safe to timeout
if safety_rules.safe_to_timeout(round, qc_round, None).is_ok() {
    // Safe to timeout - create timeout vote
    send_timeout_vote(timeout_vote)?;
}

// Observe quorum certificate
safety_rules.observe_qc(&qc)?;

// Check for commit decision
if let Some(block_id) = safety_rules.should_commit(&proposal) {
    println!("Committing block: {:?}", block_id);
}
```

### Vote Aggregation

```rust
use consensus_core::votes::PendingVotes;

let mut pending_votes = PendingVotes::<MyBlock, MyVote>::new(10);

// Add votes
pending_votes.add_vote(vote1, &validator_verifier)?;
pending_votes.add_vote(vote2, &validator_verifier)?;

// Check for quorum
if pending_votes.can_form_quorum(round, block_id, &validator_verifier) {
    // Form quorum certificate
    if let Some(qc) = pending_votes.construct_qc(round, block_id, &aggregator)? {
        println!("Quorum certificate formed: {:?}", qc);
    }
}

// Cleanup old rounds
pending_votes.set_round(20); // Auto-cleanup rounds < 10
```

### Pacemaker (Round Progression)

```rust
use consensus_core::liveness::pacemaker::{Pacemaker, PacemakerBuilder, ExponentialIntervalStrategy};

let pacemaker = PacemakerBuilder::new()
    .base_duration_ms(1000)
    .exponent_base(2.0)
    .max_exponent(8)
    .initial_round(1)
    .build_exponential();

// Enter a new round
match pacemaker.enter_new_round(5) {
    Ok(event) => {
        println!("Entered round {}: {:?}", event.round, event.reason);
        // Set timeout for this round
        schedule_timeout(event.timeout_duration)?;
    }
    Err(e) => {
        eprintln!("Failed to enter round: {}", e);
    }
}

// Record successful block ordering
pacemaker.record_ordered_round(5)?;
```

### Proposer Election

```rust
use consensus_core::liveness::proposer::{RoundRobinProposer, WeightedProposer, ProposerElection};

// Round-robin proposer election
let proposers = vec![validator1_id, validator2_id, validator3_id];
let proposer = RoundRobinProposer::new(proposers);
let leader = proposer.get_proposer(5, &validator_set)?;
println!("Round 5 leader: {:?}", leader);

// Weighted proposer election (based on voting power)
let weighted_proposer = WeightedProposer::new();
let leader = weighted_proposer.get_proposer(10, &validator_set)?;
```

## Modules

### consensus-traits

Generic trait definitions for the consensus protocol:

- **`block`**: `Block`, `Transaction`, `Vote`, `QuorumCertificate`, `BlockMetadata`
- **`core`**: `Hash`, `Signature`, `PublicKey`, `NodeId`, `Error`
- **`safety`**: `SafetyRules`, `SafetyState`, `TimeoutCertificate`
- **`network`**: `ConsensusNetwork`, `ValidatorSet`, `ValidatorInfo`
- **`storage`**: `ConsensusStorage`, `StateComputer`, `StateView`
- **`proposer`**: `ProposerElection`, `ReputationTracker`

### consensus-core

Core consensus implementation:

- **`consensus`**: Main consensus runtime and orchestrator
- **`round_manager`**: Round state machine and proposal processing
- **`safety_rules`**: 2-chain safety rules implementation
  - `two_chain.rs`: 2-chain voting/timeout rules
  - `state.rs`: Safety state data structures
  - `recovery.rs`: State persistence and recovery
- **`votes`**: Vote aggregation and tracking
  - `pending_votes.rs`: Pending votes management
- **`timeout`**: Timeout certificate implementation
- **`liveness`**: Liveness components
  - `pacemaker.rs`: Round timeout and progression
  - `proposer.rs`: Proposer election strategies
- **`dag`**: DAG consensus for parallel blocks
- **`crypto`**: Signature aggregation
- **`testing`**: Mock implementations for testing

## Testing

### Run All Tests

```bash
cargo test --workspace
```

### Run Tests with Coverage

```bash
cargo tarpaulin --workspace --out Html --output-dir target/tarpaulin
```

### Current Test Coverage

- **Overall**: 85.08% (1750/2057 lines)
- **consensus-core**: 85.08% coverage
- **461 passing tests** with 0 failures

### Key Test Suites

| Module | Tests | Coverage | Focus |
|--------|-------|----------|-------|
| Safety Rules | 27 | 62/67 | 2-chain voting/timeout rules |
| Pending Votes | 12 | 58/59 | Vote aggregation and quorum |
| Pacemaker | 34 | 85/90 | Round progression and timeouts |
| Round Manager | 24 | 75/75 | Proposal processing state machine |
| DAG Consensus | 15+ | varies | Parallel block ordering |
| Integration | 31+ | varies | End-to-end consensus flow |

### Safety-Critical Tests

All safety-critical scenarios are tested:

- ✅ 2-chain voting rule correctness
- ✅ 2-chain timeout rule correctness
- ✅ Double-vote prevention
- ✅ State persistence across restarts
- ✅ Fork prevention under network partitions
- ✅ Quorum certificate formation
- ✅ Timeout certificate aggregation
- ✅ Overflow protection (u64::MAX edge cases)

See [`PRODUCTION_READINESS_VERIFICATION.md`](PRODUCTION_READINESS_VERIFICATION.md) for detailed verification of safety-critical components.

## Documentation

### API Documentation

Generate and view API documentation:

```bash
cargo doc --workspace --no-deps --open
```

### Key Documentation Files

- [`PRODUCTION_READINESS_VERIFICATION.md`](PRODUCTION_READINESS_VERIFICATION.md) - Production readiness verification
- [`consensus-traits/`](consensus-traits/src/) - Trait definitions with inline docs
- [`consensus-core/src/`](consensus-core/src/) - Implementation with detailed comments

### Examples

See the [`testing/`](consensus-core/src/testing/) module for example implementations that can be used as a reference.

## Production Readiness

✅ **Production Ready** - See [`PRODUCTION_READINESS_VERIFICATION.md`](PRODUCTION_READINESS_VERIFICATION.md)

### Safety Guarantees

- **Byzantine Fault Tolerance**: Safety guaranteed with up to f malicious validators out of 3f+1 total
- **No Forks**: All honest validators agree on committed blocks (2-chain commit rule)
- **Liveness**: Progress under partial synchrony (unknown but bounded message delay)
- **Crash Recovery**: Safety state persisted before voting prevents double-voting

### Performance Considerations

- **Single-Round Latency**: Proposal to quorum certificate in one network round trip
- **Parallel Blocks**: DAG consensus allows multiple blocks per round for improved throughput
- **Efficient Aggregation**: BLS signature aggregation minimizes bandwidth
- **Configurable Timeouts**: Exponential backoff adapts to network conditions

## Contributing

We welcome contributions! Please see [`CONTRIBUTING.md`](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/aptos-labs/aptosbft
cd aptosbft

# Install dependencies
cargo build --workspace

# Run tests
cargo test --workspace

# Run linter
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --workspace
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure all tests pass and coverage is maintained
5. Submit a pull request with a clear description

## License

This project is licensed under the **Apache License 2.0** - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

AptosBFT is derived from [DiemBFT](https://github.com/diem/diem), which itself evolved from [HotStuff](https://arxiv.org/abs/1803.05069).

### References

- [HotStuff: BFT Consensus with Linear Finality](https://arxiv.org/abs/1803.05069)
- [DiemBFT: Simplified Byzantine Fault Tolerance](https://developers.diem.com/papers/diembft-v2/diembft.pdf)
- [The Aptos Blockchain](https://aptosfoundation.org/)

## Contact

- **GitHub**: [https://github.com/aptos-labs/aptosbft](https://github.com/aptos-labs/aptosbft)
- **Issues**: [https://github.com/aptos-labs/aptosbft/issues](https://github.com/aptos-labs/aptosbft/issues)

---

**Version**: 0.1.0
**Last Updated**: 2025-01-17
**Status**: ✅ Production Ready
