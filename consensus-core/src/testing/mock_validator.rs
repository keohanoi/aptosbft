// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use consensus_traits::{Vote, ValidatorVerifier};

use super::{MockBlock, MockVote, MockHash, MockNodeId, MockSignature, MockAggregatedSignature, MockPublicKey};
use consensus_traits::core::{Error, VerifyError};

/// A mock validator for testing.
///
/// This struct represents a validator with an ID, public key, and voting power.
#[derive(Clone, Debug)]
pub struct MockValidator {
    /// The validator's unique identifier
    id: MockHash,

    /// The validator's public key
    public_key: MockPublicKey,

    /// The validator's voting power
    voting_power: u64,
}

impl MockValidator {
    /// Create a new mock validator.
    ///
    /// # Parameters
    ///
    /// * `id` - The validator's unique identifier
    /// * `voting_power` - The validator's voting power
    pub fn new(id: u8, voting_power: u64) -> Self {
        Self {
            id: MockHash(id),
            public_key: MockPublicKey,
            voting_power,
        }
    }

    /// Get the validator's ID.
    pub fn id(&self) -> MockHash {
        self.id
    }

    /// Get the validator's public key.
    pub fn public_key(&self) -> &MockPublicKey {
        &self.public_key
    }

    /// Get the validator's voting power.
    pub fn voting_power(&self) -> u64 {
        self.voting_power
    }
}

/// A set of mock validators for testing.
///
/// This struct manages a collection of validators and provides
/// voting power calculations and verification capabilities.
#[derive(Clone, Debug)]
pub struct MockValidatorSet {
    /// Map from validator ID to validator
    validators: HashMap<MockNodeId, MockValidator>,

    /// Total voting power of all validators
    total_voting_power: u128,

    /// Quorum threshold (2/3 of total voting power + 1)
    quorum_threshold: u128,
}

impl MockValidatorSet {
    /// Create a new mock validator set.
    ///
    /// # Parameters
    ///
    /// * `validators` - The validators in this set
    pub fn new(validators: Vec<MockValidator>) -> Self {
        let mut validator_map = HashMap::new();
        let mut total_power = 0u128;

        for validator in validators {
            let id = validator.id();
            total_power += validator.voting_power() as u128;
            validator_map.insert(id, validator);
        }

        // Quorum is 2/3 of total voting power + 1
        let quorum_threshold = (total_power * 2 / 3) + 1;

        Self {
            validators: validator_map,
            total_voting_power: total_power,
            quorum_threshold,
        }
    }

    /// Get the number of validators in the set.
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if the validator set is empty.
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Get the total voting power of all validators.
    pub fn total_voting_power(&self) -> u128 {
        self.total_voting_power
    }

    /// Get the quorum threshold.
    pub fn quorum_threshold(&self) -> u128 {
        self.quorum_threshold
    }

    /// Get a validator by ID.
    pub fn get_validator(&self, id: MockNodeId) -> Option<&MockValidator> {
        self.validators.get(&id)
    }

    /// Get all validator IDs.
    pub fn validator_ids(&self) -> impl Iterator<Item = &MockNodeId> {
        self.validators.keys()
    }
}

impl ValidatorVerifier<MockBlock, MockVote> for MockValidatorSet {
    fn verify_vote(&self, vote: &MockVote) -> Result<(), Error> {
        // In a real implementation, this would verify the signature
        // For testing, we just check that the voter is in our validator set
        if self.validators.contains_key(&vote.author()) {
            Ok(())
        } else {
            Err(VerifyError::UnknownAuthor.into())
        }
    }

    fn verify_block(&self, _block: &MockBlock) -> Result<(), Error> {
        // In a real implementation, this would verify the block signature
        Ok(())
    }

    fn get_voting_power(&self, validator_id: &MockNodeId) -> Option<u64> {
        self.validators.get(validator_id).map(|v| v.voting_power())
    }

    fn total_voting_power(&self) -> u128 {
        self.total_voting_power
    }

    fn quorum_voting_power(&self) -> u128 {
        self.quorum_threshold
    }

    fn check_voting_power<'a>(
        &self,
        signers: impl Iterator<Item = &'a MockNodeId>,
        check_quorum: bool,
    ) -> Result<(), VerifyError> {
        let mut power = 0u128;
        for signer in signers {
            if let Some(vp) = self.get_voting_power(signer) {
                power += vp as u128;
            }
        }

        if check_quorum && power < self.quorum_threshold {
            return Err(VerifyError::TooLittleVotingPower {
                voting_power: power,
                expected_voting_power: self.quorum_threshold,
            });
        }

        Ok(())
    }

    fn aggregate_signatures<'a>(
        &self,
        _signatures: impl Iterator<Item = &'a MockSignature>,
    ) -> Result<MockAggregatedSignature, Error> {
        Ok(MockAggregatedSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_validator_new() {
        let validator = MockValidator::new(1, 100);

        assert_eq!(validator.id(), MockHash(1));
        assert_eq!(validator.voting_power(), 100);
    }

    #[test]
    fn test_mock_validator_set_new() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
            MockValidator::new(2, 200),
        ];

        let set = MockValidatorSet::new(validators);

        assert_eq!(set.len(), 3);
        assert_eq!(set.total_voting_power(), 450);
        // Quorum should be (450 * 2 / 3) + 1 = 301
        assert_eq!(set.quorum_threshold(), 301);
    }

    #[test]
    fn test_mock_validator_set_quorum_calculation() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 100),
            MockValidator::new(2, 100),
        ];

        let set = MockValidatorSet::new(validators);

        assert_eq!(set.total_voting_power(), 300);
        // Quorum should be (300 * 2 / 3) + 1 = 201
        assert_eq!(set.quorum_threshold(), 201);
    }

    #[test]
    fn test_mock_validator_set_get_validator() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
        ];

        let set = MockValidatorSet::new(validators);

        assert!(set.get_validator(MockHash(0)).is_some());
        assert!(set.get_validator(MockHash(1)).is_some());
        assert!(set.get_validator(MockHash(2)).is_none());
    }

    #[test]
    fn test_mock_validator_set_check_voting_power() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 100),
            MockValidator::new(2, 100),
        ];

        let set = MockValidatorSet::new(validators);

        // 2 validators = 200 voting power, not enough for quorum (201)
        let signers = vec![MockHash(0), MockHash(1)];
        assert!(set.check_voting_power(signers.iter(), true).is_err());

        // 3 validators = 300 voting power, meets quorum
        let signers = vec![MockHash(0), MockHash(1), MockHash(2)];
        assert!(set.check_voting_power(signers.iter(), true).is_ok());

        // Without quorum check, should always succeed
        let signers = vec![MockHash(0)];
        assert!(set.check_voting_power(signers.iter(), false).is_ok());
    }

    #[test]
    fn test_mock_validator_set_verify_vote() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 100),
        ];

        let set = MockValidatorSet::new(validators);

        // Vote from known validator should pass
        let vote = MockVote::new(MockHash(0), MockHash(0), 0);
        assert!(set.verify_vote(&vote).is_ok());

        // Vote from unknown validator should fail
        let vote = MockVote::new(MockHash(99), MockHash(0), 0);
        assert!(set.verify_vote(&vote).is_err());
    }

    #[test]
    fn test_mock_validator_public_key() {
        let validator = MockValidator::new(1, 100);
        let _pk = validator.public_key();
        // Just check the method works
    }

    #[test]
    fn test_mock_validator_set_is_empty() {
        let validators = vec![];
        let set = MockValidatorSet::new(validators);
        assert!(set.is_empty());

        let validators = vec![MockValidator::new(0, 100)];
        let set = MockValidatorSet::new(validators);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_mock_validator_set_validator_ids() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
        ];

        let set = MockValidatorSet::new(validators);
        let ids: Vec<_> = set.validator_ids().collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_mock_validator_set_verify_block() {
        let validators = vec![MockValidator::new(0, 100)];
        let set = MockValidatorSet::new(validators);

        let block = MockBlock::genesis();
        assert!(set.verify_block(&block).is_ok());
    }

    #[test]
    fn test_mock_validator_set_get_voting_power() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
        ];

        let set = MockValidatorSet::new(validators);
        assert_eq!(set.get_voting_power(&MockHash(0)), Some(100));
        assert_eq!(set.get_voting_power(&MockHash(1)), Some(150));
        assert_eq!(set.get_voting_power(&MockHash(99)), None);
    }

    #[test]
    fn test_mock_validator_set_total_voting_power_trait() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
        ];

        let set = MockValidatorSet::new(validators);
        assert_eq!(ValidatorVerifier::total_voting_power(&set), 250);
    }

    #[test]
    fn test_mock_validator_set_quorum_voting_power() {
        let validators = vec![
            MockValidator::new(0, 100),
            MockValidator::new(1, 150),
        ];

        let set = MockValidatorSet::new(validators);
        // Quorum should be (250 * 2 / 3) + 1 = 167
        assert_eq!(ValidatorVerifier::quorum_voting_power(&set), 167);
    }

    #[test]
    fn test_mock_validator_set_aggregate_signatures() {
        let validators = vec![MockValidator::new(0, 100)];
        let set = MockValidatorSet::new(validators);

        let sigs = vec![MockSignature(1), MockSignature(2)];
        let aggregated = ValidatorVerifier::aggregate_signatures(&set, sigs.iter());
        assert!(aggregated.is_ok());
    }
}
