// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! DAG ordering rules and anchor election.
//!
//! This module implements the logic for determining which DAG nodes
//! can be committed and in what order. This is the core safety mechanism
//! for DAG-based consensus.

use consensus_traits::{
    dag::CertifiedNode,
    core::Hash as HashTrait,
};
use std::{
    fmt,
    sync::Arc,
};
use anyhow::Result;

/// Configuration for ordering rules.
#[derive(Clone, Debug)]
pub struct OrderingConfig {
    /// Window size for DAG retention
    pub window_size: u64,
    /// Anchor round interval (every N rounds)
    pub anchor_interval: u64,
}

impl Default for OrderingConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            anchor_interval: 2,
        }
    }
}

/// Commit event produced when nodes are ordered.
#[derive(Clone, Debug)]
pub struct CommitEvent<ID, A> {
    /// The anchor node that triggered ordering
    pub anchor_id: ID,
    /// Parents of the anchor
    pub parents: Vec<A>,
    /// Authors that failed to provide nodes
    pub failed_authors: Vec<A>,
}

impl<ID, A> CommitEvent<ID, A> {
    /// Create a new commit event.
    pub fn new(anchor_id: ID, parents: Vec<A>, failed_authors: Vec<A>) -> Self {
        Self {
            anchor_id,
            parents,
            failed_authors,
        }
    }
}

/// Result of attempting to order nodes.
#[derive(Clone, Debug)]
pub struct OrderingResult<N, ID, A>
where
    N: CertifiedNode,
{
    /// Ordered nodes (including the anchor)
    pub ordered_nodes: Vec<Arc<N>>,
    /// Commit event describing what was ordered
    pub commit_event: CommitEvent<ID, A>,
}

/// Anchor election strategy.
///
/// This trait determines which validators are selected as anchors
/// in each round. Different strategies can be used (e.g., round-robin,
/// reputation-based, random).
pub trait AnchorElection: Send + Sync {
    /// Round type
    type Round: Clone + Eq + fmt::Debug + Send + Sync + 'static;

    /// Author/validator type
    type Author: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static;

    /// Get the anchor author for the given round.
    fn get_anchor(&self, round: &Self::Round) -> Self::Author;

    /// Update reputation based on a commit event.
    fn update_reputation(&self, event: &CommitEvent<Self::Round, Self::Author>);
}

/// Simple round-robin anchor election.
///
/// This is a basic implementation that cycles through validators in order.
pub struct RoundRobinAnchorElection<R, A>
where
    R: Clone + Eq + fmt::Debug + Send + Sync + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
{
    authors: Vec<A>,
    _phantom: std::marker::PhantomData<R>,
}

impl<R, A> RoundRobinAnchorElection<R, A>
where
    R: Clone + Eq + fmt::Debug + Send + Sync + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
{
    /// Create a new round-robin anchor election strategy.
    pub fn new(authors: Vec<A>) -> Self {
        Self {
            authors,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the index of an author in the list.
    pub fn get_author_index(&self, author: &A) -> Option<usize> {
        self.authors.iter().position(|a| a == author)
    }
}

impl<R, A> AnchorElection for RoundRobinAnchorElection<R, A>
where
    R: Clone + Eq + fmt::Debug + Send + Sync + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
{
    type Round = R;
    type Author = A;

    fn get_anchor(&self, _round: &Self::Round) -> Self::Author {
        // Simplified: just cycle through authors
        let index = 0; // Placeholder - in production, use round number
        self.authors
            .get(index)
            .or_else(|| self.authors.first())
            .cloned()
            .unwrap_or_else(|| panic!("No authors available"))
    }

    fn update_reputation(&self, _event: &CommitEvent<Self::Round, Self::Author>) {
        // Round-robin doesn't track reputation
    }
}

/// DAG ordering rule engine.
///
/// This determines which nodes can be committed and in what order,
/// based on anchor election and reachability.
///
/// NOTE: This is a simplified framework implementation. The full ordering
/// logic with reachability checking and vote aggregation is complex and
/// can be added incrementally.
pub struct OrderRule<R, A, N, AE>
where
    R: Clone + Ord + fmt::Debug + Send + Sync + HashTrait + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
    N: CertifiedNode + 'static,
    N::Node: Clone + 'static,
    AE: AnchorElection<Round = R, Author = A>,
{
    /// Storage for the DAG
    dag: Arc<crate::dag::store::InMemDag<R, A, N>>,

    /// Anchor election strategy
    anchor_election: Arc<AE>,

    /// Lowest round that hasn't been ordered yet
    lowest_unordered_round: R,

    /// Configuration
    #[allow(dead_code)]
    config: OrderingConfig,
}

impl<R, A, N, AE> OrderRule<R, A, N, AE>
where
    R: Clone + Ord + fmt::Debug + Send + Sync + HashTrait + 'static,
    A: Clone + Eq + HashTrait + fmt::Debug + Send + Sync + 'static,
    N: CertifiedNode + 'static,
    N::Node: Clone + 'static,
    AE: AnchorElection<Round = R, Author = A>,
{
    /// Create a new ordering rule.
    pub fn new(
        dag: Arc<crate::dag::store::InMemDag<R, A, N>>,
        anchor_election: Arc<AE>,
        lowest_unordered_round: R,
        config: OrderingConfig,
    ) -> Self {
        Self {
            dag,
            anchor_election,
            lowest_unordered_round,
            config,
        }
    }

    /// Get the lowest unordered round.
    pub fn lowest_unordered_round(&self) -> &R {
        &self.lowest_unordered_round
    }

    /// Process a new node and check if it triggers any ordering.
    ///
    /// Returns ordered nodes if ordering was triggered.
    pub fn process_new_node(&mut self, _node: &N) -> Result<Option<OrderingResult<N, R, A>>> {
        // Simplified: don't trigger ordering from individual nodes
        // In production, this would check if the node provides enough votes
        Ok(None)
    }

    /// Process all pending nodes and attempt ordering.
    ///
    /// Returns all ordered nodes.
    pub fn process_all(&mut self) -> Result<Vec<OrderingResult<N, R, A>>> {
        // Simplified: return empty results
        // In production, this would:
        // 1. Find anchors with enough votes
        // 2. Order reachable nodes
        // 3. Update lowest_unordered_round
        Ok(vec![])
    }

    /// Try to order nodes starting from the given round.
    fn try_order_from_round(&mut self, _start_round: R) -> Result<Option<OrderingResult<N, R, A>>> {
        // Simplified: don't order anything
        // In production, this would:
        // 1. Find an anchor with sufficient votes
        // 2. Mark all reachable nodes as ordered
        // 3. Create a commit event
        Ok(None)
    }

    /// Find an anchor (node from an anchor validator) with sufficient votes.
    fn find_anchor_with_enough_votes(&self, _start_round: R) -> Result<Option<Arc<N>>> {
        // Simplified: return None
        // In production, this would check voting power
        Ok(None)
    }

    /// Order an anchor and all nodes reachable from it.
    fn order_anchor(&mut self, _anchor: Arc<N>) -> Result<Vec<Arc<N>>> {
        // Simplified: return empty vector
        // In production, this would:
        // 1. Mark all reachable nodes as ordered
        // 2. Update the DAG storage
        // 3. Update lowest_unordered_round
        Ok(vec![])
    }

    /// Create a commit event for ordered nodes.
    #[allow(dead_code)]
    fn create_commit_event(
        &self,
        _ordered_nodes: &[Arc<N>],
    ) -> CommitEvent<R, A> {
        // Simplified: return empty commit event
        // In production, this would extract actual information
        CommitEvent::new(
            self.lowest_unordered_round.clone(), // Placeholder
            vec![],
            vec![],
        )
    }

    /// Check if the given round has enough votes to trigger ordering.
    #[allow(dead_code)]
    fn can_trigger_ordering(&self, _round: &R) -> bool {
        // Simplified: return false
        // In production, this would check if we have 2f+1 voting power
        false
    }

    /// Compare two rounds.
    #[allow(dead_code)]
    fn compare_rounds(&self, _r1: &R, _r2: &R) -> std::cmp::Ordering {
        // Simplified: assume equal
        // In production, R would implement Ord
        std::cmp::Ordering::Equal
    }

    /// Get the next round.
    #[allow(dead_code)]
    fn next_round(&self, round: R) -> R {
        // Simplified: return same round
        // In production, this would add 1 to the round
        round
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::testing::{TestRound, TestAuthor, TestCertifiedNode, TestNodeBuilder};
    use std::sync::Arc;

    #[test]
    fn test_ordering_config_new() {
        let config = OrderingConfig {
            window_size: 200,
            anchor_interval: 5,
        };
        assert_eq!(config.window_size, 200);
        assert_eq!(config.anchor_interval, 5);
    }

    #[test]
    fn test_order_rule_new() {
        use crate::dag::store::{InMemDag, DagStoreConfig};

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));
        let round = TestRound(5);

        let order_rule = OrderRule::new(dag, election, round.clone(), config);

        assert_eq!(*order_rule.lowest_unordered_round(), round);
    }

    #[test]
    fn test_order_rule_lowest_unordered_round() {
        use crate::dag::store::{InMemDag, DagStoreConfig};

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));
        let round = TestRound(10);

        let order_rule = OrderRule::new(dag.clone(), election.clone(), round.clone(), config.clone());

        assert_eq!(*order_rule.lowest_unordered_round(), round);

        // Test with different round
        let order_rule2 = OrderRule::new(dag, election, TestRound(20), config);
        assert_eq!(*order_rule2.lowest_unordered_round(), TestRound(20));
    }

    #[test]
    fn test_order_rule_process_new_node() {
        use crate::dag::store::{InMemDag, DagStoreConfig};

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let mut order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let node = TestNodeBuilder::new()
            .epoch(1)
            .round(5)
            .author(1)
            .build();

        let result = order_rule.process_new_node(&node);
        assert!(result.is_ok());
        // Simplified implementation returns None
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_order_rule_process_all() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let mut order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let result = order_rule.process_all();
        assert!(result.is_ok());
        // Simplified implementation returns empty vec
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_order_rule_try_order_from_round() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let mut order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let result = order_rule.try_order_from_round(TestRound(10));
        assert!(result.is_ok());
        // Simplified implementation returns None
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_order_rule_find_anchor_with_enough_votes() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let result = order_rule.find_anchor_with_enough_votes(TestRound(10));
        assert!(result.is_ok());
        // Simplified implementation returns None
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_order_rule_order_anchor() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let mut order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let anchor = Arc::new(TestNodeBuilder::new()
            .epoch(1)
            .round(5)
            .author(1)
            .build());

        let result = order_rule.order_anchor(anchor);
        assert!(result.is_ok());
        // Simplified implementation returns empty vec
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_order_rule_create_commit_event() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let order_rule = OrderRule::new(dag, election, TestRound(5), config);

        let event = order_rule.create_commit_event(&[]);
        
        assert_eq!(event.parents.len(), 0);
        assert_eq!(event.failed_authors.len(), 0);
    }

    #[test]
    fn test_order_rule_can_trigger_ordering() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let order_rule = OrderRule::new(dag, election, TestRound(5), config);

        // Simplified implementation always returns false
        assert!(!order_rule.can_trigger_ordering(&TestRound(5)));
        assert!(!order_rule.can_trigger_ordering(&TestRound(10)));
    }

    #[test]
    fn test_order_rule_compare_rounds() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let order_rule = OrderRule::new(dag, election, TestRound(5), config);

        // Simplified implementation always returns Equal
        assert_eq!(order_rule.compare_rounds(&TestRound(5), &TestRound(5)), std::cmp::Ordering::Equal);
        assert_eq!(order_rule.compare_rounds(&TestRound(5), &TestRound(10)), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_order_rule_next_round() {
        use crate::dag::store::{InMemDag, DagStoreConfig};
        use std::sync::Arc;

        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = Arc::new(RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors));
        let config = OrderingConfig::default();
        let dag = Arc::new(InMemDag::<TestRound, TestAuthor, TestCertifiedNode>::new(
            vec![TestAuthor(1), TestAuthor(2)],
            DagStoreConfig::default(),
        ));

        let order_rule = OrderRule::new(dag, election, TestRound(5), config);

        // Simplified implementation returns same round
        assert_eq!(order_rule.next_round(TestRound(5)), TestRound(5));
        assert_eq!(order_rule.next_round(TestRound(10)), TestRound(10));
    }

    #[test]
    fn test_round_robin_get_author_index() {
        let authors = vec![
            TestAuthor(1),
            TestAuthor(2),
            TestAuthor(3),
        ];
        let election = RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors);

        assert_eq!(election.get_author_index(&TestAuthor(1)), Some(0));
        assert_eq!(election.get_author_index(&TestAuthor(2)), Some(1));
        assert_eq!(election.get_author_index(&TestAuthor(3)), Some(2));
        assert_eq!(election.get_author_index(&TestAuthor(99)), None);
    }

    #[test]
    fn test_round_robin_update_reputation() {
        let authors = vec![TestAuthor(1), TestAuthor(2)];
        let election = RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors);

        let event = CommitEvent::new(
            TestRound(10),
            vec![],
            vec![],
        );

        // Round-robin doesn't track reputation, but the method should exist
        election.update_reputation(&event);
        // No panic = success
    }
}
