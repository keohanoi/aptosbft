// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// Module-level attributes
#![allow(dead_code)]
#![allow(unused_imports)]

//! DAG testing utilities and helpers.
//!
//! This module provides test fixtures and helpers for testing DAG consensus components.

use consensus_traits::{
    dag::{CertifiedNode, DagNode, DagNodeMetadata, DagNodeId as DagNodeIdTrait, DagPayload, ParentCertificates},
    core::Hash as HashTrait,
};
use std::{
    fmt::{Debug, Display},
    hash::Hash as StdHash,
    sync::Arc,
};
use anyhow::Result;

use crate::dag::types::{
    DagNodeId, DagNodeMetadataImpl, DagPayloadImpl, ParentCertificatesImpl,
    NodeCertificateImpl, CertifiedNodeImpl, DagNodeImpl,
};

/// Test round type with all required trait implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash, Default)]
pub struct TestRound(pub u64);

impl Display for TestRound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HashTrait for TestRound {
    fn zero() -> Self {
        TestRound(0)
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, anyhow::Error> {
        Ok(TestRound(0))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Test author type with all required trait implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
pub struct TestAuthor(pub u8);

impl Display for TestAuthor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HashTrait for TestAuthor {
    fn zero() -> Self {
        TestAuthor(0)
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, anyhow::Error> {
        Ok(TestAuthor(0))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Test epoch type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
pub struct TestEpoch(pub u64);

impl Display for TestEpoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HashTrait for TestEpoch {
    fn zero() -> Self {
        TestEpoch(0)
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, anyhow::Error> {
        Ok(TestEpoch(0))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Test node ID type.
pub type TestNodeId = DagNodeId<TestEpoch, TestRound, TestAuthor>;

/// Test metadata type.
pub type TestMetadata = DagNodeMetadataImpl<TestNodeId, TestRound>;

/// Test payload type.
pub type TestPayload = DagPayloadImpl<TestRound>;

/// Test parents type.
pub type TestParents = ParentCertificatesImpl<TestMetadata>;

/// Test node type.
pub type TestNode = DagNodeImpl<TestMetadata, TestPayload, TestParents>;

/// Test certificate type.
pub type TestCertificate = NodeCertificateImpl<TestMetadata, TestRound>;

/// Test certified node type.
pub type TestCertifiedNode = CertifiedNodeImpl<TestNode, TestCertificate>;

/// Test node builder for creating test nodes.
pub struct TestNodeBuilder {
    epoch: TestEpoch,
    round: TestRound,
    author: TestAuthor,
    timestamp: u64,
}

impl Default for TestNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestNodeBuilder {
    /// Create a new test node builder with default values.
    pub fn new() -> Self {
        Self {
            epoch: TestEpoch(1),
            round: TestRound(1),
            author: TestAuthor(1),
            timestamp: 0,
        }
    }

    /// Set the epoch.
    pub fn epoch(mut self, epoch: u64) -> Self {
        self.epoch = TestEpoch(epoch);
        self
    }

    /// Set the round.
    pub fn round(mut self, round: u64) -> Self {
        self.round = TestRound(round);
        self
    }

    /// Set the author.
    pub fn author(mut self, author: u8) -> Self {
        self.author = TestAuthor(author);
        self
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Build a test certified node.
    pub fn build(self) -> TestCertifiedNode {
        let node_id = DagNodeId::new(self.epoch, self.round, self.author);
        let metadata = DagNodeMetadataImpl::new(node_id, self.timestamp, self.round);
        let payload = DagPayloadImpl::new(vec![], self.round);
        let parents = ParentCertificatesImpl::empty();
        let node = DagNodeImpl::new(metadata, payload, parents);
        let cert = NodeCertificateImpl::new(node.metadata().clone(), self.round);
        CertifiedNodeImpl::new(node, cert)
    }
}

/// Create a simple test DAG with the given number of rounds and authors.
pub fn create_test_dag(rounds: u64, authors: Vec<TestAuthor>) -> Vec<TestCertifiedNode> {
    let mut nodes = Vec::new();

    for round in 1..=rounds {
        for (i, author) in authors.iter().enumerate() {
            let node = TestNodeBuilder::new()
                .epoch(1)
                .round(round)
                .author(author.0)
                .timestamp(round as u64 * 1000 + i as u64)
                .build();
            nodes.push(node);
        }
    }

    nodes
}

/// Create test authors.
pub fn create_test_authors(count: u8) -> Vec<TestAuthor> {
    (1..=count).map(TestAuthor).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_round_all_trait_methods() {
        let round = TestRound(42);

        // Test HashTrait methods
        assert_eq!(TestRound::zero(), TestRound(0));
        assert_eq!(round.as_bytes().len(), 0);
        
        // Test Display
        assert_eq!(format!("{}", round), "42");
        
        // Test Ord and PartialOrd
        assert!(TestRound(10) < TestRound(20));
        assert!(TestRound(10) <= TestRound(10));
    }

    #[test]
    fn test_test_round_default() {
        let round = TestRound::default();
        assert_eq!(round.0, 0);
    }

    #[test]
    fn test_test_author_all_trait_methods() {
        let author = TestAuthor(42);

        // Test HashTrait methods
        assert_eq!(TestAuthor::zero(), TestAuthor(0));
        assert_eq!(author.as_bytes().len(), 0);
        
        // Test Display
        assert_eq!(format!("{}", author), "42");
    }

    #[test]
    fn test_test_epoch_all_trait_methods() {
        let epoch = TestEpoch(42);

        // Test HashTrait methods
        assert_eq!(TestEpoch::zero(), TestEpoch(0));
        assert_eq!(epoch.as_bytes().len(), 0);
        
        // Test Display
        assert_eq!(format!("{}", epoch), "42");
        
        // Test Ord and PartialOrd
        assert!(TestEpoch(10) < TestEpoch(20));
        assert!(TestEpoch(10) <= TestEpoch(10));
    }

    #[test]
    fn test_test_node_builder_chaining() {
        let node = TestNodeBuilder::new()
            .epoch(2)
            .round(15)
            .author(7)
            .timestamp(5000)
            .build();

        let metadata = node.node().metadata();
        let node_id = metadata.node_id();

        assert_eq!(node_id.epoch(), &TestEpoch(2));
        assert_eq!(node_id.round(), &TestRound(15));
        assert_eq!(node_id.author(), &TestAuthor(7));
        assert_eq!(metadata.timestamp(), 5000);
    }

    #[test]
fn test_test_node_builder_default() {
        let node = TestNodeBuilder::default().build();

    let metadata = node.node().metadata();
    let node_id = metadata.node_id();

    assert_eq!(node_id.epoch(), &TestEpoch(1));
    assert_eq!(node_id.round(), &TestRound(1));
    assert_eq!(node_id.author(), &TestAuthor(1));
    assert_eq!(metadata.timestamp(), 0);
}

    #[test]
    fn test_create_test_dag_multiple_rounds() {
        let authors = create_test_authors(2);
        let nodes = create_test_dag(3, authors);

        assert_eq!(nodes.len(), 6); // 3 rounds * 2 authors

        // Verify different rounds
        assert_eq!(nodes[0].node().metadata().node_id().round(), &TestRound(1));
        assert_eq!(nodes[2].node().metadata().node_id().round(), &TestRound(2));
        assert_eq!(nodes[4].node().metadata().node_id().round(), &TestRound(3));
    }

    #[test]
    fn test_create_test_dag_single_author() {
        let authors = create_test_authors(1);
        let nodes = create_test_dag(5, authors);

        assert_eq!(nodes.len(), 5); // 5 rounds * 1 author

        // All nodes should have the same author
        for node in &nodes {
            assert_eq!(node.node().metadata().node_id().author(), &TestAuthor(1));
        }
    }

    #[test]
    fn test_create_test_authors_count() {
        let authors1 = create_test_authors(3);
        assert_eq!(authors1.len(), 3);

        let authors2 = create_test_authors(10);
        assert_eq!(authors2.len(), 10);
    }

    #[test]
    fn test_test_node_id_components() {
        let id = TestNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(7));

        assert_eq!(id.epoch(), &TestEpoch(5));
        assert_eq!(id.round(), &TestRound(10));
        assert_eq!(id.author(), &TestAuthor(7));
    }

    #[test]
    fn test_test_metadata_type() {
        let id = TestNodeId::new(TestEpoch(1), TestRound(5), TestAuthor(3));
        let metadata = DagNodeMetadataImpl::new(id.clone(), 12345, TestRound(5));

        assert_eq!(metadata.node_id(), &id);
        assert_eq!(metadata.timestamp(), 12345);
        assert_eq!(metadata.round(), TestRound(5));
    }

    #[test]
fn test_test_payload_type() {
        let payload = DagPayloadImpl::new(vec![], TestRound(10));

        // The payload has data
        assert_eq!(payload.data().len(), 0);
    }

    #[test]
    fn test_test_parents_type() {
        let parents: TestParents = ParentCertificatesImpl::empty();

        // Test that empty parents is created
        assert!(parents.is_empty());
    }

    #[test]
    fn test_test_node_type() {
        let certified_node = TestNodeBuilder::new().build();

        // Test that we can access the node components through the certified node
        let _metadata = certified_node.node().metadata();
        let _payload = certified_node.node().payload();
        let _parents = certified_node.node().parents();
        let _cert = certified_node.certificate();
    }

    #[test]
    fn test_test_certificate_type() {
        let node = TestNodeBuilder::new()
            .epoch(1)
            .round(5)
            .author(3)
            .build();

        let cert = node.certificate();

        // The certificate should reference the node metadata
        assert_eq!(cert.metadata().node_id().round(), &TestRound(5));
    }

    #[test]
    fn test_test_certified_node_type() {
        // TestNodeBuilder::build() already returns a TestCertifiedNode
        let certified_node = TestNodeBuilder::new()
            .epoch(1)
            .round(5)
            .author(3)
            .build();

        // Test that we can access components
        assert_eq!(certified_node.node().metadata().node_id().round(), &TestRound(5));
        assert_eq!(certified_node.certificate().metadata().node_id().round(), &TestRound(5));
    }

    #[test]
    fn test_test_types_zero_values() {
        // Test zero value for each type
        assert_eq!(TestRound::zero().0, 0);
        assert_eq!(TestAuthor::zero().0, 0);
        assert_eq!(TestEpoch::zero().0, 0);

        // Test that zero values create valid node IDs
        let id1 = TestNodeId::new(TestEpoch::zero(), TestRound::zero(), TestAuthor::zero());
        let id2 = TestNodeId::new(TestEpoch(0), TestRound(0), TestAuthor(0));

        assert_eq!(id1.epoch(), id2.epoch());
        assert_eq!(id1.round(), id2.round());
        assert_eq!(id1.author(), id2.author());
    }

    #[test]
    fn test_create_test_dag_timestamps() {
        let authors = create_test_authors(2);
        let nodes = create_test_dag(2, authors);

        // Verify timestamps increase with both round and position in round
        let node1 = &nodes[0]; // round 1, position 0
        let node2 = &nodes[1]; // round 1, position 1

        let timestamp1 = node1.node().metadata().timestamp();
        let timestamp2 = node2.node().metadata().timestamp();

        // Timestamps should be: round * 1000 + position
        assert_eq!(timestamp1, 1000); // 1 * 1000 + 0
        assert_eq!(timestamp2, 1001); // 1 * 1000 + 1
    }

    #[test]
    fn test_create_test_authors_sequence() {
        let authors = create_test_authors(5);

        // Authors should be numbered 1-5
        assert_eq!(authors[0], TestAuthor(1));
        assert_eq!(authors[1], TestAuthor(2));
        assert_eq!(authors[2], TestAuthor(3));
        assert_eq!(authors[3], TestAuthor(4));
        assert_eq!(authors[4], TestAuthor(5));
    }
}
// Note: Tests are in the test module above (lines 203-439)
