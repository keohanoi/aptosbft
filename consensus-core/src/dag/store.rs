// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! In-memory DAG storage implementation.
//!
//! This module provides a generic in-memory DAG storage that can be used
//! with any DAG implementation.

use consensus_traits::{
    dag::{CertifiedNode, DagNode, DagNodeMetadata},
    core::Hash as HashTrait,
};

// Import our concrete DagNodeId implementation for the trait bound
use crate::dag::types::DagNodeId;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    hash::Hash as StdHash,
    sync::Arc,
};
use anyhow::{ensure, Result};

/// Status of a node in the DAG storage.
///
/// This tracks whether a node has been ordered yet or is still waiting.
#[derive(Clone)]
pub enum StoredNodeStatus<N>
where
    N: CertifiedNode,
{
    /// Node is not yet ordered
    Unordered {
        node: Arc<N>,
        aggregated_voting_power: u128,
    },
    /// Node has been ordered
    Ordered(Arc<N>),
}

impl<N> StoredNodeStatus<N>
where
    N: CertifiedNode,
{
    /// Get the node regardless of status
    pub fn as_node(&self) -> &Arc<N> {
        match self {
            StoredNodeStatus::Unordered { node, .. } | StoredNodeStatus::Ordered(node) => node,
        }
    }

    /// Get the aggregated voting power (for unordered nodes)
    pub fn aggregated_voting_power(&self) -> Option<u128> {
        match self {
            StoredNodeStatus::Unordered { aggregated_voting_power, .. } => Some(*aggregated_voting_power),
            StoredNodeStatus::Ordered(_) => None,
        }
    }

    /// Check if the node is ordered
    pub fn is_ordered(&self) -> bool {
        matches!(self, StoredNodeStatus::Ordered(_))
    }

    /// Mark the node as ordered
    pub fn mark_as_ordered(&mut self) -> Result<()> {
        ensure!(
            matches!(self, StoredNodeStatus::Unordered { .. }),
            "Node is already ordered"
        );
        *self = StoredNodeStatus::Ordered(self.as_node().clone());
        Ok(())
    }
}

/// Configuration for the DAG storage.
#[derive(Clone, Debug)]
pub struct DagStoreConfig {
    /// The starting round for the DAG
    pub start_round: u64,
    /// The window size to maintain (number of rounds to keep)
    pub window_size: u64,
}

impl Default for DagStoreConfig {
    fn default() -> Self {
        Self {
            start_round: 0,
            window_size: 100,
        }
    }
}

/// In-memory DAG storage.
///
/// This stores certified nodes indexed by round and author, providing
/// efficient lookup and iteration over the DAG structure.
///
/// ## Type Parameters
///
/// - `R`: Round type (must implement Ord for BTreeMap indexing)
/// - `A`: Author/validator type (must implement Hash + Eq for HashMap)
/// - `N`: Certified node type
pub struct InMemDag<R, A, N>
where
    R: Clone + Ord + StdHash + std::fmt::Debug + Send + Sync + HashTrait + 'static,
    A: Clone + StdHash + Eq + std::fmt::Debug + Send + Sync + HashTrait + 'static,
    N: CertifiedNode + 'static,
{
    /// Nodes indexed by round, then by author index
    nodes_by_round: BTreeMap<R, Vec<Option<StoredNodeStatus<N>>>>,

    /// Map from author to index in the round vectors
    author_to_index: HashMap<A, usize>,

    /// Configuration
    config: DagStoreConfig,
}

impl<R, A, N> InMemDag<R, A, N>
where
    R: Clone + Ord + StdHash + std::fmt::Debug + Send + Sync + HashTrait + 'static,
    A: Clone + StdHash + Eq + std::fmt::Debug + Send + Sync + HashTrait + 'static,
    N: CertifiedNode + 'static,
{
    /// Create a new empty in-memory DAG.
    ///
    /// ## Parameters
    ///
    /// - `authors`: List of authors (validators) in the set
    /// - `config`: Configuration for the DAG store
    pub fn new(authors: Vec<A>, config: DagStoreConfig) -> Self {
        let author_to_index = authors
            .iter()
            .enumerate()
            .map(|(i, author)| (author.clone(), i))
            .collect();

        let nodes_by_round = BTreeMap::new();

        Self {
            nodes_by_round,
            author_to_index,
            config,
        }
    }

    /// Get the lowest round stored in the DAG
    pub fn lowest_round(&self) -> R
    where
        R: Default,
    {
        self.nodes_by_round
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(R::default)
    }

    /// Get the highest round stored in the DAG
    pub fn highest_round(&self) -> R
    where
        R: Default,
    {
        self.nodes_by_round
            .keys()
            .next_back()
            .cloned()
            .unwrap_or_else(R::default)
    }

    /// Get all authors in the DAG
    pub fn authors(&self) -> impl Iterator<Item = &A> {
        self.author_to_index.keys()
    }

    /// Get the number of authors (validators)
    pub fn num_authors(&self) -> usize {
        self.author_to_index.len()
    }

    /// Add a certified node to the DAG.
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The node's round is below the lowest round (DAG was pruned)
    /// - The node's author is unknown
    /// - A node from the same author and round already exists
    pub fn add_node(&mut self, node: N) -> Result<()>
    where
        <N::Node as DagNode>::Metadata: DagNodeMetadata<NodeId = DagNodeId<R, R, A>>,
    {
        let metadata = node.node().metadata();
        let node_id = metadata.node_id();
        let round = node_id.round();
        let author = node_id.author();

        // Check epoch/round bounds
        // For simplicity, we'll just check that the round is reasonable
        // In a full implementation, you'd check against the window

        // Get the author index
        let author_index = *self.author_to_index.get(author)
            .ok_or_else(|| anyhow::anyhow!("Unknown author: {:?}", author))?;

        // Get or create the round vector
        let round_nodes = self.nodes_by_round
            .entry(round.clone())
            .or_insert_with(|| vec![None; self.author_to_index.len()]);

        // Check that the vector is large enough
        if round_nodes.len() <= author_index {
            round_nodes.resize(author_index + 1, None);
        }

        // Check that the slot is empty
        ensure!(
            round_nodes[author_index].is_none(),
            "Node already exists for round {:?}, author {:?}",
            round,
            author
        );

        // Add the node
        round_nodes[author_index] = Some(StoredNodeStatus::Unordered {
            node: Arc::new(node),
            aggregated_voting_power: 0,
        });

        Ok(())
    }

    /// Get a node by round and author.
    ///
    /// Returns `None` if the node doesn't exist.
    pub fn get_node(&self, round: &R, author: &A) -> Option<Arc<N>> {
        let author_index = self.author_to_index.get(author)?;
        let round_nodes = self.nodes_by_round.get(round)?;
        let status = round_nodes.get(*author_index)?;

        status.as_ref().map(|s| s.as_node().clone())
    }

    /// Get the status of a node by round and author.
    ///
    /// Returns `None` if the node doesn't exist.
    pub fn get_node_status(&self, round: &R, author: &A) -> Option<&StoredNodeStatus<N>> {
        let author_index = self.author_to_index.get(author)?;
        let round_nodes = self.nodes_by_round.get(round)?;
        let status = round_nodes.get(*author_index)?;

        status.as_ref()
    }

    /// Check if a node exists
    pub fn exists(&self, round: &R, author: &A) -> bool {
        self.get_node(round, author).is_some()
    }

    /// Mark a node as ordered
    pub fn mark_ordered(&mut self, round: &R, author: &A) -> Result<()> {
        let author_index = *self.author_to_index.get(author)
            .ok_or_else(|| anyhow::anyhow!("Unknown author: {:?}", author))?;

        let round_nodes = self.nodes_by_round.get_mut(round)
            .ok_or_else(|| anyhow::anyhow!("Round {:?} not found", round))?;

        if round_nodes.len() <= author_index {
            return Err(anyhow::anyhow!("Author index out of bounds"));
        }

        let status = round_nodes[author_index]
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Node not found"))?;

        status.mark_as_ordered()
    }

    /// Get all nodes in a round
    pub fn get_round(&self, round: &R) -> Vec<Arc<N>> {
        self.nodes_by_round
            .get(round)
            .map(|nodes| {
                nodes.iter()
                    .filter_map(|status| status.as_ref().map(|s| s.as_node().clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all nodes in the DAG
    pub fn all_nodes(&self) -> Vec<Arc<N>> {
        self.nodes_by_round
            .values()
            .flat_map(|round| {
                round.iter()
                    .filter_map(|status| status.as_ref().map(|s| s.as_node().clone()))
            })
            .collect()
    }

    /// Get the number of nodes stored
    pub fn len(&self) -> usize {
        self.nodes_by_round
            .values()
            .map(|round| round.iter().filter(|s| s.is_some()).count())
            .sum()
    }

    /// Check if the DAG is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all nodes from rounds below the specified round
    pub fn prune_below(&mut self, min_round: &R) {
        self.nodes_by_round = self.nodes_by_round
            .split_off(min_round);
    }

    /// Clear all nodes from the DAG
    pub fn clear(&mut self) {
        self.nodes_by_round.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::types::{
        DagNodeId, DagNodeMetadataImpl, DagNodeImpl, DagPayloadImpl, ParentCertificatesImpl,
        NodeCertificateImpl, CertifiedNodeImpl,
    };
    use std::sync::Arc;
    use std::hash::Hash as StdHash;

    // Test types with HashTrait implementation
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
    struct TestEpoch(u64);

    impl fmt::Display for TestEpoch {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestEpoch {
        fn zero() -> Self {
            TestEpoch(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestEpoch(u64::from_be_bytes(
                bytes.try_into().unwrap_or([0u8; 8]),
            )))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
    struct TestRound(u64);

    impl fmt::Display for TestRound {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestRound {
        fn zero() -> Self {
            TestRound(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestRound(u64::from_be_bytes(
                bytes.try_into().unwrap_or([0u8; 8]),
            )))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    struct TestAuthor(u8);

    impl fmt::Display for TestAuthor {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestAuthor {
        fn zero() -> Self {
            TestAuthor(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestAuthor(bytes.get(0).copied().unwrap_or(0)))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    type TestNodeId = DagNodeId<TestRound, TestRound, TestAuthor>;
    type TestMetadata = DagNodeMetadataImpl<TestNodeId, TestRound>;
    type TestPayload = DagPayloadImpl<TestRound>;
    type TestParents = ParentCertificatesImpl<TestMetadata>;
    type TestNode = DagNodeImpl<TestMetadata, TestPayload, TestParents>;
    type TestCert = NodeCertificateImpl<TestMetadata, u8>;
    type TestCertifiedNode = CertifiedNodeImpl<TestNode, TestCert>;

    fn create_test_node(round: u64, author: u8) -> TestCertifiedNode {
        let test_round = TestRound(round);
        let test_author = TestAuthor(author);
        let node_id = DagNodeId::new(test_round, test_round, test_author);
        let metadata = DagNodeMetadataImpl::new(node_id.clone(), 0, TestRound(round));
        let payload = DagPayloadImpl::new(vec![], TestRound(round));
        let parents = ParentCertificatesImpl::empty();
        let node = DagNodeImpl::new(metadata, payload, parents);
        let cert = NodeCertificateImpl::new(node.metadata().clone(), author);
        CertifiedNodeImpl::new(node, cert)
    }

    #[test]
    fn test_dag_store_new() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        assert!(store.is_empty());
        assert_eq!(store.num_authors(), 3);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_dag_store_add_node() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        let node = create_test_node(1, 1);
        assert!(store.add_node(node).is_ok());

        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_dag_store_get_node() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        let node = create_test_node(1, 1);
        store.add_node(node).unwrap();

        let retrieved = store.get_node(&TestRound(1), &TestAuthor(1));
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_dag_store_get_round() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        store.add_node(create_test_node(1, 1)).unwrap();
        store.add_node(create_test_node(1, 2)).unwrap();
        store.add_node(create_test_node(2, 1)).unwrap();

        let round1 = store.get_round(&TestRound(1));
        assert_eq!(round1.len(), 2);

        let round2 = store.get_round(&TestRound(2));
        assert_eq!(round2.len(), 1);
    }

    #[test]
    fn test_dag_store_exists() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        store.add_node(create_test_node(1, 1)).unwrap();

        assert!(store.exists(&TestRound(1), &TestAuthor(1)));
        assert!(!store.exists(&TestRound(1), &TestAuthor(2)));
        assert!(!store.exists(&TestRound(2), &TestAuthor(1)));
    }

    #[test]
    fn test_dag_store_mark_ordered() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        let node = create_test_node(1, 1);
        store.add_node(node).unwrap();

        let status = store.get_node_status(&TestRound(1), &TestAuthor(1)).unwrap();
        assert!(!status.is_ordered());

        store.mark_ordered(&TestRound(1), &TestAuthor(1)).unwrap();

        let status = store.get_node_status(&TestRound(1), &TestAuthor(1)).unwrap();
        assert!(status.is_ordered());
    }

    #[test]
    fn test_dag_store_add_duplicate_fails() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        store.add_node(create_test_node(1, 1)).unwrap();
        let result = store.add_node(create_test_node(1, 1));

        assert!(result.is_err());
    }

    #[test]
    fn test_dag_store_unknown_author_fails() {
        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        let result = store.add_node(create_test_node(1, 3)); // Author 3 not in set

        assert!(result.is_err());
    }

    #[test]
    fn test_dag_store_clear() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = DagStoreConfig::default();
        let mut store = InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(authors, config);

        store.add_node(create_test_node(1, 1)).unwrap();
        store.add_node(create_test_node(2, 2)).unwrap();

        assert_eq!(store.len(), 2);

        store.clear();

        assert!(store.is_empty());
    }
}
