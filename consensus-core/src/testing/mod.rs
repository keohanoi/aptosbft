// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Testing utilities for consensus-core.
//!
//! This module provides mock implementations and utilities for testing
//! consensus functionality without needing a full blockchain implementation.

mod mock_types;
mod mock_validator;
mod mock_vote;
mod mock_block;

pub use mock_types::{MockHash, MockNodeId, MockSignature, MockPublicKey, MockAggregatedSignature};
pub use mock_validator::{MockValidator, MockValidatorSet};
pub use mock_vote::{MockVote, MockVoteData, MockLedgerInfo, MockCommitInfo};
pub use mock_block::{MockBlock, MockBlockMetadata, MockQuorumCert, MockTransaction};

use std::sync::Arc;

/// Create a set of mock validators with voting power.
///
/// This is a convenience function for creating a set of validators
/// with the specified voting powers for testing.
///
/// # Parameters
///
/// * `voting_powers` - Slice of voting powers for each validator
///
/// # Returns
///
/// * A `MockValidatorSet` containing all validators
///
/// # Example
///
/// ```ignore
/// use consensus_core::testing::make_validator_set;
///
/// // Create 4 validators with varying voting power
/// let validators = make_validator_set(&[100, 150, 200, 250]);
/// assert_eq!(validators.total_voting_power(), 700);
/// ```
pub fn make_validator_set(voting_powers: &[u64]) -> MockValidatorSet {
    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    MockValidatorSet::new(validators)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_validator_set() {
        let validators = make_validator_set(&[100, 150, 200, 250]);

        assert_eq!(validators.len(), 4);
        assert_eq!(validators.total_voting_power(), 700);
    }

    #[test]
    fn test_make_validator_set_empty() {
        let validators = make_validator_set(&[]);

        assert_eq!(validators.len(), 0);
        assert_eq!(validators.total_voting_power(), 0);
    }

    #[test]
    fn test_make_validator_set_single() {
        let validators = make_validator_set(&[1000]);

        assert_eq!(validators.len(), 1);
        assert_eq!(validators.total_voting_power(), 1000);
    }
}
