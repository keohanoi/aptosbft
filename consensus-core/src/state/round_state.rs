// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use consensus_traits::Block;
use std::sync::Arc;

use crate::types::{Epoch, Round};

/// Configuration for RoundState
#[derive(Clone, Debug)]
pub struct RoundStateConfig {
    /// The initial epoch
    pub initial_epoch: Epoch,

    /// The initial round
    pub initial_round: Round,

    /// Maximum number of rounds to buffer votes for
    pub max_buffered_rounds: u64,
}

impl Default for RoundStateConfig {
    fn default() -> Self {
        Self {
            initial_epoch: crate::types::GENESIS_EPOCH,
            initial_round: crate::types::GENESIS_ROUND,
            max_buffered_rounds: 10,
        }
    }
}

/// Represents the state of the consensus protocol for a specific round
///
/// This struct tracks the current round, epoch, and related state needed
/// for the consensus protocol to function correctly.
#[derive(Clone, Debug)]
pub struct RoundState<B>
where
    B: Block,
    <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// The current epoch
    epoch: Epoch,

    /// The current round within the epoch
    round: Round,

    /// The proposed block for the current round (if any)
    proposed_block: Option<Arc<B>>,

    /// The current quorum certificate (if any)
    quorum_cert: Option<<<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert>,

    /// Configuration for the round state
    config: RoundStateConfig,
}

impl<B> RoundState<B>
where
    B: Block,
    <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// Create a new RoundState with the given configuration
    pub fn new(config: RoundStateConfig) -> Self {
        Self {
            epoch: config.initial_epoch,
            round: config.initial_round,
            proposed_block: None,
            quorum_cert: None,
            config,
        }
    }

    /// Get the current epoch
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Get the current round
    pub fn round(&self) -> Round {
        self.round
    }

    /// Set the current round
    pub fn set_round(&mut self, round: Round) {
        self.round = round;
        // Clear the proposed block when changing rounds
        self.proposed_block = None;
    }

    /// Set the current epoch
    pub fn set_epoch(&mut self, epoch: Epoch) {
        self.epoch = epoch;
        self.round = self.config.initial_round;
        self.proposed_block = None;
        self.quorum_cert = None;
    }

    /// Get the proposed block for the current round
    pub fn proposed_block(&self) -> Option<&Arc<B>> {
        self.proposed_block.as_ref()
    }

    /// Set the proposed block for the current round
    pub fn set_proposed_block(&mut self, block: Arc<B>) {
        self.proposed_block = Some(block);
    }

    /// Get the current quorum certificate
    pub fn quorum_cert(
        &self,
    ) -> Option<&<<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert> {
        self.quorum_cert.as_ref()
    }

    /// Set the quorum certificate
    pub fn set_quorum_cert(
        &mut self,
        qc: <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert,
    ) {
        self.quorum_cert = Some(qc);
    }

    /// Advance to the next round
    pub fn advance_round(&mut self) {
        self.round += 1;
        self.proposed_block = None;
    }

    /// Check if a vote is for the current round
    pub fn is_current_round(&self, round: Round) -> bool {
        self.round == round
    }

    /// Check if a vote is for the current epoch
    pub fn is_current_epoch(&self, epoch: Epoch) -> bool {
        self.epoch == epoch
    }

    /// Check if a round is within the buffered range
    pub fn is_buffered_round(&self, round: Round) -> bool {
        round > self.round && round <= self.round + self.config.max_buffered_rounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_traits::core::Error;
    use std::sync::Arc;

    // Mock Block and Vote implementations for testing
    #[derive(Clone, Debug, PartialEq, Eq)]
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
            static SIG: MockSignature = MockSignature;
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
    struct MockSignature;

    #[derive(Clone, Copy, Debug)]
    struct MockAggregatedSignature;

    impl consensus_traits::core::Signature for MockSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = MockPublicKey;
        type Aggregated = MockAggregatedSignature;

        fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
            Ok(())
        }

        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockSignature)
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
            Ok(MockAggregatedSignature)
        }
    }

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

    use consensus_traits::Vote;

    #[derive(Clone, Debug)]
    struct MockVote;

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
            MockNodeId([0u8; 32])
        }

        fn ledger_info(&self) -> &Self::LedgerInfo {
            static INFO: MockLedgerInfo = MockLedgerInfo;
            &INFO
        }

        fn signature(&self) -> &Self::Signature {
            static SIG: MockSignature = MockSignature;
            &SIG
        }

        fn block_id(&self) -> Self::Hash {
            MockHash([0u8; 32])
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

    #[test]
    fn test_round_state_initialization() {
        let config = RoundStateConfig::default();
        let state = RoundState::<MockBlock>::new(config);

        assert_eq!(state.epoch(), 0);
        assert_eq!(state.round(), 0);
        assert!(state.proposed_block().is_none());
    }

    #[test]
    fn test_round_state_advance() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        state.advance_round();
        assert_eq!(state.round(), 1);
        assert!(state.proposed_block().is_none());
    }

    #[test]
    fn test_round_state_set_round() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        let block = Arc::new(MockBlock);
        state.set_proposed_block(block.clone());
        assert!(state.proposed_block().is_some());

        state.set_round(5);
        assert_eq!(state.round(), 5);
        assert!(state.proposed_block().is_none());
    }

    #[test]
    fn test_round_state_buffered_round() {
        let config = RoundStateConfig {
            initial_epoch: 0,
            initial_round: 5,
            max_buffered_rounds: 3,
        };
        let state = RoundState::<MockBlock>::new(config);

        assert!(state.is_buffered_round(6)); // 5 + 1
        assert!(state.is_buffered_round(8)); // 5 + 3
        assert!(!state.is_buffered_round(9)); // 5 + 4 (beyond buffer)
        assert!(!state.is_buffered_round(5)); // current round
    }

    #[test]
    fn test_round_state_is_current_round() {
        let config = RoundStateConfig::default();
        let state = RoundState::<MockBlock>::new(config);

        assert!(state.is_current_round(0));
        assert!(!state.is_current_round(1));
        assert!(!state.is_current_round(100));
    }

    #[test]
    fn test_round_state_is_current_epoch() {
        let config = RoundStateConfig::default();
        let state = RoundState::<MockBlock>::new(config);

        assert!(state.is_current_epoch(0));
        assert!(!state.is_current_epoch(1));
        assert!(!state.is_current_epoch(100));
    }

    #[test]
    fn test_round_state_set_epoch() {
        let config = RoundStateConfig {
            initial_epoch: 0,
            initial_round: 5,
            max_buffered_rounds: 3,
        };
        let mut state = RoundState::<MockBlock>::new(config);

        // Set a proposed block and QC
        let block = Arc::new(MockBlock);
        state.set_proposed_block(block);
        state.set_quorum_cert(MockQuorumCert);

        // Set new epoch
        state.set_epoch(10);
        assert_eq!(state.epoch(), 10);
        assert_eq!(state.round(), 5); // Reset to initial_round
        assert!(state.proposed_block().is_none());
        assert!(state.quorum_cert().is_none());
    }

    #[test]
    fn test_round_state_quorum_cert() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        // Initially no QC
        assert!(state.quorum_cert().is_none());

        // Set a QC
        state.set_quorum_cert(MockQuorumCert);
        assert!(state.quorum_cert().is_some());
    }

    #[test]
    fn test_round_state_set_quorum_cert() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        // Set a QC
        state.set_quorum_cert(MockQuorumCert);
        assert!(state.quorum_cert().is_some());

        // Set a different QC
        state.set_quorum_cert(MockQuorumCert);
        assert!(state.quorum_cert().is_some());
    }

    #[test]
    fn test_round_state_config_default() {
        let config = RoundStateConfig::default();
        assert_eq!(config.initial_epoch, 0);
        assert_eq!(config.initial_round, 0);
        assert_eq!(config.max_buffered_rounds, 10);
    }

    #[test]
    fn test_round_state_config_custom() {
        let config = RoundStateConfig {
            initial_epoch: 5,
            initial_round: 10,
            max_buffered_rounds: 20,
        };
        assert_eq!(config.initial_epoch, 5);
        assert_eq!(config.initial_round, 10);
        assert_eq!(config.max_buffered_rounds, 20);
    }

    #[test]
    fn test_round_state_with_custom_initial_values() {
        let config = RoundStateConfig {
            initial_epoch: 5,
            initial_round: 10,
            max_buffered_rounds: 3,
        };
        let state = RoundState::<MockBlock>::new(config);

        assert_eq!(state.epoch(), 5);
        assert_eq!(state.round(), 10);
        assert!(state.is_current_epoch(5));
        assert!(state.is_current_round(10));
    }

    #[test]
    fn test_round_state_advance_multiple_rounds() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        // Advance several rounds
        state.advance_round();
        assert_eq!(state.round(), 1);

        state.advance_round();
        assert_eq!(state.round(), 2);

        state.advance_round();
        assert_eq!(state.round(), 3);
    }

    #[test]
    fn test_round_state_proposed_block() {
        let config = RoundStateConfig::default();
        let mut state = RoundState::<MockBlock>::new(config);

        // Initially no proposed block
        assert!(state.proposed_block().is_none());

        // Set a proposed block
        let block = Arc::new(MockBlock);
        state.set_proposed_block(block.clone());

        let retrieved = state.proposed_block();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), block.id());
    }
}
