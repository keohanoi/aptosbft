// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Safety state for consensus rules.
//!
//! This module provides the persistent safety state that tracks the critical
//! information needed to enforce the 2-chain commit rule.

use consensus_traits::SafetyState;
use serde::{Deserialize, Serialize};

/// Safety state data for enforcing consensus safety rules.
///
/// This structure contains all the persistent state needed to enforce the
/// 2-chain commit rule. It must be persisted across restarts to prevent
/// safety violations.
///
/// # Fields
///
/// - `epoch`: The current epoch
/// - `last_voted_round`: The last round this validator voted in
/// - `preferred_round`: The highest 2-chain round seen (for 3-chain protocol)
/// - `one_chain_round`: The highest 1-chain round seen (highest QC round)
/// - `highest_timeout_round`: The highest round for which a TC was seen
///
/// # Safety Invariants
///
/// 1. `last_voted_round` is monotonically increasing (prevents double-voting)
/// 2. `one_chain_round <= preferred_round`
/// 3. `highest_timeout_round >= 0`
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SafetyStateData {
    /// The current epoch
    pub epoch: u64,

    /// The last round this validator voted in
    ///
    /// This prevents voting for older rounds (double-vote protection).
    /// Must only increase.
    pub last_voted_round: u64,

    /// The highest 2-chain round seen (preferred round)
    ///
    /// A 2-chain is two consecutive certified blocks. This is used for
    /// proposer election and in the 3-chain commit rule.
    pub preferred_round: u64,

    /// The highest 1-chain round seen
    ///
    /// A 1-chain is a single certified block (QC). This is critical for
    /// the 2-chain commit rule: we can only vote for blocks that extend
    /// from at least this round.
    pub one_chain_round: u64,

    /// The highest round for which a timeout certificate was seen
    ///
    /// This tracks timeout certificates to ensure we only extend from
    /// the most recent timeout.
    pub highest_timeout_round: u64,
}

impl SafetyStateData {
    /// Create a new safety state with the given values
    pub fn new(
        epoch: u64,
        last_voted_round: u64,
        preferred_round: u64,
        one_chain_round: u64,
        highest_timeout_round: u64,
    ) -> Self {
        Self {
            epoch,
            last_voted_round,
            preferred_round,
            one_chain_round,
            highest_timeout_round,
        }
    }

    /// Create a new safety state for a new epoch
    ///
    /// This resets the round tracking but preserves the preferred_round
    /// if it's higher.
    pub fn new_epoch(&self, new_epoch: u64) -> Self {
        Self {
            epoch: new_epoch,
            last_voted_round: 0,
            preferred_round: self.preferred_round,
            one_chain_round: 0,
            highest_timeout_round: 0,
        }
    }

    /// Check if a round is safe to vote in
    ///
    /// A round is safe if it's greater than last_voted_round.
    pub fn can_vote_in_round(&self, round: u64) -> bool {
        round >= self.last_voted_round
    }

    /// Update the one_chain_round if the new round is higher
    ///
    /// Returns true if the value was updated.
    pub fn update_one_chain_round(&mut self, new_round: u64) -> bool {
        if new_round > self.one_chain_round {
            self.one_chain_round = new_round;
            true
        } else {
            false
        }
    }

    /// Update the preferred_round if the new round is higher
    ///
    /// Returns true if the value was updated.
    pub fn update_preferred_round(&mut self, new_round: u64) -> bool {
        if new_round > self.preferred_round {
            self.preferred_round = new_round;
            true
        } else {
            false
        }
    }

    /// Update the highest_timeout_round if the new round is higher
    ///
    /// Returns true if the value was updated.
    pub fn update_highest_timeout_round(&mut self, new_round: u64) -> bool {
        if new_round > self.highest_timeout_round {
            self.highest_timeout_round = new_round;
            true
        } else {
            false
        }
    }
}

impl SafetyState for SafetyStateData {
    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn last_voted_round(&self) -> u64 {
        self.last_voted_round
    }

    fn one_chain_round(&self) -> u64 {
        self.one_chain_round
    }

    fn preferred_round(&self) -> u64 {
        self.preferred_round
    }

    fn highest_timeout_round(&self) -> u64 {
        self.highest_timeout_round
    }
}

impl std::fmt::Display for SafetyStateData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SafetyStateData: [\n\
             \tepoch = {},\n\
             \tlast_voted_round = {},\n\
             \tpreferred_round = {},\n\
             \tone_chain_round = {},\n\
             \thighest_timeout_round = {}\n\
             ]",
            self.epoch,
            self.last_voted_round,
            self.preferred_round,
            self.one_chain_round,
            self.highest_timeout_round
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_state_creation() {
        let state = SafetyStateData::new(1, 5, 10, 8, 3);
        assert_eq!(state.epoch(), 1);
        assert_eq!(state.last_voted_round(), 5);
        assert_eq!(state.preferred_round(), 10);
        assert_eq!(state.one_chain_round(), 8);
        assert_eq!(state.highest_timeout_round(), 3);
    }

    #[test]
    fn test_can_vote_in_round() {
        let state = SafetyStateData::new(0, 5, 10, 8, 3);
        assert!(state.can_vote_in_round(5)); // Equal to last_voted_round
        assert!(state.can_vote_in_round(6)); // Greater
        assert!(!state.can_vote_in_round(4)); // Less
    }

    #[test]
    fn test_update_one_chain_round() {
        let mut state = SafetyStateData::new(0, 0, 0, 5, 0);
        assert!(state.update_one_chain_round(10));
        assert_eq!(state.one_chain_round(), 10);
        assert!(!state.update_one_chain_round(8)); // No update
        assert_eq!(state.one_chain_round(), 10);
    }

    #[test]
    fn test_new_epoch() {
        let state = SafetyStateData::new(1, 10, 20, 15, 5);
        let new_state = state.new_epoch(2);
        assert_eq!(new_state.epoch(), 2);
        assert_eq!(new_state.last_voted_round(), 0); // Reset
        assert_eq!(new_state.preferred_round(), 20); // Preserved
        assert_eq!(new_state.one_chain_round(), 0); // Reset
        assert_eq!(new_state.highest_timeout_round(), 0); // Reset
    }

    #[test]
    fn test_display() {
        let state = SafetyStateData::new(1, 5, 10, 8, 3);
        let display = format!("{}", state);
        assert!(display.contains("epoch = 1"));
        assert!(display.contains("last_voted_round = 5"));
    }
}
