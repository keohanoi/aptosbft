// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Safety rules for Byzantine fault-tolerant consensus.
//!
//! This module defines traits for enforcing safety rules in consensus protocols.
//! Safety rules ensure that validators never violate consensus safety properties,
//! preventing forks and double-spending attacks.
//!
//! # Overview
//!
//! The safety rules are the core component that separates a voting system from a
//! BFT consensus protocol. Without safety rules, validators can vote but cannot
//! guarantee agreement. With safety rules, validators reach agreement with mathematical
//! certainty even in the presence of Byzantine faults.
//!
//! # 2-Chain Commit Rule
//!
//! The 2-chain commit rule is the primary safety mechanism in AptosBFT:
//!
//! > A block B0 can be committed if there exists a certified block B1 such that:
//! > 1. B0 extends B1 (B0.parent = B1)
//! > 2. round(B0) + 1 = round(B1)
//!
//! This ensures that all honest validators agree on committed blocks.
//!
//! # Safety Rule Types
//!
//! - **Voting Safety**: [`SafetyRules::safe_to_vote`] - Check if it's safe to vote for a proposal
//! - **Timeout Safety**: [`SafetyRules::safe_to_timeout`] - Check if it's safe to timeout in a round
//! - **State Management**: [`SafetyState`] - Track safety state (last voted round, HQC, etc.)

use crate::block::{Block, BlockMetadata, Vote};
use crate::core::{Error, Hash};
use std::sync::Arc;

/// Safety state tracking for consensus.
///
/// This trait represents the persistent state required to enforce safety rules.
/// The state must be persisted across restarts to prevent safety violations.
///
/// # Required State
///
/// - `last_voted_round`: The last round this validator voted in (prevents double-voting)
/// - `one_chain_round`: The round of the highest 1-chain (QC) seen
/// - `preferred_round`: The round of the highest 2-chain seen
/// - `highest_timeout_round`: The highest round for which a timeout certificate was seen
///
/// # Safety Invariants
///
/// 1. `last_voted_round` is monotonically increasing
/// 2. `one_chain_round <= preferred_round`
/// 3. `highest_timeout_round` tracks the highest TC round seen
pub trait SafetyState: Clone + Send + Sync + 'static {
    /// Get the epoch this safety state is for
    fn epoch(&self) -> u64;

    /// Get the last round this validator voted in
    ///
    /// This prevents voting for older rounds (double-vote protection)
    fn last_voted_round(&self) -> u64;

    /// Get the round of the highest 1-chain (quorum certificate) seen
    ///
    /// A 1-chain is a single certified block. This is used in the 2-chain commit rule
    /// to determine if we can commit the parent block.
    fn one_chain_round(&self) -> u64;

    /// Get the preferred round (highest 2-chain round seen)
    ///
    /// A 2-chain is two consecutive certified blocks. The preferred round indicates
    /// the highest round in a 2-chain, which is used for proposer election and
    /// commit decisions.
    fn preferred_round(&self) -> u64;

    /// Get the highest round for which a timeout certificate was seen
    ///
    /// Timeout certificates allow the protocol to make progress when the proposer
    /// is faulty or offline. This tracks the highest TC round to ensure we only
    /// extend from the most recent timeout.
    fn highest_timeout_round(&self) -> u64;
}

/// Timeout certificate for round advancement.
///
/// A timeout certificate proves that 2f+1 validators have timed out in a round,
/// allowing all validators to safely advance to the next round without violating
/// safety.
///
/// # Generic over Block and Vote
///
/// Like other consensus types, this is generic over the Block and Vote types
/// to work with any blockchain implementation.
pub trait TimeoutCertificate: Clone + Send + Sync + 'static {
    /// The block type
    type Block: Block;

    /// Get the round this timeout certificate is for
    fn round(&self) -> u64;

    /// Get the epoch this timeout certificate is for
    fn epoch(&self) -> u64;

    /// Get the highest quorum certificate round referenced in this timeout
    ///
    /// This is used to ensure the timeout rule is safe: we can only timeout
    /// if the referenced QC round is >= our one_chain_round.
    fn highest_qc_round(&self) -> u64;

    /// Get the highest timeout certificate round referenced in this timeout
    ///
    /// When TCs chain together, each TC references the previous TC's round.
    /// This allows tracking the "highest HQC round" in the timeout chain.
    fn highest_tc_round(&self) -> u64;

    /// Verify this timeout certificate
    ///
    /// # Errors
    ///
    /// Returns an error if the TC signature is invalid
    fn verify(&self) -> Result<(), Error>;
}

/// Safety rules for Byzantine fault-tolerant consensus.
///
/// This trait enforces the critical safety rules that prevent consensus violations
/// including forks, double-spending, and safety violations.
///
/// # Why Safety Rules Matter
///
/// Without safety rules:
/// - A malicious validator could propose conflicting blocks
/// - Different validators could commit different blocks (fork)
/// - Double-spend attacks become possible
///
/// With safety rules:
/// - All honest validators agree on committed blocks (safety)
/// - No forks can occur
/// - The protocol makes progress under partial synchrony (liveness)
///
/// # 2-Chain Safety Rule
///
/// The core voting rule (from [`SafetyRules::safe_to_vote`]) is:
///
/// > A vote for block B is safe if EITHER:
/// > 1. B.round == B.quorum_cert.round + 1 (normal round progression), OR
/// > 2. B.round == timeout_cert.round + 1 AND B.quorum_cert.round >= timeout_cert.highest_qc_round (with TC)
///
/// # 2-Chain Timeout Rule
///
/// The timeout rule (from [`SafetyRules::safe_to_timeout`]) is:
///
/// > A timeout in round R is safe if:
/// > 1. R == timeout.qc.round + 1 OR R == timeout_cert.round + 1, AND
/// > 2. timeout.qc.round >= one_chain_round
///
/// # Thread Safety
///
/// Safety rules are typically used behind a mutex or RwLock since they're
/// called from multiple consensus tasks. Implementations should be thread-safe.
pub trait SafetyRules<B, V>: Send + Sync
where
    B: Block,
    V: Vote<Block = B>,
{
    /// The safety state type for this implementation
    type State: SafetyState;

    /// The timeout certificate type
    type TimeoutCert: TimeoutCertificate<Block = B>;

    /// Check if it's safe to vote for this proposal
    ///
    /// This enforces the 2-chain commit rule to prevent safety violations.
    ///
    /// # Parameters
    ///
    /// * `proposal` - The block proposal being voted on
    /// * `timeout_cert` - Optional timeout certificate if voting after a timeout
    ///
    /// # Errors
    ///
    /// Returns an error if voting would violate safety rules:
    /// - `Error::NotSafeToVote` - The 2-chain rule is violated
    ///
    /// # Example
    ///
    /// The 2-chain safety rule ensures you only extend the longest certified chain:
    ///
    /// ```rust,no_run
    /// // Safe: block.round == qc.round + 1 (normal progression)
    /// // Safe: block.round == tc.round + 1 AND qc.round >= tc.highest_qc_round (with timeout)
    /// // Unsafe: gap in rounds or violates one-chain invariant
    /// #
    /// # use consensus_traits::{safety::SafetyRules, core::Error};
    /// # // The actual implementation would check:
    /// # // 1. Normal round progression: block_round == qc_round + 1
    /// # // 2. OR with timeout: block_round == tc_round + 1 AND qc_round >= tc_hqc_round
    /// ```
    ///
    /// Always call `safe_to_vote()` before casting a vote to prevent safety violations.
    fn safe_to_vote(
        &self,
        proposal: &Arc<B>,
        timeout_cert: Option<&Self::TimeoutCert>,
    ) -> Result<(), Error>;

    /// Check if it's safe to timeout in this round
    ///
    /// This ensures timeouts don't violate the 2-chain safety rule.
    ///
    /// # Parameters
    ///
    /// * `round` - The round timing out
    /// * `qc_round` - The highest QC round known
    /// * `timeout_cert` - Optional existing timeout certificate
    ///
    /// # Errors
    ///
    /// Returns an error if the timeout would violate safety:
    /// - `Error::NotSafeToTimeout` - The timeout rule is violated
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // Safe to timeout: consecutive round from QC
    /// // round == qc_round + 1 OR round == tc_round + 1
    /// // AND qc_round >= one_chain_round
    /// #
    /// # use consensus_traits::{safety::SafetyRules, core::Error};
    /// # // Example: round=5, qc_round=4, one_chain_round=3 => Safe
    /// # // Example: round=5, qc_round=2, one_chain_round=3 => NotSafeToTimeout error
    /// ```
    fn safe_to_timeout(
        &self,
        round: u64,
        qc_round: u64,
        timeout_cert: Option<&Self::TimeoutCert>,
    ) -> Result<(), Error>;

    /// Update safety state with a new quorum certificate
    ///
    /// This should be called whenever a new QC is observed to update the
    /// one_chain_round and potentially the preferred_round.
    ///
    /// # Parameters
    ///
    /// * `qc` - The new quorum certificate to observe
    ///
    /// # Errors
    ///
    /// Returns an error if the QC is invalid or would violate safety
    fn observe_qc(&mut self, qc: &<B::Metadata as BlockMetadata>::QuorumCert) -> Result<(), Error>;

    /// Update safety state with a timeout certificate
    ///
    /// This should be called whenever a new TC is observed to update the
    /// highest_timeout_round.
    ///
    /// # Parameters
    ///
    /// * `tc` - The new timeout certificate to observe
    ///
    /// # Errors
    ///
    /// Returns an error if the TC is invalid
    fn observe_tc(&mut self, tc: &Self::TimeoutCert) -> Result<(), Error>;

    /// Get the current safety state
    ///
    /// This returns a snapshot of the safety state for monitoring and debugging.
    fn state(&self) -> &Self::State;

    /// Check if we should commit a block based on the 2-chain commit rule
    ///
    /// The 2-chain commit rule states: block B0 can be committed if there exists
    /// a certified block B1 such that B1 extends B0 and round(B0) + 1 = round(B1).
    ///
    /// # Parameters
    ///
    /// * `proposal` - The proposed block
    ///
    /// # Returns
    ///
    /// Returns the block ID to commit, or None if no block should be committed
    fn should_commit(&self, proposal: &Arc<B>) -> Option<<B as Block>::Hash>;

    /// Record that we voted in a specific round
    ///
    /// This updates the last_voted_round to prevent double-voting.
    ///
    /// # Parameters
    ///
    /// * `round` - The round we voted in
    ///
    /// # Errors
    ///
    /// Returns an error if the round is older than last_voted_round
    fn record_vote(&mut self, round: u64) -> Result<(), Error>;

    /// Record a timeout in a specific round
    ///
    /// This updates the highest_timeout_round.
    ///
    /// # Parameters
    ///
    /// * `round` - The round that timed out
    fn record_timeout(&mut self, round: u64);
}

/// Result of the 2-chain commit rule check
///
/// When a validator votes for a block, it may be able to commit an older block
/// based on the 2-chain rule. This enum represents the outcome of that check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitDecision<H> {
    /// No block should be committed (doesn't satisfy 2-chain rule)
    NoCommit,

    /// A block should be committed (contains the block ID)
    Commit(H),
}

impl<H: Hash> CommitDecision<H> {
    /// Get the committed block ID if any
    pub fn committed_id(&self) -> Option<&H> {
        match self {
            CommitDecision::NoCommit => None,
            CommitDecision::Commit(id) => Some(id),
        }
    }

    /// Check if this is a commit decision
    pub fn is_commit(&self) -> bool {
        matches!(self, CommitDecision::Commit(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_decision() {
        use crate::core::Hash;

        // Use a wrapper type that implements the full Hash trait
        #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
        struct TestHash(u64);

        impl Hash for TestHash {
            fn zero() -> Self {
                TestHash(0)
            }

            fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
                Ok(TestHash(0))
            }

            fn as_bytes(&self) -> &[u8] {
                &[]
            }
        }

        let no_commit = CommitDecision::<TestHash>::NoCommit;
        let commit = CommitDecision::Commit(TestHash(123));

        assert!(!no_commit.is_commit());
        assert!(commit.is_commit());
        assert_eq!(commit.committed_id(), Some(&TestHash(123)));
        assert_eq!(no_commit.committed_id(), None);
    }
}
