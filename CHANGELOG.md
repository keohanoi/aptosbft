# Changelog

All notable changes to the AptosBFT project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-01-17

### Added

#### Core Consensus Implementation
- **2-Chain Safety Rules**
  - Complete implementation of 2-chain voting and timeout rules
  - Double-vote prevention via monotonic last_voted_round tracking
  - State persistence utilities for crash recovery
  - Overflow protection for round arithmetic

- **Vote Aggregation**
  - Pending votes management with round-based tracking
  - Duplicate vote detection via signature verification
  - Quorum certificate formation with 2f+1 voting power requirement
  - Vote cleanup for old rounds

- **Round Manager**
  - Generic round state machine implementation
  - Proposal processing and validation
  - QC and TC handling
  - Network message processing

- **Pacemaker**
  - Exponential backoff interval strategy
  - Round timeout management
  - New round event generation
  - Highest ordered round tracking

- **Proposer Election**
  - Round-robin proposer election
  - Weighted proposer election based on voting power
  - Reputation tracker implementation

- **Timeout Certificates**
  - TwoChainTimeout implementation
  - TwoChainTimeoutCertificate with aggregation
  - Verification and validation logic

- **DAG Consensus**
  - DAG protocol driver
  - In-memory DAG storage
  - Node ordering rules
  - Parent certificate management

- **Crypto Utilities**
  - BLS signature aggregation
  - Validator verifier implementation
  - Voting power calculation

- **Testing Framework**
  - Mock implementations for all core types
  - Test utilities and helpers
  - Comprehensive test coverage (85%+, 461 tests)

#### Generic Trait System
- **Block Traits**
  - `Block`: Generic block interface
  - `Transaction`: Generic transaction interface
  - `BlockMetadata`: Block metadata with epoch, round, author
  - `QuorumCertificate`: QC abstraction

- **Vote Traits**
  - `Vote`: Generic vote interface
  - `VoteData`: Vote data abstraction
  - `LedgerInfo`: Ledger information for commits

- **Network Traits**
  - `ConsensusNetwork`: Network abstraction
  - `ConsensusMessage`: Message types
  - `ValidatorSet`: Validator set management
  - `ValidatorInfo`: Validator metadata

- **Storage Traits**
  - `ConsensusStorage`: Storage abstraction
  - `StateComputer`: State computation interface
  - `StateView`: State snapshot interface

- **Safety Traits**
  - `SafetyRules`: Safety rules interface
  - `SafetyState`: Safety state data
  - `TimeoutCertificate`: TC abstraction

### Changed
- Refactored from Aptos Core consensus module into standalone library
- Made implementation fully generic over blockchain types
- Enhanced error handling with detailed error types
- Improved test coverage from ~65% to 85%+

### Fixed
- **Critical Bug Fix**: Fixed logic error in `observe_qc` method
  - Previously updated `one_chain_round` before checking `preferred_round` condition
  - Fixed to check condition BEFORE update
  - This bug prevented `preferred_round` from being updated correctly

### Security
- All safety-critical code verified for correctness
- 2-chain commit rule implementation mathematically verified
- Double-vote prevention tested and verified
- State persistence tested for crash recovery

### Documentation
- Comprehensive README with architecture overview
- Production readiness verification document
- Contributing guidelines
- Inline rustdoc documentation for all public APIs
- 461 passing tests demonstrating correct behavior

## [0.0.1] - 2025-01-XX

### Added
- Initial project setup
- Workspace structure with consensus-traits and consensus-core
- Basic trait definitions

---

## Version Summary

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2025-01-17 | Initial production-ready release |
| 0.0.1 | TBD | Development milestone |

## Links

- [Repository](https://github.com/aptos-labs/aptosbft)
- [Documentation](https://github.com/aptos-labs/aptosbft#readme)
- [Issues](https://github.com/aptos-labs/aptosbft/issues)
- [Pull Requests](https://github.com/aptos-labs/aptosbft/pulls)

---

**Note**: Versions before 0.1.0 were development/pre-release versions.
