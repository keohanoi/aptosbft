// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Proposer election traits for consensus.
//!
//! This module defines traits for selecting proposers (leaders) in each round
//! of consensus. Different strategies can be implemented (round-robin, stake-weighted,
//! reputation-based, etc.).

use crate::core::Hash as HashTrait;
use std::fmt::{Debug, Display};

/// Proposer election strategy.
///
/// This trait determines which validator is selected as the proposer
/// for each round of consensus.
pub trait ProposerElection: Send + Sync {
    /// Round type
    type Round: Clone + Eq + Debug + Display + Send + Sync + 'static;

    /// Author/validator type
    type Author: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static;

    /// Check if a given author is the valid proposer for the round.
    ///
    /// Returns true if the author is the proposer for this round.
    fn is_valid_proposer(&self, author: &Self::Author, round: &Self::Round) -> bool {
        self.get_valid_proposer(round).as_ref() == Some(author)
    }

    /// Get the valid proposer for a given round.
    ///
    /// Returns None if no proposer is available for this round.
    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author>;

    /// Get the voting power participation ratio for the round.
    ///
    /// This represents the fraction of total voting power that is
    /// actively participating in consensus (0.0 to 1.0).
    ///
    /// Default implementation returns 1.0 (full participation).
    fn get_voting_power_participation_ratio(&self, _round: &Self::Round) -> f64 {
        1.0
    }

    /// Get both the proposer and participation ratio for a round.
    ///
    /// This is a convenience method that combines the two queries.
    fn get_proposer_info(
        &self,
        round: &Self::Round,
    ) -> ProposerInfo<Self::Author> {
        ProposerInfo {
            proposer: self.get_valid_proposer(round),
            participation_ratio: self.get_voting_power_participation_ratio(round),
        }
    }
}

/// Information about the proposer for a round.
///
/// This includes both the proposer identity and the voting power
/// participation ratio for the round.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposerInfo<A> {
    /// The proposer for the round (None if no proposer)
    pub proposer: Option<A>,
    /// Voting power participation ratio (0.0 to 1.0)
    pub participation_ratio: f64,
}

impl<A> ProposerInfo<A> {
    /// Create new proposer info.
    pub fn new(proposer: Option<A>, participation_ratio: f64) -> Self {
        Self {
            proposer,
            participation_ratio,
        }
    }

    /// Check if there is a proposer.
    pub fn has_proposer(&self) -> bool {
        self.proposer.is_some()
    }

    /// Check if participation is healthy (>= 2/3).
    pub fn is_healthy(&self) -> bool {
        self.participation_ratio >= 2.0 / 3.0
    }
}

/// Reputation tracker for proposer election.
///
/// Some proposer election strategies track validator reputation
/// to preferentially select well-behaved validators.
pub trait ReputationTracker: Send + Sync {
    /// Author/validator type
    type Author: Clone + Eq + HashTrait + Debug + Display + Send + Sync + 'static;

    /// Record successful proposal by an author.
    fn record_success(&mut self, author: &Self::Author);

    /// Record failure by an author.
    fn record_failure(&mut self, author: &Self::Author);

    /// Get reputation score for an author.
    ///
    /// Returns a value from 0.0 (worst) to 1.0 (best).
    fn get_reputation(&self, author: &Self::Author) -> f64;

    /// Reset reputation scores (e.g., on epoch change).
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Error;

    // Simple test types
    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestRound(u64);

    impl Display for TestRound {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestRound {
        fn zero() -> Self {
            TestRound(0)
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(TestRound(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestAuthor(u8);

    impl Display for TestAuthor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestAuthor {
        fn zero() -> Self {
            TestAuthor(0)
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(TestAuthor(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    // Simple test implementation
    struct TestProposerElection;

    impl ProposerElection for TestProposerElection {
        type Round = TestRound;
        type Author = TestAuthor;

        fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
            // Always return author 1 as proposer
            if round.0 > 0 {
                Some(TestAuthor(1))
            } else {
                None
            }
        }

        fn get_voting_power_participation_ratio(&self, round: &Self::Round) -> f64 {
            // When there's no proposer, participation is 0.0 (unhealthy)
            if round.0 > 0 {
                1.0
            } else {
                0.0
            }
        }
    }

    #[test]
    fn test_is_valid_proposer() {
        let election = TestProposerElection;

        assert!(election.is_valid_proposer(&TestAuthor(1), &TestRound(1)));
        assert!(!election.is_valid_proposer(&TestAuthor(2), &TestRound(1)));
    }

    #[test]
    fn test_proposer_info() {
        let election = TestProposerElection;
        let info = election.get_proposer_info(&TestRound(1));

        assert_eq!(info.proposer, Some(TestAuthor(1)));
        assert_eq!(info.participation_ratio, 1.0);
        assert!(info.has_proposer());
        assert!(info.is_healthy());
    }

    #[test]
    fn test_proposer_info_no_proposer() {
        let election = TestProposerElection;
        let info = election.get_proposer_info(&TestRound(0));

        assert_eq!(info.proposer, None);
        assert!(!info.has_proposer());
        assert!(!info.is_healthy()); // No proposer means unhealthy
    }
}
