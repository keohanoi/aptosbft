// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use consensus_traits::{Block, Transaction};
use consensus_traits::block::{BlockMetadata, QuorumCertificate};
use consensus_traits::core::Error;

use super::{MockHash, MockSignature, MockNodeId};

/// Mock block metadata for testing.
#[derive(Clone, Debug, PartialEq)]
pub struct MockBlockMetadata {
    /// The epoch number
    epoch: u64,

    /// The round number
    round: u64,

    /// The block author
    author: MockNodeId,

    /// The parent block ID
    parent_id: MockHash,

    /// The timestamp
    timestamp: u64,
}

impl MockBlockMetadata {
    /// Create a new block metadata.
    pub fn new(epoch: u64, round: u64, author: MockNodeId, parent_id: MockHash, timestamp: u64) -> Self {
        Self {
            epoch,
            round,
            author,
            parent_id,
            timestamp,
        }
    }

    /// Create genesis block metadata.
    pub fn genesis() -> Self {
        Self {
            epoch: 0,
            round: 0,
            author: MockHash(0),
            parent_id: MockHash(0),
            timestamp: 0,
        }
    }
}

impl BlockMetadata for MockBlockMetadata {
    type QuorumCert = MockQuorumCert;
    type NodeId = MockNodeId;
    type Hash = MockHash;

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn round(&self) -> u64 {
        self.round
    }

    fn author(&self) -> Self::NodeId {
        self.author
    }

    fn parent_id(&self) -> Self::Hash {
        self.parent_id
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

/// Mock quorum certificate for testing.
#[derive(Clone, Debug)]
pub struct MockQuorumCert {
    /// The certified block metadata
    metadata: MockBlockMetadata,

    /// The block ID
    block_id: MockHash,
}

impl MockQuorumCert {
    /// Create a new mock quorum certificate.
    pub fn new(block_id: MockHash) -> Self {
        Self {
            metadata: MockBlockMetadata::genesis(),
            block_id,
        }
    }

    /// Create a mock quorum certificate with custom metadata.
    pub fn with_metadata(metadata: MockBlockMetadata, block_id: MockHash) -> Self {
        Self { metadata, block_id }
    }
}

impl Default for MockQuorumCert {
    fn default() -> Self {
        Self::new(MockHash(0))
    }
}

impl QuorumCertificate for MockQuorumCert {
    type BlockMetadata = MockBlockMetadata;
    type Hash = MockHash;

    fn certified_block(&self) -> &Self::BlockMetadata {
        &self.metadata
    }

    fn block_id(&self) -> Self::Hash {
        self.block_id
    }

    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }

    fn from_votes<B, V>(
        _votes: &[Arc<V>],
        _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
    ) -> Result<Self, Error>
    where
        B: Block,
        V: consensus_traits::Vote<Block = B>,
    {
        Ok(Self::default())
    }
}

/// Mock block for testing.
#[derive(Clone, Debug)]
pub struct MockBlock {
    /// The block metadata
    metadata: MockBlockMetadata,

    /// The transactions in the block
    transactions: Vec<MockTransaction>,
}

impl MockBlock {
    /// Create a new mock block.
    ///
    /// # Parameters
    ///
    /// * `transactions` - Transactions to include in the block
    /// * `metadata` - Block metadata
    pub fn new(transactions: Vec<MockTransaction>, metadata: MockBlockMetadata) -> Self {
        Self {
            metadata,
            transactions,
        }
    }

    /// Create a genesis block.
    pub fn genesis() -> Self {
        Self {
            metadata: MockBlockMetadata::genesis(),
            transactions: vec![],
        }
    }
}

impl Block for MockBlock {
    type Transaction = MockTransaction;
    type Metadata = MockBlockMetadata;
    type Signature = MockSignature;
    type Hash = MockHash;

    fn id(&self) -> Self::Hash {
        // Simple hash derivation based on round and author
        MockHash(self.metadata.round as u8 ^ self.metadata.author.0)
    }

    fn parent_id(&self) -> Self::Hash {
        self.metadata.parent_id
    }

    fn transactions(&self) -> &[Self::Transaction] {
        &self.transactions
    }

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn signature(&self) -> Option<&Self::Signature> {
        None
    }

    fn verify_signature(&self) -> Result<(), Error> {
        Ok(())
    }

    fn is_genesis(&self) -> bool {
        self.metadata.epoch() == 0 && self.metadata.round() == 0
    }

    fn is_nil(&self) -> bool {
        false
    }

    fn new_genesis() -> Self {
        Self::genesis()
    }
}

/// Mock transaction for testing.
#[derive(Clone, Debug)]
pub struct MockTransaction;

impl Transaction for MockTransaction {
    type Hash = MockHash;

    fn hash(&self) -> Self::Hash {
        MockHash(0)
    }

    fn size(&self) -> usize {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_block_metadata_new() {
        let metadata = MockBlockMetadata::new(1, 2, MockHash(3), MockHash(4), 5);

        assert_eq!(metadata.epoch(), 1);
        assert_eq!(metadata.round(), 2);
        assert_eq!(metadata.author(), MockHash(3));
        assert_eq!(metadata.parent_id(), MockHash(4));
        assert_eq!(metadata.timestamp(), 5);
    }

    #[test]
    fn test_mock_block_metadata_genesis() {
        let metadata = MockBlockMetadata::genesis();

        assert_eq!(metadata.epoch(), 0);
        assert_eq!(metadata.round(), 0);
        assert_eq!(metadata.author(), MockHash(0));
        assert_eq!(metadata.parent_id(), MockHash(0));
        assert_eq!(metadata.timestamp(), 0);
    }

    #[test]
    fn test_mock_block_new() {
        let metadata = MockBlockMetadata::new(1, 2, MockHash(3), MockHash(4), 5);
        let block = MockBlock::new(vec![MockTransaction, MockTransaction], metadata.clone());

        assert_eq!(block.metadata(), &metadata);
        assert_eq!(block.transactions().len(), 2);
        assert_eq!(block.metadata().epoch(), 1);
        assert_eq!(block.metadata().round(), 2);
    }

    #[test]
    fn test_mock_block_genesis() {
        let block = MockBlock::genesis();

        assert!(block.is_genesis());
        assert_eq!(block.transactions().len(), 0);
    }

    #[test]
    fn test_mock_block_id_derivation() {
        let metadata = MockBlockMetadata::new(1, 5, MockHash(3), MockHash(4), 0);
        let block = MockBlock::new(vec![], metadata);

        // Hash should be round ^ author = 5 ^ 3 = 6
        assert_eq!(block.id(), MockHash(6));
    }

    #[test]
    fn test_mock_quorum_cert_new() {
        let qc = MockQuorumCert::new(MockHash(42));

        assert_eq!(qc.block_id(), MockHash(42));
    }

    #[test]
    fn test_mock_quorum_cert_default() {
        let qc = MockQuorumCert::default();

        assert_eq!(qc.block_id(), MockHash(0));
    }

    #[test]
    fn test_mock_transaction() {
        let txn = MockTransaction;

        assert_eq!(txn.size(), 100);
        assert_eq!(txn.hash(), MockHash(0));
    }

    #[test]
    fn test_mock_quorum_cert_with_metadata() {
        let metadata = MockBlockMetadata::new(1, 2, MockHash(3), MockHash(4), 5);
        let qc = MockQuorumCert::with_metadata(metadata.clone(), MockHash(42));

        assert_eq!(qc.block_id(), MockHash(42));
        assert_eq!(qc.certified_block(), &metadata);
    }

    #[test]
    fn test_mock_quorum_cert_verify() {
        let qc = MockQuorumCert::new(MockHash(42));
        assert!(qc.verify().is_ok());
    }

    #[test]
    fn test_mock_quorum_cert_from_votes() {
        use crate::testing::{MockVote, MockAggregatedSignature};
        use consensus_traits::Vote;

        let votes = vec![Arc::new(MockVote::new(MockHash(1), MockHash(0), 0))];
        let sig = MockAggregatedSignature;
        let qc = MockQuorumCert::from_votes::<MockBlock, MockVote>(&votes, sig);
        assert!(qc.is_ok());
    }

    #[test]
    fn test_mock_block_is_nil() {
        let block = MockBlock::genesis();
        assert!(!block.is_nil());
    }

    #[test]
    fn test_mock_block_new_genesis() {
        let block = MockBlock::new_genesis();
        assert!(block.is_genesis());
    }

    #[test]
    fn test_mock_block_parent_id() {
        let metadata = MockBlockMetadata::new(1, 2, MockHash(3), MockHash(99), 5);
        let block = MockBlock::new(vec![], metadata);
        assert_eq!(block.parent_id(), MockHash(99));
    }

    #[test]
    fn test_mock_block_verify_signature() {
        let block = MockBlock::genesis();
        assert!(block.verify_signature().is_ok());
    }

    #[test]
    fn test_mock_block_signature_none() {
        let block = MockBlock::genesis();
        assert!(block.signature().is_none());
    }
}
