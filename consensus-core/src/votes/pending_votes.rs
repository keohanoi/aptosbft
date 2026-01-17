// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use consensus_traits::{Block, ValidatorVerifier, Vote};
use std::collections::HashMap;
use std::sync::Arc;

use crate::crypto::SignatureAggregator;
use crate::types::Round;

/// Result of adding a vote to pending votes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoteReceptionResult {
    /// The vote was accepted and added to pending votes
    Accepted,

    /// The vote was rejected (duplicate, invalid, or from wrong epoch)
    Rejected(VoteStatus),

    /// The vote completed a quorum certificate
    QuorumCompleted,

    /// The vote was for an old round and ignored
    OldRound,

    /// The vote was for a future round and buffered
    FutureRound,
}

/// Status of a vote
pub use crate::types::VoteStatus;

/// Tracks pending votes for blocks in various rounds
///
/// This struct collects votes for different blocks and rounds, and
/// can detect when a quorum has been reached for a particular block.
#[derive(Clone, Debug)]
pub struct PendingVotes<B, V>
where
    B: Block,
    V: Vote<Block = B>,
{
    /// Map from (round, block_id) to a map of validator -> vote
    votes: HashMap<(Round, <V as Vote>::Hash), HashMap<<V as Vote>::Signature, Arc<V>>>,

    /// The current round
    current_round: Round,

    /// Maximum number of rounds to keep votes for
    max_rounds: u64,
}

impl<B, V> PendingVotes<B, V>
where
    B: Block,
    V: Vote<Block = B>,
{
    /// Create a new PendingVotes tracker
    pub fn new(max_rounds: u64) -> Self {
        Self {
            votes: HashMap::new(),
            current_round: 0,
            max_rounds,
        }
    }

    /// Set the current round
    pub fn set_round(&mut self, round: Round) {
        self.current_round = round;
        self.cleanup_old_votes();
    }

    /// Add a vote to the pending votes
    ///
    /// Returns a VoteReceptionResult indicating what happened with the vote.
    pub fn add_vote<VV>(
        &mut self,
        vote: Arc<V>,
        verifier: &VV,
    ) -> VoteReceptionResult
    where
        VV: ValidatorVerifier<B, V>,
    {
        let round = vote.round();
        let block_id = vote.block_id();

        // Check if vote is for an old round
        if round < self.current_round {
            return VoteReceptionResult::OldRound;
        }

        // Convert Vote::Hash to Block::Hash (they should be the same in practice)
        // For now, we'll use the block_id from the vote directly
        let key = (round, block_id);

        // Check for duplicate vote
        if let Some(round_votes) = self.votes.get(&key) {
            let signature = vote.signature();
            if round_votes.contains_key(signature) {
                return VoteReceptionResult::Rejected(VoteStatus::Rejected);
            }
        }

        // Add the vote
        self.votes
            .entry(key)
            .or_insert_with(HashMap::new)
            .insert(vote.signature().clone(), vote.clone());

        // Check if we have a quorum
        if self.check_quorum(&key, verifier) {
            return VoteReceptionResult::QuorumCompleted;
        }

        VoteReceptionResult::Accepted
    }

    /// Get all votes for a specific block in a specific round
    pub fn get_votes(
        &self,
        round: Round,
        block_id: <V as Vote>::Hash,
    ) -> Vec<&Arc<V>> {
        self.votes
            .get(&(round, block_id))
            .map(|votes| votes.values().collect())
            .unwrap_or_default()
    }

    /// Get the number of votes for a specific block
    pub fn vote_count(&self, round: Round, block_id: <V as Vote>::Hash) -> usize {
        self.votes
            .get(&(round, block_id))
            .map(|votes| votes.len())
            .unwrap_or(0)
    }

    /// Remove all votes for a specific round
    pub fn remove_round(&mut self, round: Round) {
        self.votes.retain(|(r, _), _| *r != round);
    }

    /// Check if there's a quorum for a specific block
    fn check_quorum<VV>(
        &self,
        key: &(Round, <V as Vote>::Hash),
        verifier: &VV,
    ) -> bool
    where
        VV: ValidatorVerifier<B, V>,
    {
        if let Some(votes) = self.votes.get(key) {
            // Build signature aggregator from all votes
            let mut aggregator = SignatureAggregator::<V>::new();

            for (_signature, vote) in votes.iter() {
                let author = vote.author();

                // Get voting power for this validator
                if let Some(voting_power) = verifier.get_voting_power(&author) {
                    aggregator.add_signature(author, vote.signature().clone(), voting_power);
                }
            }

            // Check if we have quorum
            aggregator.check_quorum(verifier).is_ok()
        } else {
            false
        }
    }

    /// Clean up votes from old rounds
    fn cleanup_old_votes(&mut self) {
        let cutoff_round = self.current_round.saturating_sub(self.max_rounds);
        self.votes.retain(|(round, _), _| *round >= cutoff_round);
    }

    /// Get the total number of pending votes across all rounds and blocks
    pub fn total_votes(&self) -> usize {
        self.votes.values().map(|votes| votes.len()).sum()
    }

    /// Get the aggregated voting power for a specific block in a round.
    ///
    /// # Parameters
    ///
    /// * `round` - The round number
    /// * `block_id` - The hash of the block
    /// * `verifier` - The validator verifier to get voting power from
    ///
    /// # Returns
    ///
    /// The total voting power of all validators who have voted for this block.
    pub fn aggregated_voting_power<VV>(
        &self,
        round: Round,
        block_id: <V as Vote>::Hash,
        verifier: &VV,
    ) -> u128
    where
        VV: ValidatorVerifier<B, V>,
    {
        let key = (round, block_id);

        self.votes
            .get(&key)
            .map(|votes| {
                votes
                    .values()
                    .filter_map(|vote| verifier.get_voting_power(&vote.author()))
                    .map(|vp| vp as u128)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Check if a quorum can be formed for a specific block.
    ///
    /// This is a convenience method that checks if the accumulated voting power
    /// for a block meets the quorum threshold.
    ///
    /// # Parameters
    ///
    /// * `round` - The round number
    /// * `block_id` - The hash of the block
    /// * `verifier` - The validator verifier to check quorum against
    ///
    /// # Returns
    ///
    /// `true` if quorum can be formed, `false` otherwise.
    pub fn can_form_quorum<VV>(
        &self,
        round: Round,
        block_id: <V as Vote>::Hash,
        verifier: &VV,
    ) -> bool
    where
        VV: ValidatorVerifier<B, V>,
    {
        let voting_power = self.aggregated_voting_power(round, block_id, verifier);
        voting_power >= verifier.quorum_voting_power()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_traits::core::Error;

    // Mock types for testing
    #[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
    struct MockBlock;

    impl Block for MockBlock {
        type Transaction = MockTransaction;
        type Metadata = MockMetadata;
        type Signature = MockSignature;
        type Hash = MockHash;

        fn id(&self) -> Self::Hash {
            MockHash([0u8; 32])
        }

        fn parent_id(&self) -> Self::Hash {
            MockHash([0u8; 32])
        }

        fn transactions(&self) -> &[Self::Transaction] {
            &[]
        }

        fn metadata(&self) -> &Self::Metadata {
            static METADATA: MockMetadata = MockMetadata;
            &METADATA
        }

        fn signature(&self) -> Option<&Self::Signature> {
            static SIG: MockSignature = MockSignature(0);
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
            MockBlock
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct MockHash([u8; 32]);

    impl consensus_traits::core::Hash for MockHash {
        fn zero() -> Self {
            MockHash([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length").into());
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(bytes);
            Ok(MockHash(hash))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockTransaction;

    impl consensus_traits::block::Transaction for MockTransaction {
        type Hash = MockHash;

        fn hash(&self) -> Self::Hash {
            MockHash([0u8; 32])
        }

        fn size(&self) -> usize {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct MockMetadata;

    impl consensus_traits::block::BlockMetadata for MockMetadata {
        type QuorumCert = MockQuorumCert;
        type NodeId = MockNodeId;
        type Hash = MockHash;

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn author(&self) -> Self::NodeId {
            MockNodeId([0u8; 32])
        }

        fn parent_id(&self) -> Self::Hash {
            MockHash([0u8; 32])
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct MockNodeId([u8; 32]);

    impl consensus_traits::core::Hash for MockNodeId {
        fn zero() -> Self {
            MockNodeId([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length").into());
            }
            let mut id = [0u8; 32];
            id.copy_from_slice(bytes);
            Ok(MockNodeId(id))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    impl consensus_traits::core::NodeId for MockNodeId {
        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length").into());
            }
            let mut id = [0u8; 32];
            id.copy_from_slice(bytes);
            Ok(MockNodeId(id))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct MockQuorumCert;

    impl consensus_traits::block::QuorumCertificate for MockQuorumCert {
        type BlockMetadata = MockMetadata;
        type Hash = MockHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            static BLOCK: MockMetadata = MockMetadata;
            &BLOCK
        }

        fn block_id(&self) -> Self::Hash {
            MockHash([0u8; 32])
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[std::sync::Arc<V>],
            _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
        ) -> Result<Self, Error>
        where
            B: consensus_traits::Block,
            V: consensus_traits::Vote<Block = B>,
        {
            Ok(MockQuorumCert)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct MockSignature(u8);

    #[derive(Clone, Debug)]
    struct MockPublicKey;

    impl consensus_traits::core::PublicKey for MockPublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockPublicKey)
        }
    }

    #[derive(Clone, Debug)]
    struct MockVoteData;

    impl consensus_traits::block::VoteData for MockVoteData {
        type BlockMetadata = MockMetadata;

        fn proposed_block(&self) -> &Self::BlockMetadata {
            static BLOCK: MockMetadata = MockMetadata;
            &BLOCK
        }

        fn parent_block(&self) -> &Self::BlockMetadata {
            static BLOCK: MockMetadata = MockMetadata;
            &BLOCK
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct MockLedgerInfo;

    impl consensus_traits::block::LedgerInfo for MockLedgerInfo {
        type CommitInfo = MockCommitInfo;
        type Hash = MockHash;

        fn commit_info(&self) -> &Self::CommitInfo {
            static INFO: MockCommitInfo = MockCommitInfo;
            &INFO
        }

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn accumulated_state(&self) -> &Self::Hash {
            static HASH: MockHash = MockHash([0u8; 32]);
            &HASH
        }
    }

    #[derive(Clone, Debug)]
    struct MockCommitInfo;

    impl consensus_traits::block::CommitInfo for MockCommitInfo {
        type Hash = MockHash;

        fn block_id(&self) -> Self::Hash {
            MockHash([0u8; 32])
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

    #[derive(Clone, Debug)]
    struct MockVote {
        round: u64,
        block_id: MockHash,
        signature: MockSignature,
        author: MockNodeId,  // Each vote can have a unique author
    }

    use consensus_traits::Vote;

    impl Vote for MockVote {
        type Block = MockBlock;
        type Hash = MockHash;
        type VoteData = MockVoteData;
        type NodeId = MockNodeId;
        type LedgerInfo = MockLedgerInfo;
        type Signature = MockSignature;

        fn vote_data(&self) -> &Self::VoteData {
            static DATA: MockVoteData = MockVoteData;
            &DATA
        }

        fn author(&self) -> Self::NodeId {
            self.author
        }

        fn ledger_info(&self) -> &Self::LedgerInfo {
            static INFO: MockLedgerInfo = MockLedgerInfo;
            &INFO
        }

        fn signature(&self) -> &Self::Signature {
            &self.signature
        }

        fn block_id(&self) -> Self::Hash {
            self.block_id
        }

        fn round(&self) -> u64 {
            self.round
        }

        fn verify<B, VV>(&self, _verifier: &VV) -> Result<(), anyhow::Error>
        where
            B: consensus_traits::Block,
            VV: consensus_traits::ValidatorVerifier<B, Self>,
        {
            Ok(())
        }
    }

    struct MockValidatorVerifier;

    impl ValidatorVerifier<MockBlock, MockVote> for MockValidatorVerifier {
        fn verify_vote(
            &self,
            _vote: &MockVote,
        ) -> Result<(), consensus_traits::core::Error> {
            Ok(())
        }

        fn verify_block(
            &self,
            _block: &MockBlock,
        ) -> Result<(), consensus_traits::core::Error> {
            Ok(())
        }

        fn get_voting_power(&self, _validator_id: &<MockVote as Vote>::NodeId) -> Option<u64> {
            Some(100)
        }

        fn total_voting_power(&self) -> u128 {
            400
        }

        fn quorum_voting_power(&self) -> u128 {
            267
        }

        fn check_voting_power<'a>(
            &self,
            signers: impl Iterator<Item = &'a <MockVote as Vote>::NodeId>,
            check_quorum: bool,
        ) -> Result<(), consensus_traits::core::VerifyError> {
            let mut power = 0u128;
            for _signer in signers {
                power += 100;
            }

            if check_quorum && power < self.quorum_voting_power() {
                return Err(consensus_traits::core::VerifyError::TooLittleVotingPower {
                    voting_power: power,
                    expected_voting_power: self.quorum_voting_power(),
                });
            }

            Ok(())
        }

        fn aggregate_signatures<'a>(
            &self,
            _signatures: impl Iterator<Item = &'a <MockVote as Vote>::Signature>,
        ) -> Result<MockAggregatedSignature, consensus_traits::core::Error> {
            Ok(MockAggregatedSignature)
        }
    }

    // Add the Aggregated type for MockSignature
    impl consensus_traits::core::Signature for MockSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = MockPublicKey;
        type Aggregated = MockAggregatedSignature;

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
            Ok(MockSignature(bytes[0]))
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
            Ok(MockAggregatedSignature)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct MockAggregatedSignature;

    fn create_vote(round: u64, byte: u8, sig_byte: u8) -> Arc<MockVote> {
        Arc::new(MockVote {
            round,
            block_id: MockHash([byte; 32]),
            signature: MockSignature(sig_byte),
            author: MockNodeId([sig_byte; 32]), // Unique author per vote
        })
    }

    #[test]
    fn test_pending_votes_new() {
        let pending = PendingVotes::<MockBlock, MockVote>::new(10);
        assert_eq!(pending.current_round, 0);
        assert_eq!(pending.total_votes(), 0);
    }

    #[test]
    fn test_pending_votes_add_vote() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let vote = create_vote(0, 1, 1);
        let result = pending.add_vote(vote, &verifier);

        assert_eq!(result, VoteReceptionResult::Accepted);
        assert_eq!(pending.total_votes(), 1);
    }

    #[test]
    fn test_pending_votes_duplicate() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let vote = create_vote(0, 1, 1);
        pending.add_vote(vote.clone(), &verifier);

        let result = pending.add_vote(vote, &verifier);
        assert_eq!(result, VoteReceptionResult::Rejected(VoteStatus::Rejected));
    }

    #[test]
    fn test_pending_votes_old_round() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        pending.set_round(5);

        let verifier = MockValidatorVerifier;
        let vote = create_vote(3, 1, 1); // Old round

        let result = pending.add_vote(vote, &verifier);
        assert_eq!(result, VoteReceptionResult::OldRound);
    }

    #[test]
    fn test_pending_votes_set_round_cleanup() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(3);
        let verifier = MockValidatorVerifier;

        // Add votes for rounds 0, 1, 2
        for round in 0..=2 {
            for byte in 0..3 {
                let vote = create_vote(round, byte, byte);
                pending.add_vote(vote, &verifier);
            }
        }

        assert_eq!(pending.total_votes(), 9);

        // Set round to 5, which should clean up rounds < 2
        pending.set_round(5);

        // Should have cleaned up old votes
        assert!(pending.vote_count(0, MockHash([0; 32])) == 0);
    }

    #[test]
    fn test_pending_votes_vote_count() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let block_id = MockHash([1; 32]);

        // Add 3 votes for the same block with different signatures
        for i in 0..3 {
            let vote = Arc::new(MockVote {
                round: 0,
                block_id,
                signature: MockSignature(i),
                author: MockNodeId([i; 32]),
            });
            pending.add_vote(vote, &verifier);
        }

        assert_eq!(pending.vote_count(0, block_id), 3);
    }

    #[test]
    fn test_aggregated_voting_power() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let block_id = MockHash([1; 32]);

        // Add 3 votes with 100 voting power each
        for i in 0..3 {
            let vote = create_vote(0, 1, i);
            pending.add_vote(vote, &verifier);
        }

        // Should have 300 total voting power
        assert_eq!(pending.aggregated_voting_power(0, block_id, &verifier), 300);
    }

    #[test]
    fn test_aggregated_voting_power_no_votes() {
        let pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let block_id = MockHash([1; 32]);

        // No votes should result in 0 voting power
        assert_eq!(pending.aggregated_voting_power(0, block_id, &verifier), 0);
    }

    #[test]
    fn test_can_form_quorum() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        let block_id = MockHash([1; 32]);

        // Add 2 votes (200 voting power) - not enough for quorum (267)
        for i in 0..2 {
            let vote = create_vote(0, 1, i);
            pending.add_vote(vote, &verifier);
        }

        assert!(!pending.can_form_quorum(0, block_id, &verifier));

        // Add third vote (300 voting power) - now we have quorum
        let vote = create_vote(0, 1, 2);
        pending.add_vote(vote, &verifier);

        assert!(pending.can_form_quorum(0, block_id, &verifier));
    }

    #[test]
    fn test_quorum_with_voting_power() {
        let mut pending = PendingVotes::<MockBlock, MockVote>::new(10);
        let verifier = MockValidatorVerifier;

        // Add 3 votes to reach quorum threshold (267)
        for i in 0..3 {
            let vote = create_vote(0, 1, i);
            let result = pending.add_vote(vote, &verifier);

            // First 2 votes should be accepted, 3rd should form quorum
            if i < 2 {
                assert_eq!(result, VoteReceptionResult::Accepted);
            } else {
                assert_eq!(result, VoteReceptionResult::QuorumCompleted);
            }
        }
    }
}
