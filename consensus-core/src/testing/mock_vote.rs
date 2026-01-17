// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use consensus_traits::{Vote, VoteData, LedgerInfo};
use consensus_traits::core::Error;

use super::{MockHash, MockSignature, MockNodeId};
use super::mock_block::{MockBlockMetadata};

/// Mock vote for testing.
#[derive(Clone, Debug)]
pub struct MockVote {
    /// The author of the vote
    author: MockNodeId,

    /// The block ID being voted on
    block_id: MockHash,

    /// The round of the vote
    round: u64,

    /// The signature
    signature: MockSignature,

    /// Vote data
    vote_data: super::MockVoteData,

    /// Ledger info
    ledger_info: super::MockLedgerInfo,
}

impl MockVote {
    /// Create a new mock vote.
    ///
    /// # Parameters
    ///
    /// * `author` - The validator creating the vote
    /// * `block_id` - The block ID being voted on
    /// * `round` - The round number
    pub fn new(author: MockNodeId, block_id: MockHash, round: u64) -> Self {
        Self {
            author,
            block_id,
            round,
            signature: MockSignature(author.0),
            vote_data: super::MockVoteData,
            ledger_info: super::MockLedgerInfo,
        }
    }

    /// Create a new mock vote with a specific signature byte.
    ///
    /// This is useful for creating votes with different signatures
    /// while keeping other fields the same.
    ///
    /// # Parameters
    ///
    /// * `author` - The validator creating the vote
    /// * `block_id` - The block ID being voted on
    /// * `round` - The round number
    /// * `sig_byte` - The signature byte value
    pub fn with_signature(author: MockNodeId, block_id: MockHash, round: u64, sig_byte: u8) -> Self {
        Self {
            author,
            block_id,
            round,
            signature: MockSignature(sig_byte),
            vote_data: super::MockVoteData,
            ledger_info: super::MockLedgerInfo,
        }
    }
}

impl Vote for MockVote {
    type Block = super::MockBlock;
    type Hash = MockHash;
    type VoteData = super::MockVoteData;
    type NodeId = MockNodeId;
    type LedgerInfo = super::MockLedgerInfo;
    type Signature = MockSignature;

    fn vote_data(&self) -> &Self::VoteData {
        &self.vote_data
    }

    fn author(&self) -> Self::NodeId {
        self.author
    }

    fn ledger_info(&self) -> &Self::LedgerInfo {
        &self.ledger_info
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

/// Mock vote data for testing.
#[derive(Clone, Debug)]
pub struct MockVoteData;

impl VoteData for MockVoteData {
    type BlockMetadata = MockBlockMetadata;

    fn proposed_block(&self) -> &Self::BlockMetadata {
        // Use LazyLock for static metadata
        use std::sync::OnceLock;
        static META: OnceLock<MockBlockMetadata> = OnceLock::new();
        META.get_or_init(|| MockBlockMetadata::genesis())
    }

    fn parent_block(&self) -> &Self::BlockMetadata {
        // Use LazyLock for static metadata
        use std::sync::OnceLock;
        static META: OnceLock<MockBlockMetadata> = OnceLock::new();
        META.get_or_init(|| MockBlockMetadata::genesis())
    }

    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }
}

/// Mock ledger info for testing.
#[derive(Clone, Debug)]
pub struct MockLedgerInfo;

impl LedgerInfo for MockLedgerInfo {
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
        static HASH: MockHash = MockHash(0);
        &HASH
    }
}

/// Mock commit info for testing.
#[derive(Clone, Debug)]
pub struct MockCommitInfo;

impl consensus_traits::block::CommitInfo for MockCommitInfo {
    type Hash = MockHash;

    fn block_id(&self) -> Self::Hash {
        MockHash(0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_traits::block::CommitInfo;

    #[test]
    fn test_mock_vote_new() {
        let vote = MockVote::new(MockHash(1), MockHash(2), 5);

        assert_eq!(vote.author(), MockHash(1));
        assert_eq!(vote.block_id(), MockHash(2));
        assert_eq!(vote.round(), 5);
    }

    #[test]
    fn test_mock_vote_with_signature() {
        let vote = MockVote::with_signature(MockHash(1), MockHash(2), 5, 99);

        assert_eq!(vote.signature(), &MockSignature(99));
    }

    #[test]
    fn test_mock_vote_default_signature() {
        let vote = MockVote::new(MockHash(5), MockHash(0), 0);

        // Signature should match the author's ID byte
        assert_eq!(vote.signature(), &MockSignature(5));
    }

    #[test]
    fn test_mock_vote_vote_data() {
        let vote = MockVote::new(MockHash(1), MockHash(2), 5);
        let _data = vote.vote_data();
        // Just check the method works
    }

    #[test]
    fn test_mock_vote_ledger_info() {
        let vote = MockVote::new(MockHash(1), MockHash(2), 5);
        let _info = vote.ledger_info();
        // Just check the method works
    }

    #[test]
    fn test_mock_vote_verify() {
        use crate::testing::MockValidatorSet;
        use crate::testing::mock_validator::MockValidator;

        let validators = vec![MockValidator::new(1, 100)];
        let verifier = MockValidatorSet::new(validators);
        let vote = MockVote::new(MockHash(1), MockHash(2), 5);
        assert!(vote.verify(&verifier).is_ok());
    }

    #[test]
    fn test_mock_vote_data_proposed_block() {
        let vote_data = MockVoteData;
        let _block = vote_data.proposed_block();
        // Just check the method works
    }

    #[test]
    fn test_mock_vote_data_parent_block() {
        let vote_data = MockVoteData;
        let _block = vote_data.parent_block();
        // Just check the method works
    }

    #[test]
    fn test_mock_vote_data_verify() {
        let vote_data = MockVoteData;
        assert!(vote_data.verify().is_ok());
    }

    #[test]
    fn test_mock_ledger_info_commit_info() {
        let ledger_info = MockLedgerInfo;
        let _info = ledger_info.commit_info();
        // Just check the method works
    }

    #[test]
    fn test_mock_ledger_info_epoch() {
        let ledger_info = MockLedgerInfo;
        assert_eq!(ledger_info.epoch(), 0);
    }

    #[test]
    fn test_mock_ledger_info_round() {
        let ledger_info = MockLedgerInfo;
        assert_eq!(ledger_info.round(), 0);
    }

    #[test]
    fn test_mock_ledger_info_accumulated_state() {
        let ledger_info = MockLedgerInfo;
        let _hash = ledger_info.accumulated_state();
        // Just check the method works
    }

    #[test]
    fn test_mock_commit_info_block_id() {
        let commit_info = MockCommitInfo;
        assert_eq!(commit_info.block_id(), MockHash(0));
    }

    #[test]
    fn test_mock_commit_info_epoch() {
        let commit_info = MockCommitInfo;
        assert_eq!(commit_info.epoch(), 0);
    }

    #[test]
    fn test_mock_commit_info_round() {
        let commit_info = MockCommitInfo;
        assert_eq!(commit_info.round(), 0);
    }

    #[test]
    fn test_mock_commit_info_version() {
        let commit_info = MockCommitInfo;
        assert_eq!(commit_info.version(), 0);
    }

    #[test]
    fn test_mock_commit_info_timestamp() {
        let commit_info = MockCommitInfo;
        assert_eq!(commit_info.timestamp(), 0);
    }
}
