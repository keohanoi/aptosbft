// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Property-based tests for consensus safety properties.
//!
//! This module tests critical safety properties of the consensus algorithm,
//! such as quorum thresholds, voting power calculations, and signature aggregation.

use consensus_traits::ValidatorVerifier;

use consensus_core::crypto::SignatureAggregator;
use consensus_core::testing::{MockVote, MockHash, MockSignature, MockValidator, MockValidatorSet};

/// Test that quorum threshold is correctly calculated.
///
/// Property: Quorum should be (2/3 * total_voting_power) + 1
#[test]
fn test_quorum_threshold_calculation() {
    // Test with various validator configurations
    let test_cases = vec![
        (vec![100u64; 3], 300, 201),   // 3 validators, 300 total, 201 quorum
        (vec![100u64; 4], 400, 267),   // 4 validators, 400 total, 267 quorum
        (vec![50u64; 10], 500, 334),    // 10 validators, 500 total, 334 quorum
        (vec![100u64; 10], 1000, 667), // 10 validators, 1000 total, 667 quorum
    ];

    for (voting_powers, expected_total, expected_quorum) in test_cases {
        let validators = voting_powers
            .iter()
            .enumerate()
            .map(|(i, &power)| MockValidator::new(i as u8, power))
            .collect();

        let set = MockValidatorSet::new(validators);

        assert_eq!(set.total_voting_power(), expected_total);
        assert_eq!(set.quorum_threshold(), expected_quorum);
    }
}

/// Test that quorum cannot be reached with insufficient voting power.
///
/// Property: Any subset of validators with < 2/3 voting power cannot form quorum
#[test]
fn test_quorum_rejection_insufficient_power() {
    let voting_powers = vec![100u64; 4]; // 400 total, quorum 267

    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    let verifier = MockValidatorSet::new(validators);

    // Test various insufficient combinations
    let insufficient_combinations = vec![
        vec![0],                    // 100 voting power
        vec![0, 1],                 // 200 voting power
    ];

    for signers in insufficient_combinations {
        let signer_ids: Vec<MockHash> = signers.iter().map(|&i| MockHash(i)).collect();

        let result = verifier.check_voting_power(signer_ids.iter(), true);
        assert!(result.is_err(), "Should reject insufficient voting power");
    }
}

/// Test that quorum can be reached with sufficient voting power.
///
/// Property: Any subset of validators with ≥ 2/3 voting power can form quorum
#[test]
fn test_quorum_acceptance_sufficient_power() {
    let voting_powers = vec![100u64; 4]; // 400 total, quorum 267

    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    let verifier = MockValidatorSet::new(validators);

    // Test various sufficient combinations
    let sufficient_combinations = vec![
        vec![0, 1, 2],              // 300 voting power
        vec![0, 1, 2, 3],           // 400 voting power (all validators)
    ];

    for signers in sufficient_combinations {
        let signer_ids: Vec<MockHash> = signers.iter().map(|&i| MockHash(i)).collect();

        let result = verifier.check_voting_power(signer_ids.iter(), true);
        assert!(result.is_ok(), "Should accept sufficient voting power");
    }
}

/// Test that duplicate votes from the same validator are not counted twice.
///
/// Property: A validator can only contribute their voting power once per round
#[test]
fn test_duplicate_votes_not_counted() {
    let voting_powers = vec![100u64; 3];

    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    let verifier = MockValidatorSet::new(validators);

    let mut aggregator = SignatureAggregator::<MockVote>::new();

    let validator_id = MockHash(0);
    let sig1 = MockSignature(1);
    let sig2 = MockSignature(2);

    // Add first vote
    aggregator.add_signature(validator_id, sig1, 100);

    assert_eq!(aggregator.voting_power(), 100);
    assert_eq!(aggregator.signer_count(), 1);

    // Try to add duplicate from same validator with different signature
    aggregator.add_signature(validator_id, sig2, 100);

    // Should not increase
    assert_eq!(aggregator.voting_power(), 100);
    assert_eq!(aggregator.signer_count(), 1);
    assert!(aggregator.has_signed(&validator_id));
}

/// Test that voting power is correctly accumulated across validators.
///
/// Property: Total voting power = sum of individual validator voting powers
#[test]
fn test_voting_power_accumulation() {
    let mut aggregator = SignatureAggregator::<MockVote>::new();

    let validators = vec![
        (MockHash(0), MockSignature(0), 150u64),
        (MockHash(1), MockSignature(1), 200u64),
        (MockHash(2), MockSignature(2), 250u64),
    ];

    for (id, sig, power) in validators {
        aggregator.add_signature(id, sig, power);
    }

    assert_eq!(aggregator.voting_power(), 600); // 150 + 200 + 250
    assert_eq!(aggregator.signer_count(), 3);
}

/// Test that empty aggregator reports zero voting power.
///
/// Property: No votes = zero voting power
#[test]
fn test_empty_aggregator_zero_power() {
    let aggregator = SignatureAggregator::<MockVote>::new();

    assert_eq!(aggregator.voting_power(), 0);
    assert_eq!(aggregator.signer_count(), 0);
    assert!(!aggregator.has_signed(&MockHash(0)));
}

/// Test quorum threshold edge cases.
///
/// Property: Exactly 2/3 voting power should not be enough (need +1)
#[test]
fn test_quorum_threshold_edge_cases() {
    // Test with 300 total power (quorum = 201)
    let voting_powers = vec![100u64; 3];
    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    let verifier = MockValidatorSet::new(validators);

    // Exactly 2/3 (200) should fail
    let signers: Vec<MockHash> = vec![MockHash(0), MockHash(1)].iter().copied().collect();
    assert!(verifier.check_voting_power(signers.iter(), true).is_err());

    // 2/3 + 1 (201) should succeed
    let signers: Vec<MockHash> = vec![MockHash(0), MockHash(1), MockHash(2)].iter().copied().collect();
    assert!(verifier.check_voting_power(signers.iter(), true).is_ok());
}

/// Test that validators with zero voting power don't contribute to quorum.
///
/// Property: Zero voting power validators should not count toward quorum
#[test]
fn test_zero_voting_power_validators() {
    let mut aggregator = SignatureAggregator::<MockVote>::new();

    // Add validator with zero power
    aggregator.add_signature(MockHash(0), MockSignature(0), 0);
    aggregator.add_signature(MockHash(1), MockSignature(1), 100);

    assert_eq!(aggregator.voting_power(), 100); // Only the second vote counts
    assert_eq!(aggregator.signer_count(), 2);
}

/// Test that signature aggregation preserves all signers.
///
/// Property: All unique signers should be trackable
#[test]
fn test_signature_aggregation_preserves_signers() {
    let mut aggregator = SignatureAggregator::<MockVote>::new();

    let validators = vec![
        (MockHash(0), MockSignature(0), 100u64),
        (MockHash(1), MockSignature(1), 150u64),
        (MockHash(2), MockSignature(2), 200u64),
    ];

    for (id, sig, power) in validators {
        aggregator.add_signature(id, sig, power);
    }

    let signers: Vec<_> = aggregator.signers().collect();
    assert_eq!(signers.len(), 3);
    assert!(signers.iter().any(|s| s.0 == 0));
    assert!(signers.iter().any(|s| s.0 == 1));
    assert!(signers.iter().any(|s| s.0 == 2));
}

/// Test property: Quorum calculation is monotonic.
///
/// Property: Adding more validators should never decrease quorum threshold
#[test]
fn test_quorum_threshold_monotonic() {
    for count in 1..=20 {
        let voting_powers = vec![100u64; count];
        let validators = voting_powers
            .iter()
            .enumerate()
            .map(|(i, &power)| MockValidator::new(i as u8, power))
            .collect();

        let set = MockValidatorSet::new(validators);
        let current_quorum = set.quorum_threshold();

        if count > 1 {
            let prev_powers = vec![100u64; count - 1];
            let prev_validators = prev_powers
                .iter()
                .enumerate()
                .map(|(i, &power)| MockValidator::new(i as u8, power))
                .collect();

            let prev_set = MockValidatorSet::new(prev_validators);
            let prev_quorum = prev_set.quorum_threshold();

            assert!(current_quorum >= prev_quorum,
                "Quorum threshold should be monotonically increasing");
        }
    }
}

/// Test property: Voting power is bounded by total.
///
/// Property: No subset can have more voting power than the total
#[test]
fn test_voting_power_bounded_by_total() {
    let voting_powers = vec![100u64, 150, 200, 250];
    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect();

    let set = MockValidatorSet::new(validators);
    let total = set.total_voting_power();

    // Any subset should have ≤ total voting power
    let mut aggregator = SignatureAggregator::<MockVote>::new();

    for (idx, &power) in voting_powers.iter().enumerate() {
        let id = MockHash(idx as u8);
        aggregator.add_signature(id, MockSignature(idx as u8), power);

        assert!(aggregator.voting_power() <= total,
            "Subset voting power cannot exceed total");
    }
}

/// Test safety: Single validator cannot form quorum alone.
///
/// Property: No single validator should be able to form quorum by themselves
/// (unless they have > 2/3 of all voting power, which is a degenerate case)
#[test]
fn test_single_validator_cannot_form_quorum() {
    // Test with distributed power
    let voting_powers = vec![100u64, 150, 200, 250]; // 700 total, quorum 467
    let validators = voting_powers
        .iter()
        .enumerate()
        .map(|(i, &power)| MockValidator::new(i as u8, power))
        .collect::<Vec<_>>();

    let verifier = MockValidatorSet::new(validators.clone());

    // Each single validator should not be able to form quorum
    for i in 0..voting_powers.len() {
        let signers = vec![MockHash(i as u8)];
        let result = verifier.check_voting_power(signers.iter(), true);
        assert!(result.is_err(), "Single validator should not form quorum");
    }
}
