// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// Module-level attributes come first
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

/// DAG consensus driver.
///
/// This module implements the main driver for DAG-based consensus.
/// It coordinates node creation, broadcast, and ordering.

use consensus_traits::{
    dag::{CertifiedNode, DagNode, DagNodeMetadata, DagNodeId as DagNodeIdTrait, NodeStatus},
    core::Hash as HashTrait,
};
use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
};
use anyhow::Result;

use crate::dag::{
    store::{InMemDag, StoredNodeStatus},
    ordering::{OrderRule, AnchorElection, OrderingConfig},
};

/// Configuration for the DAG driver.
#[derive(Clone, Debug)]
pub struct DagDriverConfig {
    /// Window size for DAG retention
    pub window_size: u64,
    /// Anchor round interval (every N rounds)
    pub anchor_interval: u64,
    /// Maximum number of concurrent rounds
    pub max_concurrent_rounds: usize,
}

impl Default for DagDriverConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            anchor_interval: 2,
            max_concurrent_rounds: 10,
        }
    }
}

/// DAG consensus driver.
///
/// This is the main coordinator for DAG consensus, responsible for:
/// - Managing the DAG store
/// - Processing incoming certified nodes
/// - Coordinating with ordering rules
/// - Tracking round state
///
/// ## Type Parameters
///
/// - `R`: Round type
/// - `A`: Author/validator type
/// - `N`: Certified node type
/// - `AE`: Anchor election strategy
pub struct DagDriver<R, A, N, AE>
where
    R: Clone + Ord + fmt::Debug + Send + Sync + HashTrait + Default + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
    N: CertifiedNode + 'static,
    N::Node: Clone + 'static,
    AE: AnchorElection<Round = R, Author = A>,
{
    /// In-memory DAG storage
    dag: Arc<InMemDag<R, A, N>>,

    /// Ordering rule engine
    order_rule: Arc<OrderRule<R, A, N, AE>>,

    /// Configuration
    config: DagDriverConfig,

    /// Current round
    current_round: R,

    /// Pending nodes to be processed
    pending_nodes: VecDeque<Arc<N>>,

    /// Whether we're currently processing
    is_processing: bool,
}

impl<R, A, N, AE> DagDriver<R, A, N, AE>
where
    R: Clone + Ord + fmt::Debug + Send + Sync + HashTrait + Default + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
    N: CertifiedNode + 'static,
    N::Node: Clone + 'static,
    AE: AnchorElection<Round = R, Author = A>,
{
    /// Create a new DAG driver.
    ///
    /// ## Parameters
    ///
    /// - `authors`: List of validators
    /// - `anchor_election`: Anchor election strategy
    /// - `start_round`: Starting round number
    /// - `config`: Driver configuration
    pub fn new(
        authors: Vec<A>,
        anchor_election: Arc<AE>,
        start_round: R,
        config: DagDriverConfig,
    ) -> Self {
        let dag_store_config = crate::dag::store::DagStoreConfig {
            start_round: 0, // Will be determined by start_round
            window_size: config.window_size,
        };

        let dag = Arc::new(InMemDag::new(authors, dag_store_config));
        let ordering_config = OrderingConfig {
            window_size: config.window_size,
            anchor_interval: config.anchor_interval,
        };

        let order_rule = Arc::new(OrderRule::new(
            dag.clone(),
            anchor_election,
            start_round.clone(),
            ordering_config,
        ));

        Self {
            dag,
            order_rule,
            config,
            current_round: start_round,
            pending_nodes: VecDeque::new(),
            is_processing: false,
        }
    }

    /// Get the current round.
    pub fn current_round(&self) -> &R {
        &self.current_round
    }

    /// Get the DAG store.
    pub fn dag(&self) -> &Arc<InMemDag<R, A, N>> {
        &self.dag
    }

    /// Get the ordering rule engine.
    pub fn order_rule(&self) -> &Arc<OrderRule<R, A, N, AE>> {
        &self.order_rule
    }

    /// Process a certified node from another validator.
    ///
    /// This is the main entry point for receiving nodes from the network.
    /// The node will be added to the DAG and trigger ordering if appropriate.
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The node's round is below the lowest round (DAG was pruned)
    /// - The node's author is unknown
    /// - The node already exists
    /// - The node's parents are missing
    ///
    /// NOTE: This is a simplified implementation. The type system complexity
    /// makes it difficult to fully validate nodes without more specific type
    /// constraints. In production, you'd need either:
    /// 1. More specific type bounds to ensure R/A match the node's types
    /// 2. Interior mutability (Arc<Mutex<InMemDag>>) for runtime checks
    pub fn process_certified_node(&mut self, _node: N) -> Result<()> {
        // In production, we would:
        // 1. Extract round and author from node metadata
        // 2. Check if the node already exists
        // 3. Add the node to the DAG
        // 4. Trigger ordering check

        // For now, just trigger ordering check
        self.check_ordering()?;

        Ok(())
    }

    /// Check if any nodes can be ordered and process them.
    ///
    /// This method should be called after adding new nodes or when
    /// explicitly triggering ordering.
    fn check_ordering(&mut self) -> Result<()> {
        let mut ordered_results = Vec::new();

        // Process all pending ordering
        loop {
            let results = {
                // Get a reference to the order rule without using Arc::make_mut
                // We'll just use the reference directly
                let order_rule = &self.order_rule;
                // Note: This is a simplified version that doesn't actually mutate
                // In production, you'd need interior mutability (Mutex/RwLock)
                vec![] // Placeholder
            };

            if results.is_empty() {
                break;
            }

            ordered_results.extend(results);
        }

        // Process ordered results
        for result in ordered_results {
            self.handle_ordered_nodes(result)?;
        }

        Ok(())
    }

    /// Handle a batch of ordered nodes.
    ///
    /// This is called when the ordering rule successfully orders nodes.
    fn handle_ordered_nodes(&mut self, _result: crate::dag::ordering::OrderingResult<N, R, A>) -> Result<()> {
        // In production, this would:
        // 1. Notify the application layer of committed nodes
        // 2. Update the round state
        // 3. Trigger pruning if needed
        // 4. Check if we can advance to a new round

        // For now, just acknowledge the ordering
        Ok(())
    }

    /// Enter a new round.
    ///
    /// This is called when the driver is ready to start proposing in a new round.
    pub fn enter_new_round(&mut self, new_round: R) -> Result<()> {
        // Check that we're not moving backwards
        if new_round < self.current_round {
            anyhow::bail!("Cannot move to round {:?} from {:?}", new_round, self.current_round);
        }

        self.current_round = new_round;

        // In production, this would:
        // 1. Get strong links from the previous round
        // 2. Create a new proposal node
        // 3. Broadcast the node

        Ok(())
    }

    /// Get strong links for a round.
    ///
    /// Strong links are certified nodes from the previous round that have
    /// sufficient votes (typically 2f+1 voting power).
    ///
    /// This is a simplified placeholder - in production, this would query
    /// the DAG for nodes with sufficient votes.
    pub fn get_strong_links(&self, round: &R) -> Vec<Arc<N>> {
        // Simplified: return all nodes from the round
        self.dag.get_round(round)
    }

    /// Create a new proposal node.
    ///
    /// This is a placeholder - in production, this would:
    /// 1. Get strong links from previous round
    /// 2. Pull payload from mempool
    /// 3. Create and sign a new node
    pub fn create_proposal(&self) -> Result<()> {
        // Simplified: do nothing
        // In production, this would create a new node proposal
        Ok(())
    }

    /// Process pending nodes.
    ///
    /// This processes any nodes that are waiting to be handled.
    pub fn process_pending(&mut self) -> Result<()> {
        if self.is_processing {
            return Ok(());
        }

        self.is_processing = true;

        while let Some(node) = self.pending_nodes.pop_front() {
            if let Err(e) = self.process_certified_node((*node).clone()) {
                eprintln!("Error processing pending node: {}", e);
            }
        }

        self.is_processing = false;
        Ok(())
    }

    /// Add a node to the pending queue.
    pub fn add_pending(&mut self, node: Arc<N>) {
        self.pending_nodes.push_back(node);
    }

    /// Get the number of pending nodes.
    pub fn pending_count(&self) -> usize {
        self.pending_nodes.len()
    }

    /// Prune the DAG to remove old rounds.
    ///
    /// This should be called periodically to prevent unbounded growth.
    ///
    /// NOTE: This is a simplified version that doesn't actually prune.
    /// In production, you'd need interior mutability (Arc<Mutex<InMemDag>>)
    /// to allow mutation through the Arc.
    pub fn prune(&mut self) -> Result<()> {
        // Get the lowest unordered round
        let _lowest_unordered = self.order_rule.lowest_unordered_round();

        // In production, this would prune the DAG
        // self.dag.prune_below(&min_round);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::testing::{TestRound, TestAuthor, TestCertifiedNode, TestNodeBuilder, create_test_authors};
    use crate::dag::ordering::AnchorElection;
    use std::sync::Arc;

    // Simple anchor election for testing
    struct TestAnchorElection;

    impl AnchorElection for TestAnchorElection {
        type Round = TestRound;
        type Author = TestAuthor;

        fn get_anchor(&self, _round: &Self::Round) -> Self::Author {
            TestAuthor(1)
        }

        fn update_reputation(&self, _event: &crate::dag::ordering::CommitEvent<Self::Round, Self::Author>) {
            // Do nothing
        }
    }

    #[test]
    fn test_dag_driver_config_default() {
        let config = DagDriverConfig::default();
        assert_eq!(config.window_size, 100);
        assert_eq!(config.anchor_interval, 2);
        assert_eq!(config.max_concurrent_rounds, 10);
    }

    #[test]
    fn test_dag_driver_config_new() {
        let config = DagDriverConfig {
            window_size: 200,
            anchor_interval: 5,
            max_concurrent_rounds: 20,
        };
        assert_eq!(config.window_size, 200);
        assert_eq!(config.anchor_interval, 5);
        assert_eq!(config.max_concurrent_rounds, 20);
    }

    #[test]
    fn test_dag_driver_dag_accessor() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test dag accessor
        let _dag = driver.dag();
        // In production, this would give access to the DAG store
    }

    #[test]
    fn test_dag_driver_order_rule_accessor() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test order_rule accessor
        let _order_rule = driver.order_rule();
        // In production, this would give access to the ordering rule
    }

    #[test]
    fn test_dag_driver_enter_new_round() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test normal round progression
        assert!(driver.enter_new_round(TestRound(2)).is_ok());
        assert_eq!(driver.current_round(), &TestRound(2));

        // Test round advancement
        assert!(driver.enter_new_round(TestRound(5)).is_ok());
        assert_eq!(driver.current_round(), &TestRound(5));

        // Test that we can't go backwards
        let result = driver.enter_new_round(TestRound(3));
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_driver_get_strong_links() {
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test get_strong_links - simplified implementation returns empty vec
        let links = driver.get_strong_links(&TestRound(1));
        assert_eq!(links.len(), 0);

        let links2 = driver.get_strong_links(&TestRound(10));
        assert_eq!(links2.len(), 0);
    }

    #[test]
    fn test_dag_driver_create_proposal() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test create_proposal - simplified implementation just returns Ok(())
        assert!(driver.create_proposal().is_ok());
    }

    #[test]
    fn test_dag_driver_add_pending() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test that pending count starts at 0
        assert_eq!(driver.pending_count(), 0);

        // Add a pending node
        let node = Arc::new(TestNodeBuilder::new().round(1).author(1).build());
        driver.add_pending(node);

        // Test that pending count is now 1
        assert_eq!(driver.pending_count(), 1);

        // Add another pending node
        let node2 = Arc::new(TestNodeBuilder::new().round(1).author(2).build());
        driver.add_pending(node2);

        // Test that pending count is now 2
        assert_eq!(driver.pending_count(), 2);
    }

    #[test]
    fn test_dag_driver_process_pending() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Add some pending nodes
        let node1 = Arc::new(TestNodeBuilder::new().round(1).author(1).build());
        let node2 = Arc::new(TestNodeBuilder::new().round(1).author(2).build());
        driver.add_pending(node1);
        driver.add_pending(node2);

        assert_eq!(driver.pending_count(), 2);

        // Process pending nodes
        assert!(driver.process_pending().is_ok());
        
        // All nodes should be processed (even if they don't actually get added to DAG)
        assert_eq!(driver.pending_count(), 0);
    }

    #[test]
    fn test_dag_driver_process_pending_when_processing() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Add a pending node
        let node1 = Arc::new(TestNodeBuilder::new().round(1).author(1).build());
        driver.add_pending(node1);
        assert_eq!(driver.pending_count(), 1);

        // Set processing flag manually (simulating concurrent processing)
        driver.is_processing = true;

        // Process pending should return early when already processing
        assert!(driver.process_pending().is_ok());
        
        // Pending count should still be 1 since we didn't actually process
        assert_eq!(driver.pending_count(), 1);
    }

    #[test]
    fn test_dag_driver_prune() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test prune - simplified implementation just returns Ok(())
        assert!(driver.prune().is_ok());
    }

    #[test]
    fn test_dag_driver_config_all_fields() {
        let config = DagDriverConfig {
            window_size: 150,
            anchor_interval: 3,
            max_concurrent_rounds: 15,
        };
        assert_eq!(config.window_size, 150);
        assert_eq!(config.anchor_interval, 3);
        assert_eq!(config.max_concurrent_rounds, 15);
    }

    #[test]
    fn test_dag_driver_current_round() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(42),
            config,
        );

        assert_eq!(driver.current_round(), &TestRound(42));
    }

    #[test]
    fn test_dag_driver_with_custom_start_round() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(100),
            config,
        );

        assert_eq!(driver.current_round(), &TestRound(100));
        assert_eq!(driver.pending_count(), 0);
    }

    #[test]
    fn test_dag_driver_multiple_rounds() {
        let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let anchor_election = Arc::new(TestAnchorElection);
        let config = DagDriverConfig::default();

        let mut driver = DagDriver::<TestRound, TestAuthor, TestCertifiedNode, TestAnchorElection>::new(
            authors,
            anchor_election,
            TestRound(1),
            config,
        );

        // Test round progression
        assert!(driver.enter_new_round(TestRound(2)).is_ok());
        assert!(driver.enter_new_round(TestRound(3)).is_ok());
        assert!(driver.enter_new_round(TestRound(4)).is_ok());

        assert_eq!(driver.current_round(), &TestRound(4));
    }
}
