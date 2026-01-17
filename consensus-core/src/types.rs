// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Common types used throughout the consensus algorithm

use serde::{Deserialize, Serialize};

/// A round number in the consensus protocol
///
/// Rounds increment monotonically within an epoch. Each round has a designated proposer
/// who is responsible for creating a block proposal for that round.
pub type Round = u64;

/// An epoch number in the consensus protocol
///
/// Epochs represent major reconfiguration boundaries. Validator set changes, protocol upgrades,
/// and other significant changes happen at epoch boundaries.
pub type Epoch = u64;

/// The genesis round number (always 0)
pub const GENESIS_ROUND: Round = 0;

/// The genesis epoch number (always 0)
pub const GENESIS_EPOCH: Epoch = 0;

/// Reason for a round timeout
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeoutReason {
    /// The round timed out without receiving enough votes
    NoQuorum,

    /// The round timed out due to a slow leader
    SlowLeader,

    /// The round timed out due to network issues
    NetworkDelay,

    /// The round timed out due to resource constraints
    ResourceConstraints,
}

impl std::fmt::Display for TimeoutReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeoutReason::NoQuorum => write!(f, "no_quorum"),
            TimeoutReason::SlowLeader => write!(f, "slow_leader"),
            TimeoutReason::NetworkDelay => write!(f, "network_delay"),
            TimeoutReason::ResourceConstraints => write!(f, "resource_constraints"),
        }
    }
}

/// Status of a vote in the consensus protocol
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteStatus {
    /// The vote was accepted and added to pending votes
    Accepted,

    /// The vote was rejected (duplicate, invalid, or from wrong epoch)
    Rejected,

    /// The vote completed a quorum certificate
    QuorumCompleted,

    /// The vote was for an old round and ignored
    OldRound,

    /// The vote was for a future round and buffered
    FutureRound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_reason_display() {
        assert_eq!(TimeoutReason::NoQuorum.to_string(), "no_quorum");
        assert_eq!(TimeoutReason::SlowLeader.to_string(), "slow_leader");
        assert_eq!(TimeoutReason::NetworkDelay.to_string(), "network_delay");
        assert_eq!(TimeoutReason::ResourceConstraints.to_string(), "resource_constraints");
    }

    #[test]
    fn test_constants() {
        assert_eq!(GENESIS_ROUND, 0);
        assert_eq!(GENESIS_EPOCH, 0);
    }

    #[test]
    fn test_vote_status_variants() {
        // Test that all VoteStatus variants can be constructed
        let _ = VoteStatus::Accepted;
        let _ = VoteStatus::Rejected;
        let _ = VoteStatus::QuorumCompleted;
        let _ = VoteStatus::OldRound;
        let _ = VoteStatus::FutureRound;
    }
}
