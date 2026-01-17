// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! # DAG Consensus Module
//!
//! This module implements a generic DAG (Directed Acyclic Graph) consensus protocol.
//!
//! ## Overview
//!
//! DAG consensus enables parallel block proposal and improved throughput compared
//! to linear consensus. Instead of a single block per round, validators can propose
//! nodes in parallel, forming a DAG structure.
//!
//! ## Structure
//!
//! - **types**: Core DAG types and implementations
//! - **store**: In-memory DAG storage
//! - **ordering**: Node ordering rules and anchor election
//! - **driver**: Main DAG driver logic

pub mod r#types;
pub mod store;
pub mod ordering;
pub mod driver;
pub mod testing;

// Re-export commonly used types
pub use r#types::{
    DagNodeId,
};

// Re-export DAG traits from consensus-traits
pub use consensus_traits::dag::{
    DagNodeMetadata, DagNode, DagPayload, ParentCertificates,
    NodeCertificate, CertifiedNode, DagVote, NodeStatus,
};

// Re-export store types
pub use store::{InMemDag, StoredNodeStatus, DagStoreConfig};

// Re-export ordering types
pub use ordering::{
    OrderingConfig, OrderRule, AnchorElection,
    RoundRobinAnchorElection, CommitEvent, OrderingResult,
};

// Re-export driver types
pub use driver::{
    DagDriver, DagDriverConfig,
};

// Re-export testing types (available for both lib and integration tests)
pub use testing::{
    TestRound, TestAuthor, TestEpoch,
    TestNodeId, TestMetadata, TestPayload, TestParents,
    TestNode, TestCertificate, TestCertifiedNode,
    TestNodeBuilder,
    create_test_dag, create_test_authors,
};
