// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! # AptosBFT Consensus Traits
//!
//! This library provides generic trait definitions for the AptosBFT consensus algorithm.
//! These traits allow the consensus protocol to work with any blockchain implementation.
//!
//! ## Overview
//!
//! AptosBFT is a Byzantine fault-tolerant consensus protocol derived from DiemBFT.
//! This crate defines the interfaces that a blockchain must implement to use AptosBFT.
//!
//! ## Core Traits
//!
//! - **Core Types**: [`NodeId`], [`Hash`], [`Signature`], [`PublicKey`]
//! - **Blocks**: [`Block`], [`Transaction`], [`BlockMetadata`]
//! - **Voting**: [`Vote`], [`VoteData`], [`QuorumCertificate`]
//! - **Network**: [`ConsensusNetwork`], [`ValidatorSet`], [`ValidatorInfo`]
//! - **Storage**: [`ConsensusStorage`], [`StateComputer`], [`StateView`]
//!
//! ## Example
//!
//! ```text
//! use consensus_traits::{
//!     core::{Hash, NodeId},
//!     block::{Block, Transaction},
//!     network::{ConsensusNetwork, ValidatorSet},
//!     storage::{StateComputer, ConsensusStorage},
//! };
//!
//! // Define your blockchain's types
//! struct MyBlock { /* ... */ }
//! struct MyTransaction { /* ... */ }
//!
//! // Implement the required traits
//! impl Block for MyBlock {
//!     type Transaction = MyTransaction;
//!     type Hash = [u8; 32];
//!     // ...
//! }
//!
//! // Use with consensus algorithm
//! let consensus = RoundManager::new(network, storage, state_computer);
//! # /*
//! // Note: This is a simplified example. In production, you would need to:
//! // - Implement all required trait methods
//! // - Set up proper networking and storage
//! // - Configure consensus parameters
//! # */
//! ```
//!
//! ## License
//!
//! Licensed under the Apache License, Version 2.0 (LICENSE or http://www.apache.org/licenses/LICENSE-2.0)

pub mod block;
pub mod core;
pub mod dag;
pub mod network;
pub mod proposer;
pub mod safety;
pub mod storage;

// Re-export commonly used traits at the crate root
pub use block::{
    Block, BlockMetadata, CommitInfo, LedgerInfo, QuorumCertificate, Transaction,
    ValidatorVerifier, Vote, VoteData,
};
pub use core::{Hash, NodeId, PublicKey, Signature, VerifyError};
pub use dag::{
    DagNodeId, DagNodeMetadata, DagPayload, DagNode, ParentCertificates,
    NodeCertificate, CertifiedNode, DagVote, NodeStatus, OrderDecision, DagError,
};
pub use network::{ConsensusMessage, ConsensusNetwork, BlockRequest, BlockResponse};
pub use network::{ValidatorInfo, ValidatorSet};
pub use proposer::{ProposerElection, ProposerInfo, ReputationTracker};
pub use safety::{CommitDecision, SafetyRules, SafetyState, TimeoutCertificate};
pub use storage::{ConsensusStorage, StateComputer, StateView};

/// Result type alias for consensus operations.
pub type Result<T> = std::result::Result<T, core::Error>;
