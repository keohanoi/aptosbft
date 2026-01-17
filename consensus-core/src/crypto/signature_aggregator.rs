// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Signature aggregation for quorum certificate formation.
//!
//! This module provides utilities for aggregating signatures from multiple
//! validators and tracking voting power to determine when a quorum is reached.

use consensus_traits::{Block, Signature, ValidatorVerifier, Vote};
use std::collections::HashMap;

/// Aggregates signatures and tracks voting power for quorum formation.
///
/// This struct collects votes from validators, tracks their voting power,
/// and can determine when a quorum has been reached.
///
/// # Type Parameters
///
/// * `V` - Vote type, must implement the [`Vote`] trait
///
/// # Example
///
/// ```ignore
/// use consensus_core::crypto::SignatureAggregator;
/// use consensus_traits::{Block, Vote, ValidatorVerifier};
///
/// let mut aggregator = SignatureAggregator::<MyVote>::new();
///
/// // Add votes from validators
/// for vote in votes {
///     let voting_power = verifier.get_voting_power(&vote.author()).unwrap_or(0);
///     aggregator.add_signature(vote.author(), vote.signature().clone(), voting_power);
/// }
///
/// // Check if we have quorum
/// if aggregator.check_quorum(&verifier).is_ok() {
///     let aggregated_sig = aggregator.aggregate(&verifier)?;
///     // Create quorum certificate
/// }
/// ```
#[derive(Clone, Debug)]
pub struct SignatureAggregator<V>
where
    V: Vote,
{
    /// Map from validator ID to their signature
    signatures: HashMap<V::NodeId, V::Signature>,

    /// Accumulated voting power from all signers
    voting_power: u128,

    /// Number of unique signers
    signer_count: usize,
}

impl<V> SignatureAggregator<V>
where
    V: Vote,
{
    /// Create a new empty signature aggregator.
    pub fn new() -> Self {
        Self {
            signatures: HashMap::new(),
            voting_power: 0,
            signer_count: 0,
        }
    }

    /// Add a signature from a validator with their voting power.
    ///
    /// This method only adds the signature if this validator hasn't signed yet
    /// (prevents duplicate votes from the same validator).
    ///
    /// # Parameters
    ///
    /// * `validator_id` - The unique identifier of the validator
    /// * `signature` - The validator's signature
    /// * `voting_power` - The voting power of this validator
    pub fn add_signature(
        &mut self,
        validator_id: V::NodeId,
        signature: V::Signature,
        voting_power: u64,
    ) {
        // Only add if we haven't seen this validator
        if !self.signatures.contains_key(&validator_id) {
            self.voting_power += voting_power as u128;
            self.signer_count += 1;
            self.signatures.insert(validator_id, signature);
        }
    }

    /// Get the current accumulated voting power.
    pub fn voting_power(&self) -> u128 {
        self.voting_power
    }

    /// Get the number of unique signers.
    pub fn signer_count(&self) -> usize {
        self.signer_count
    }

    /// Get an iterator over all signer IDs.
    pub fn signers(&self) -> impl Iterator<Item = &V::NodeId> {
        self.signatures.keys()
    }

    /// Get an iterator over all signatures.
    pub fn signatures(&self) -> impl Iterator<Item = &V::Signature> {
        self.signatures.values()
    }

    /// Check if the current signers meet the quorum threshold.
    ///
    /// # Parameters
    ///
    /// * `verifier` - The validator verifier to check quorum against
    ///
    /// # Returns
    ///
    /// * `Ok(())` if quorum is met
    /// * `Err(VerifyError::TooLittleVotingPower)` if insufficient voting power
    pub fn check_quorum<B, VV>(
        &self,
        verifier: &VV,
    ) -> Result<(), consensus_traits::core::VerifyError>
    where
        B: Block,
        VV: ValidatorVerifier<B, V>,
    {
        verifier.check_voting_power(self.signers(), true)
    }

    /// Aggregate all signatures into a single signature.
    ///
    /// # Parameters
    ///
    /// * `verifier` - The validator verifier to use for aggregation
    ///
    /// # Returns
    ///
    /// * The aggregated signature
    /// * `Err(VerifyError)` if aggregation fails
    pub fn aggregate<B, VV>(
        &self,
        verifier: &VV,
    ) -> Result<<V::Signature as Signature>::Aggregated, consensus_traits::core::Error>
    where
        B: Block,
        VV: ValidatorVerifier<B, V>,
    {
        verifier.aggregate_signatures(self.signatures())
    }

    /// Clear all signatures and reset voting power.
    pub fn clear(&mut self) {
        self.signatures.clear();
        self.voting_power = 0;
        self.signer_count = 0;
    }

    /// Check if a specific validator has signed.
    pub fn has_signed(&self, validator_id: &V::NodeId) -> bool {
        self.signatures.contains_key(validator_id)
    }

    /// Get the signature for a specific validator, if present.
    pub fn get_signature(&self, validator_id: &V::NodeId) -> Option<&V::Signature> {
        self.signatures.get(validator_id)
    }
}

impl<V> Default for SignatureAggregator<V>
where
    V: Vote,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_traits::core::Error;
    use std::sync::Arc;

    // Mock types for testing
    #[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
    struct TestNodeId(u8);

    impl consensus_traits::core::Hash for TestNodeId {
        fn zero() -> Self {
            TestNodeId(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.is_empty() {
                return Err(anyhow::anyhow!("Empty bytes").into());
            }
            Ok(TestNodeId(bytes[0]))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    impl consensus_traits::core::NodeId for TestNodeId {
        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.is_empty() {
                return Err(anyhow::anyhow!("Empty bytes").into());
            }
            Ok(TestNodeId(bytes[0]))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestHash;

    impl consensus_traits::core::Hash for TestHash {
        fn zero() -> Self {
            TestHash
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(TestHash)
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestSignature(u8);

    #[derive(Clone, Copy, Debug)]
    struct TestAggregatedSig;

    impl Signature for TestSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = TestPublicKey;
        type Aggregated = TestAggregatedSig;

        fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
            Ok(())
        }

        fn to_bytes(&self) -> Vec<u8> {
            vec![self.0]
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.is_empty() {
                return Err(anyhow::anyhow!("Empty bytes").into());
            }
            Ok(TestSignature(bytes[0]))
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
            Ok(TestAggregatedSig)
        }
    }

    #[derive(Clone, Debug)]
    struct TestPublicKey;

    impl consensus_traits::core::PublicKey for TestPublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(TestPublicKey)
        }
    }

    #[derive(Clone, Debug)]
    struct TestVote {
        author: TestNodeId,
        signature: TestSignature,
    }

    impl Vote for TestVote {
        type Block = TestBlock;
        type Hash = TestHash;
        type VoteData = TestVoteData;
        type NodeId = TestNodeId;
        type LedgerInfo = TestLedgerInfo;
        type Signature = TestSignature;

        fn vote_data(&self) -> &Self::VoteData {
            static DATA: TestVoteData = TestVoteData;
            &DATA
        }

        fn author(&self) -> Self::NodeId {
            self.author
        }

        fn ledger_info(&self) -> &Self::LedgerInfo {
            static INFO: TestLedgerInfo = TestLedgerInfo;
            &INFO
        }

        fn signature(&self) -> &Self::Signature {
            &self.signature
        }

        fn block_id(&self) -> Self::Hash {
            TestHash
        }

        fn round(&self) -> u64 {
            0
        }

        fn verify<B, VV>(&self, _verifier: &VV) -> Result<(), anyhow::Error>
        where
            B: consensus_traits::Block,
            VV: consensus_traits::ValidatorVerifier<B, Self>,
        {
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct TestVoteData;

    impl consensus_traits::block::VoteData for TestVoteData {
        type BlockMetadata = TestMetadata;

        fn proposed_block(&self) -> &Self::BlockMetadata {
            static META: TestMetadata = TestMetadata;
            &META
        }

        fn parent_block(&self) -> &Self::BlockMetadata {
            static META: TestMetadata = TestMetadata;
            &META
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestMetadata;

    impl consensus_traits::block::BlockMetadata for TestMetadata {
        type QuorumCert = TestQuorumCert;
        type NodeId = TestNodeId;
        type Hash = TestHash;

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn author(&self) -> Self::NodeId {
            TestNodeId(0)
        }

        fn parent_id(&self) -> Self::Hash {
            TestHash
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestQuorumCert;

    impl consensus_traits::block::QuorumCertificate for TestQuorumCert {
        type BlockMetadata = TestMetadata;
        type Hash = TestHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            static META: TestMetadata = TestMetadata;
            &META
        }

        fn block_id(&self) -> Self::Hash {
            TestHash
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[Arc<V>],
            _aggregated_signature: <V::Signature as Signature>::Aggregated,
        ) -> Result<Self, Error>
        where
            B: consensus_traits::Block,
            V: consensus_traits::Vote<Block = B>,
        {
            Ok(TestQuorumCert)
        }
    }

    #[derive(Clone, Debug)]
    struct TestLedgerInfo;

    impl consensus_traits::block::LedgerInfo for TestLedgerInfo {
        type CommitInfo = TestCommitInfo;
        type Hash = TestHash;

        fn commit_info(&self) -> &Self::CommitInfo {
            static INFO: TestCommitInfo = TestCommitInfo;
            &INFO
        }

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn accumulated_state(&self) -> &Self::Hash {
            static HASH: TestHash = TestHash;
            &HASH
        }
    }

    #[derive(Clone, Debug)]
    struct TestCommitInfo;

    impl consensus_traits::block::CommitInfo for TestCommitInfo {
        type Hash = TestHash;

        fn block_id(&self) -> Self::Hash {
            TestHash
        }

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn version(&self) -> u64 {
            0
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestBlock;

    impl consensus_traits::Block for TestBlock {
        type Transaction = TestTransaction;
        type Metadata = TestMetadata;
        type Signature = TestSignature;
        type Hash = TestHash;

        fn id(&self) -> Self::Hash {
            TestHash
        }

        fn parent_id(&self) -> Self::Hash {
            TestHash
        }

        fn transactions(&self) -> &[Self::Transaction] {
            &[]
        }

        fn metadata(&self) -> &Self::Metadata {
            static META: TestMetadata = TestMetadata;
            &META
        }

        fn signature(&self) -> Option<&Self::Signature> {
            static SIG: TestSignature = TestSignature(0);
            Some(&SIG)
        }

        fn verify_signature(&self) -> Result<(), Error> {
            Ok(())
        }

        fn is_genesis(&self) -> bool {
            false
        }

        fn is_nil(&self) -> bool {
            false
        }

        fn new_genesis() -> Self {
            TestBlock
        }
    }

    #[derive(Clone, Debug)]
    struct TestTransaction;

    impl consensus_traits::block::Transaction for TestTransaction {
        type Hash = TestHash;

        fn hash(&self) -> Self::Hash {
            TestHash
        }

        fn size(&self) -> usize {
            0
        }
    }

    // Mock verifier that checks voting power
    struct TestVerifier {
        total_power: u128,
        quorum_threshold: u128,
    }

    impl ValidatorVerifier<TestBlock, TestVote> for TestVerifier {
        fn verify_vote(&self, _vote: &TestVote) -> Result<(), Error> {
            Ok(())
        }

        fn verify_block(&self, _block: &TestBlock) -> Result<(), Error> {
            Ok(())
        }

        fn get_voting_power(&self, _validator_id: &TestNodeId) -> Option<u64> {
            Some(100)
        }

        fn total_voting_power(&self) -> u128 {
            self.total_power
        }

        fn quorum_voting_power(&self) -> u128 {
            self.quorum_threshold
        }

        fn check_voting_power<'a>(
            &self,
            signers: impl Iterator<Item = &'a TestNodeId>,
            check_quorum: bool,
        ) -> Result<(), consensus_traits::core::VerifyError> {
            let mut power = 0u128;
            for _signer in signers {
                power += 100;
            }

            if check_quorum && power < self.quorum_threshold {
                return Err(consensus_traits::core::VerifyError::TooLittleVotingPower {
                    voting_power: power,
                    expected_voting_power: self.quorum_threshold,
                });
            }

            Ok(())
        }

        fn aggregate_signatures<'a>(
            &self,
            _signatures: impl Iterator<Item = &'a TestSignature>,
        ) -> Result<TestAggregatedSig, Error> {
            Ok(TestAggregatedSig)
        }
    }

    #[test]
    fn test_signature_aggregator_new() {
        let aggregator = SignatureAggregator::<TestVote>::new();
        assert_eq!(aggregator.voting_power(), 0);
        assert_eq!(aggregator.signer_count(), 0);
        assert!(!aggregator.has_signed(&TestNodeId(1)));
    }

    #[test]
    fn test_signature_aggregator_add_signature() {
        let mut aggregator = SignatureAggregator::<TestVote>::new();

        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);

        assert_eq!(aggregator.voting_power(), 100);
        assert_eq!(aggregator.signer_count(), 1);
        assert!(aggregator.has_signed(&TestNodeId(1)));
        assert_eq!(aggregator.get_signature(&TestNodeId(1)), Some(&TestSignature(1)));
    }

    #[test]
    fn test_signature_aggregator_duplicate_signer() {
        let mut aggregator = SignatureAggregator::<TestVote>::new();

        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);
        aggregator.add_signature(TestNodeId(1), TestSignature(2), 100);

        // Duplicate should not increase voting power
        assert_eq!(aggregator.voting_power(), 100);
        assert_eq!(aggregator.signer_count(), 1);
    }

    #[test]
    fn test_signature_aggregator_multiple_signers() {
        let mut aggregator = SignatureAggregator::<TestVote>::new();

        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);
        aggregator.add_signature(TestNodeId(2), TestSignature(2), 150);
        aggregator.add_signature(TestNodeId(3), TestSignature(3), 200);

        assert_eq!(aggregator.voting_power(), 450);
        assert_eq!(aggregator.signer_count(), 3);
    }

    #[test]
    fn test_signature_aggregator_check_quorum() {
        let verifier = TestVerifier {
            total_power: 400,
            quorum_threshold: 267,
        };

        let mut aggregator = SignatureAggregator::<TestVote>::new();

        // Add 2 signers (200 voting power) - not enough for quorum
        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);
        aggregator.add_signature(TestNodeId(2), TestSignature(2), 100);

        assert!(aggregator.check_quorum(&verifier).is_err());

        // Add third signer (300 voting power) - now we have quorum
        aggregator.add_signature(TestNodeId(3), TestSignature(3), 100);

        assert!(aggregator.check_quorum(&verifier).is_ok());
    }

    #[test]
    fn test_signature_aggregator_clear() {
        let mut aggregator = SignatureAggregator::<TestVote>::new();

        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);
        aggregator.add_signature(TestNodeId(2), TestSignature(2), 100);

        aggregator.clear();

        assert_eq!(aggregator.voting_power(), 0);
        assert_eq!(aggregator.signer_count(), 0);
        assert!(!aggregator.has_signed(&TestNodeId(1)));
    }

    #[test]
    fn test_signature_aggregator_aggregate() {
        let verifier = TestVerifier {
            total_power: 400,
            quorum_threshold: 267,
        };

        let mut aggregator = SignatureAggregator::<TestVote>::new();

        aggregator.add_signature(TestNodeId(1), TestSignature(1), 100);
        aggregator.add_signature(TestNodeId(2), TestSignature(2), 100);
        aggregator.add_signature(TestNodeId(3), TestSignature(3), 100);

        let aggregated = aggregator.aggregate(&verifier).unwrap();
        // Just verify it returns Ok, the actual aggregated signature type is test-specific
    }

    #[test]
    fn test_signature_aggregator_default() {
        let aggregator = SignatureAggregator::<TestVote>::default();
        assert_eq!(aggregator.voting_power(), 0);
        assert_eq!(aggregator.signer_count(), 0);
    }
}
