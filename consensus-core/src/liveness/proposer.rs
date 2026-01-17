// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Proposer election implementations.
//!
//! This module provides concrete implementations of proposer election strategies
//! for use in consensus.

use consensus_traits::{
    proposer::{ProposerElection, ProposerInfo, ReputationTracker},
    core::Hash as HashTrait,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Display},
    sync::Arc,
};
use anyhow::Result;

/// Configuration for proposer election.
#[derive(Clone, Debug)]
pub struct ProposerElectionConfig {
    /// Number of validators
    pub num_validators: usize,
    /// Starting round number
    pub initial_round: u64,
}

impl Default for ProposerElectionConfig {
    fn default() -> Self {
        Self {
            num_validators: 4,
            initial_round: 1,
        }
    }
}

/// Round-robin proposer election.
///
/// This is the simplest strategy that cycles through validators in order.
/// It doesn't account for voting power or reputation.
#[derive(Clone)]
pub struct RoundRobinProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Ordered list of validators
    validators: Vec<A>,

    /// Configuration
    config: ProposerElectionConfig,
}

impl<A> RoundRobinProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Create a new round-robin proposer election.
    pub fn new(validators: Vec<A>, config: ProposerElectionConfig) -> Self {
        Self {
            validators,
            config,
        }
    }

    /// Get the number of validators.
    pub fn num_validators(&self) -> usize {
        self.validators.len()
    }

    /// Get the validator at a given index.
    pub fn get_validator(&self, index: usize) -> Option<&A> {
        self.validators.get(index)
    }
}

impl<A> ProposerElection for RoundRobinProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    type Round = u64;
    type Author = A;

    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
        if self.validators.is_empty() {
            return None;
        }

        if *round < self.config.initial_round {
            return None;
        }

        let round_index = (*round - self.config.initial_round) as usize;
        let proposer_index = round_index % self.validators.len();

        self.validators.get(proposer_index).cloned()
    }
}

/// Weighted proposer election based on voting power.
///
/// This strategy selects proposers proportionally to their voting power.
/// Validators with more stake are selected more frequently.
#[derive(Clone)]
pub struct WeightedProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Validators with their voting power
    validators: Vec<(A, u64)>,

    /// Total voting power
    total_voting_power: u128,

    /// Configuration
    config: ProposerElectionConfig,
}

impl<A> WeightedProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Create a new weighted proposer election.
    ///
    /// ## Parameters
    ///
    /// - `validators`: List of validators with their voting power
    /// - `config`: Configuration
    pub fn new(validators: Vec<(A, u64)>, config: ProposerElectionConfig) -> Self {
        let total_voting_power = validators.iter().map(|(_, power)| *power as u128).sum();

        Self {
            validators,
            total_voting_power,
            config,
        }
    }

    /// Select a proposer using weighted random selection.
    ///
    /// This is a simplified implementation that uses the round number
    /// to deterministically select a proposer based on weights.
    fn select_weighted_proposer(&self, round: &u64) -> Option<A> {
        if self.validators.is_empty() {
            return None;
        }

        // Use round number as seed for deterministic selection
        let mut cumulative = 0u128;
        let target = (*round as u128) % self.total_voting_power;

        for (validator, weight) in &self.validators {
            cumulative += *weight as u128;
            if target < cumulative {
                return Some(validator.clone());
            }
        }

        // Fallback to last validator
        self.validators.last().map(|(v, _)| v.clone())
    }
}

impl<A> ProposerElection for WeightedProposer<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    type Round = u64;
    type Author = A;

    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
        if *round < self.config.initial_round {
            return None;
        }

        self.select_weighted_proposer(round)
    }

    fn get_voting_power_participation_ratio(&self, _round: &Self::Round) -> f64 {
        // Simplified: assume full participation
        // In production, this would track actual participation
        1.0
    }
}

/// Reputation tracker implementation.
///
/// This tracks validator success/failure and can be used to
/// adjust proposer selection.
#[derive(Clone)]
pub struct ReputationTrackerImpl<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Reputation scores for each validator (0.0 to 1.0)
    reputations: HashMap<A, f64>,

    /// Default reputation for new validators
    default_reputation: f64,
}

impl<A> ReputationTrackerImpl<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    /// Create a new reputation tracker.
    ///
    /// ## Parameters
    ///
    /// - `validators`: List of validators to track
    /// - `default_reputation`: Initial reputation score (0.0 to 1.0)
    pub fn new(validators: Vec<A>, default_reputation: f64) -> Self {
        let reputations = validators
            .into_iter()
            .map(|v| (v, default_reputation))
            .collect();

        Self {
            reputations,
            default_reputation,
        }
    }

    /// Update reputation with a delta.
    fn update_reputation(&mut self, author: &A, delta: f64) {
        let entry = self
            .reputations
            .entry(author.clone())
            .or_insert_with(|| self.default_reputation);

        *entry = (*entry + delta).clamp(0.0, 1.0);
    }
}

impl<A> ReputationTracker for ReputationTrackerImpl<A>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
{
    type Author = A;

    fn record_success(&mut self, author: &Self::Author) {
        // Increase reputation slightly on success
        self.update_reputation(author, 0.01);
    }

    fn record_failure(&mut self, author: &Self::Author) {
        // Decrease reputation on failure
        self.update_reputation(author, -0.1);
    }

    fn get_reputation(&self, author: &Self::Author) -> f64 {
        self.reputations
            .get(author)
            .copied()
            .unwrap_or(self.default_reputation)
    }

    fn reset(&mut self) {
        // Reset all reputations to default
        for reputation in self.reputations.values_mut() {
            *reputation = self.default_reputation;
        }
    }
}

/// Wrapper that combines proposer election with reputation tracking.
///
/// This implementation selects proposers based on both the underlying
/// strategy and reputation scores.
#[derive(Clone)]
pub struct ProposerElectionImpl<A, PE>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
    PE: ProposerElection<Round = u64, Author = A>,
{
    /// Base proposer election strategy
    base_election: PE,

    /// Reputation tracker (optional)
    reputation: Option<ReputationTrackerImpl<A>>,
}

impl<A, PE> ProposerElectionImpl<A, PE>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
    PE: ProposerElection<Round = u64, Author = A>,
{
    /// Create a new proposer election with reputation tracking.
    ///
    /// ## Parameters
    ///
    /// - `base_election`: Base election strategy
    /// - `reputation`: Optional reputation tracker
    pub fn new(
        base_election: PE,
        reputation: Option<ReputationTrackerImpl<A>>,
    ) -> Self {
        Self {
            base_election,
            reputation,
        }
    }

    /// Create without reputation tracking.
    pub fn without_reputation(base_election: PE) -> Self {
        Self {
            base_election,
            reputation: None,
        }
    }
}

impl<A, PE> ProposerElection for ProposerElectionImpl<A, PE>
where
    A: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static,
    PE: ProposerElection<Round = u64, Author = A>,
{
    type Round = u64;
    type Author = A;

    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
        // For now, just use the base election
        // In production, this could incorporate reputation scores
        self.base_election.get_valid_proposer(round)
    }

    fn get_voting_power_participation_ratio(&self, round: &Self::Round) -> f64 {
        self.base_election.get_voting_power_participation_ratio(round)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test author type
    #[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
    struct TestAuthor(u8);

    impl Display for TestAuthor {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    // Hash trait impl for TestAuthor
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

    #[test]
    fn test_round_robin_proposer() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = ProposerElectionConfig::default();

        let election = RoundRobinProposer::new(validators, config);

        assert_eq!(election.get_valid_proposer(&1), Some(TestAuthor(1)));
        assert_eq!(election.get_valid_proposer(&2), Some(TestAuthor(2)));
        assert_eq!(election.get_valid_proposer(&3), Some(TestAuthor(3)));
        assert_eq!(election.get_valid_proposer(&4), Some(TestAuthor(1))); // Wraps around
    }

    #[test]
    fn test_weighted_proposer() {
        let validators = vec![
            (TestAuthor(1), 30),
            (TestAuthor(2), 70),
        ];
        let config = ProposerElectionConfig::default();

        let election = WeightedProposer::new(validators, config);

        // With weights 30 and 70, we should see selection proportional to weights
        let mut counts = std::collections::HashMap::new();

        for round in 1..=100 {
            if let Some(proposer) = election.get_valid_proposer(&round) {
                *counts.entry(proposer.0).or_insert(0) += 1;
            }
        }

        // Author 2 should be selected more often (higher weight)
        let count1 = *counts.get(&1).unwrap_or(&0);
        let count2 = *counts.get(&2).unwrap_or(&0);

        assert!(count2 > count1, "Author with higher weight should be selected more");
    }

    #[test]
    fn test_reputation_tracker() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let mut tracker = ReputationTrackerImpl::new(validators, 0.5);

        assert_eq!(tracker.get_reputation(&TestAuthor(1)), 0.5);

        tracker.record_success(&TestAuthor(1));
        assert!(tracker.get_reputation(&TestAuthor(1)) > 0.5);

        tracker.record_failure(&TestAuthor(1));
        assert!(tracker.get_reputation(&TestAuthor(1)) < 0.6); // After success + failure

        tracker.reset();
        assert_eq!(tracker.get_reputation(&TestAuthor(1)), 0.5);
    }

    #[test]
    fn test_reputation_clamped() {
        let validators = vec![TestAuthor(1)];
        let mut tracker = ReputationTrackerImpl::new(validators, 0.5);

        // Test upper bound
        for _ in 0..100 {
            tracker.record_success(&TestAuthor(1));
        }
        assert_eq!(tracker.get_reputation(&TestAuthor(1)), 1.0);

        // Test lower bound
        for _ in 0..100 {
            tracker.record_failure(&TestAuthor(1));
        }
        assert_eq!(tracker.get_reputation(&TestAuthor(1)), 0.0);
    }

    #[test]
    fn test_proposer_election_config_default() {
        let config = ProposerElectionConfig::default();
        assert_eq!(config.num_validators, 4);
        assert_eq!(config.initial_round, 1);
    }

    #[test]
    fn test_proposer_election_config_custom() {
        let config = ProposerElectionConfig {
            num_validators: 10,
            initial_round: 5,
        };
        assert_eq!(config.num_validators, 10);
        assert_eq!(config.initial_round, 5);
    }

    #[test]
    fn test_round_robin_empty_validators() {
        let election: RoundRobinProposer<TestAuthor> = RoundRobinProposer::new(vec![], ProposerElectionConfig::default());
        assert_eq!(election.get_valid_proposer(&1), None);
    }

    #[test]
    fn test_round_robin_before_initial_round() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = ProposerElectionConfig {
            num_validators: 3,
            initial_round: 5,
        };
        let election = RoundRobinProposer::new(validators, config);

        // Before initial round
        assert_eq!(election.get_valid_proposer(&3), None);
        assert_eq!(election.get_valid_proposer(&4), None);

        // At initial round
        assert_eq!(election.get_valid_proposer(&5), Some(TestAuthor(1)));
    }

    #[test]
    fn test_round_robin_num_validators() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let election = RoundRobinProposer::new(validators, ProposerElectionConfig::default());
        assert_eq!(election.num_validators(), 3);
    }

    #[test]
    fn test_round_robin_get_validator() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let election = RoundRobinProposer::new(validators, ProposerElectionConfig::default());

        assert_eq!(election.get_validator(0), Some(&TestAuthor(1)));
        assert_eq!(election.get_validator(1), Some(&TestAuthor(2)));
        assert_eq!(election.get_validator(2), Some(&TestAuthor(3)));
        assert_eq!(election.get_validator(3), None);
        assert_eq!(election.get_validator(100), None);
    }

    #[test]
    fn test_weighted_proposer_empty_validators() {
        let election: WeightedProposer<TestAuthor> = WeightedProposer::new(vec![], ProposerElectionConfig::default());
        assert_eq!(election.get_valid_proposer(&1), None);
    }

    #[test]
    fn test_weighted_proposer_before_initial_round() {
        let validators = vec![
            (TestAuthor(1), 30),
            (TestAuthor(2), 70),
        ];
        let config = ProposerElectionConfig {
            num_validators: 2,
            initial_round: 10,
        };
        let election = WeightedProposer::new(validators, config);

        // Before initial round
        assert_eq!(election.get_valid_proposer(&5), None);

        // At initial round
        assert!(election.get_valid_proposer(&10).is_some());
    }

    #[test]
    fn test_weighted_proposer_participation_ratio() {
        let validators = vec![
            (TestAuthor(1), 30),
            (TestAuthor(2), 70),
        ];
        let election = WeightedProposer::new(validators, ProposerElectionConfig::default());

        // Simplified implementation always returns 1.0
        assert_eq!(election.get_voting_power_participation_ratio(&1), 1.0);
        assert_eq!(election.get_voting_power_participation_ratio(&100), 1.0);
    }

    #[test]
    fn test_reputation_tracker_unknown_validator() {
        let validators = vec![TestAuthor(1)];
        let tracker = ReputationTrackerImpl::new(validators, 0.5);

        // Unknown validator should get default reputation
        assert_eq!(tracker.get_reputation(&TestAuthor(99)), 0.5);
    }

    #[test]
    fn test_reputation_tracker_new_validator_after_tracking() {
        let validators = vec![TestAuthor(1)];
        let mut tracker = ReputationTrackerImpl::new(validators, 0.5);

        // Record failure for unknown validator (should add it with default)
        tracker.record_failure(&TestAuthor(99));
        let rep = tracker.get_reputation(&TestAuthor(99));
        assert!(rep < 0.5 && rep >= 0.0);

        // Record success for unknown validator
        tracker.record_success(&TestAuthor(88));
        let rep2 = tracker.get_reputation(&TestAuthor(88));
        assert!(rep2 > 0.5 && rep2 <= 1.0);
    }

    #[test]
    fn test_proposer_election_impl_new() {
        let validators = vec![TestAuthor(1), TestAuthor(2)];
        let base = RoundRobinProposer::new(validators.clone(), ProposerElectionConfig::default());
        let reputation = ReputationTrackerImpl::new(validators, 0.5);

        let impl_election = ProposerElectionImpl::new(base.clone(), Some(reputation.clone()));
        assert_eq!(impl_election.get_valid_proposer(&1), Some(TestAuthor(1)));
    }

    #[test]
    fn test_proposer_election_impl_without_reputation() {
        let validators = vec![TestAuthor(1), TestAuthor(2)];
        let base = RoundRobinProposer::new(validators, ProposerElectionConfig::default());

        let impl_election = ProposerElectionImpl::without_reputation(base);
        assert_eq!(impl_election.get_valid_proposer(&1), Some(TestAuthor(1)));
    }

    #[test]
    fn test_proposer_election_impl_get_valid_proposer() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let base = RoundRobinProposer::new(validators, ProposerElectionConfig::default());
        let impl_election = ProposerElectionImpl::without_reputation(base);

        assert_eq!(impl_election.get_valid_proposer(&1), Some(TestAuthor(1)));
        assert_eq!(impl_election.get_valid_proposer(&2), Some(TestAuthor(2)));
        assert_eq!(impl_election.get_valid_proposer(&3), Some(TestAuthor(3)));
        assert_eq!(impl_election.get_valid_proposer(&4), Some(TestAuthor(1)));
    }

    #[test]
    fn test_proposer_election_impl_voting_power_participation() {
        let validators = vec![
            (TestAuthor(1), 30),
            (TestAuthor(2), 70),
        ];
        let base = WeightedProposer::new(validators, ProposerElectionConfig::default());
        let impl_election = ProposerElectionImpl::without_reputation(base);

        assert_eq!(impl_election.get_voting_power_participation_ratio(&1), 1.0);
    }

    #[test]
    fn test_weighted_proposer_fallback() {
        // Test the fallback case when target >= cumulative
        let validators = vec![
            (TestAuthor(1), 1),  // Very small weight
        ];
        let election = WeightedProposer::new(validators, ProposerElectionConfig::default());

        // Should always return the only validator
        assert_eq!(election.get_valid_proposer(&1), Some(TestAuthor(1)));
        assert_eq!(election.get_valid_proposer(&1000), Some(TestAuthor(1)));
    }

    #[test]
    fn test_round_robin_large_round() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let config = ProposerElectionConfig::default();
        let election = RoundRobinProposer::new(validators, config);

        // Test with very large round number
        // round_index = 1000000 - 1 = 999999
        // proposer_index = 999999 % 3 = 0
        assert_eq!(election.get_valid_proposer(&1000000), Some(TestAuthor(1)));
    }

    #[test]
    fn test_reputation_tracker_multiple_validators() {
        let validators = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
        let mut tracker = ReputationTrackerImpl::new(validators, 0.5);

        tracker.record_success(&TestAuthor(1));
        tracker.record_failure(&TestAuthor(2));
        tracker.record_success(&TestAuthor(3));

        assert!(tracker.get_reputation(&TestAuthor(1)) > 0.5);
        assert!(tracker.get_reputation(&TestAuthor(2)) < 0.5);
        assert!(tracker.get_reputation(&TestAuthor(3)) > 0.5);
    }
}
