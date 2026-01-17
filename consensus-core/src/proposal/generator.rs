// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use consensus_traits::{Block, BlockMetadata, QuorumCertificate};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Epoch, Round};

/// Configuration for proposal generation.
///
/// This configuration controls how proposals are created,
/// including maximum block sizes and timing constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalConfig {
    /// Maximum number of transactions per block
    pub max_block_txns: usize,

    /// Maximum block size in bytes
    pub max_block_bytes: usize,

    /// Whether to include a timestamp in proposals
    pub include_timestamp: bool,
}

impl Default for ProposalConfig {
    fn default() -> Self {
        Self {
            max_block_txns: 10000,
            max_block_bytes: 10 * 1024 * 1024, // 10 MB
            include_timestamp: true,
        }
    }
}

/// Generates block proposals for the consensus protocol.
///
/// This generic proposal generator handles:
/// - Parent block selection from quorum certificates
/// - Transaction collection through a trait interface
/// - Block creation with proper metadata (epoch, round, author, timestamp)
/// - Block signing with proposer's private key
///
/// # Type Parameters
///
/// * `B` - Block type, must implement the [`Block`] trait
///
/// # Example
///
/// ```ignore
/// use consensus_core::proposal::{ProposalGenerator, ProposalConfig};
/// use consensus_traits::{Block, ValidatorVerifier};
///
/// let config = ProposalConfig::default();
/// let mut generator = ProposalGenerator::<MyBlock>::new(config, my_validator_id);
///
/// // Generate a proposal for the current round
/// let proposal = generator.generate_proposal(
///     &quorum_cert,
///     current_round,
///     current_epoch,
///     &transaction_provider,
///     &private_key,
/// )?;
/// ```
pub struct ProposalGenerator<B>
where
    B: Block,
{
    /// Configuration for proposal generation
    config: ProposalConfig,

    /// The validator ID (author) for proposals
    author_id: <B::Metadata as BlockMetadata>::NodeId,
}

impl<B> ProposalGenerator<B>
where
    B: Block,
    <B::Metadata as BlockMetadata>::NodeId: Clone,
    <B::Metadata as BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// Create a new proposal generator.
    ///
    /// # Parameters
    ///
    /// * `config` - Configuration for proposal generation
    /// * `author_id` - The validator ID that will author proposals
    pub fn new(config: ProposalConfig, author_id: <B::Metadata as BlockMetadata>::NodeId) -> Self {
        Self { config, author_id }
    }

    /// Generate a block proposal for the given round.
    ///
    /// This method provides the information needed to create a proposal,
    /// and uses a provided builder function to construct the actual block.
    ///
    /// # Parameters
    ///
    /// * `parent_qc` - The quorum certificate for the parent block
    /// * `round` - The current round
    /// * `epoch` - The current epoch
    /// * `txns` - Transactions to include in the proposal
    /// * `block_builder` - Function to construct a block from metadata and transactions
    ///
    /// # Returns
    ///
    /// * The proposed block
    /// * `Err(Error)` if proposal generation fails
    pub fn generate_proposal<F>(
        &self,
        parent_qc: &<B::Metadata as BlockMetadata>::QuorumCert,
        round: Round,
        epoch: Epoch,
        txns: Vec<B::Transaction>,
        block_builder: F,
    ) -> Result<B, consensus_traits::core::Error>
    where
        F: FnOnce(&Self, Round, Epoch, <<B::Metadata as BlockMetadata>::QuorumCert as QuorumCertificate>::Hash, u64, Vec<B::Transaction>) -> B,
    {
        // Get parent block ID from the QC
        let parent_id = parent_qc.block_id();

        // Calculate timestamp if configured
        let timestamp = if self.config.include_timestamp {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        // Use the provided block builder to create the block
        Ok(block_builder(self, round, epoch, parent_id, timestamp, txns))
    }

    /// Get the configuration for this proposal generator.
    pub fn config(&self) -> &ProposalConfig {
        &self.config
    }

    /// Get the author ID for this proposal generator.
    pub fn author_id(&self) -> &<B::Metadata as BlockMetadata>::NodeId {
        &self.author_id
    }

    /// Update the configuration.
    pub fn update_config(&mut self, config: ProposalConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Mock types for testing
    #[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
    struct TestNodeId(u8);

    impl consensus_traits::core::Hash for TestNodeId {
        fn zero() -> Self {
            TestNodeId(0)
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
            Ok(TestNodeId(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    impl consensus_traits::core::NodeId for TestNodeId {
        fn from_bytes(_bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
            Ok(TestNodeId(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestHash(u8);

    impl consensus_traits::core::Hash for TestHash {
        fn zero() -> Self {
            TestHash(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
            if bytes.is_empty() {
                return Err(anyhow::anyhow!("Empty bytes").into());
            }
            Ok(TestHash(bytes[0]))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Debug)]
    struct TestBlock {
        metadata: TestMetadata,
        transactions: Vec<TestTransaction>,
    }

    impl Block for TestBlock {
        type Transaction = TestTransaction;
        type Metadata = TestMetadata;
        type Signature = TestSignature;
        type Hash = TestHash;

        fn id(&self) -> Self::Hash {
            // Simple hash derivation based on round and author
            TestHash(self.metadata.round as u8 ^ self.metadata.author.0)
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

        fn verify_signature(&self) -> Result<(), consensus_traits::core::Error> {
            Ok(())
        }

        fn is_genesis(&self) -> bool {
            false
        }

        fn is_nil(&self) -> bool {
            false
        }

        fn new_genesis() -> Self {
            Self {
                metadata: TestMetadata {
                    epoch: 0,
                    round: 0,
                    author: TestNodeId(0),
                    parent_id: TestHash(0),
                    timestamp: 0,
                },
                transactions: vec![],
            }
        }
    }

    impl TestBlock {
        fn new(transactions: Vec<TestTransaction>, metadata: TestMetadata) -> Self {
            Self {
                metadata,
                transactions,
            }
        }
    }

    #[derive(Clone, Debug)]
    struct TestMetadata {
        epoch: u64,
        round: u64,
        author: TestNodeId,
        parent_id: TestHash,
        timestamp: u64,
    }

    impl BlockMetadata for TestMetadata {
        type QuorumCert = TestQuorumCert;
        type NodeId = TestNodeId;
        type Hash = TestHash;

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


    #[derive(Clone, Debug)]
    struct TestQuorumCert {
        metadata: TestMetadata,
    }

    impl TestQuorumCert {
        fn new() -> Self {
            Self {
                metadata: TestMetadata {
                    epoch: 0,
                    round: 0,
                    author: TestNodeId(0),
                    parent_id: TestHash(0),
                    timestamp: 0,
                },
            }
        }
    }

    impl Default for TestQuorumCert {
        fn default() -> Self {
            Self::new()
        }
    }

    impl QuorumCertificate for TestQuorumCert {
        type BlockMetadata = TestMetadata;
        type Hash = TestHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            &self.metadata
        }

        fn block_id(&self) -> Self::Hash {
            TestHash(0)
        }

        fn verify(&self) -> Result<(), consensus_traits::core::Error> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[Arc<V>],
            _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
        ) -> Result<Self, consensus_traits::core::Error>
        where
            B: Block,
            V: consensus_traits::Vote<Block = B>,
        {
            Ok(TestQuorumCert::new())
        }
    }

    #[derive(Clone, Debug)]
    struct TestTransaction;

    impl consensus_traits::Transaction for TestTransaction {
        type Hash = TestHash;

        fn hash(&self) -> Self::Hash {
            TestHash(0)
        }

        fn size(&self) -> usize {
            100
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
    struct TestSignature;

    impl consensus_traits::core::Signature for TestSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = TestPublicKey;
        type Aggregated = TestAggregatedSignature;

        fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
            Ok(())
        }

        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
            Ok(TestSignature)
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, consensus_traits::core::Error> {
            Ok(TestAggregatedSignature)
        }
    }

    #[derive(Clone, Debug)]
    struct TestPublicKey;

    impl consensus_traits::core::PublicKey for TestPublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
            Ok(TestPublicKey)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct TestAggregatedSignature;

    #[test]
    fn test_proposal_generator_new() {
        let config = ProposalConfig::default();
        let author_id = TestNodeId(1);
        let generator = ProposalGenerator::<TestBlock>::new(config.clone(), author_id);

        assert_eq!(generator.config(), &config);
        assert_eq!(generator.author_id(), &author_id);
    }

    #[test]
    fn test_proposal_generator_generate_proposal() {
        let config = ProposalConfig::default();
        let author_id = TestNodeId(1);
        let generator = ProposalGenerator::<TestBlock>::new(config, author_id);

        let parent_qc = TestQuorumCert::new();
        let round = 5;
        let epoch = 2;
        let txns = vec![TestTransaction, TestTransaction];

        let proposal = generator
            .generate_proposal(&parent_qc, round, epoch, txns, |gen, r, e, parent_id, timestamp, txns| {
                TestBlock::new(
                    txns,
                    TestMetadata {
                        epoch: e,
                        round: r,
                        author: gen.author_id.clone(),
                        parent_id,
                        timestamp,
                    },
                )
            })
            .unwrap();

        assert_eq!(proposal.metadata().epoch(), epoch);
        assert_eq!(proposal.metadata().round(), round);
        assert_eq!(proposal.metadata().author(), TestNodeId(1));
        assert_eq!(proposal.transactions().len(), 2);
    }

    #[test]
    fn test_proposal_config_default() {
        let config = ProposalConfig::default();

        assert_eq!(config.max_block_txns, 10000);
        assert_eq!(config.max_block_bytes, 10 * 1024 * 1024);
        assert!(config.include_timestamp);
    }

    #[test]
    fn test_proposal_generator_update_config() {
        let mut config = ProposalConfig::default();
        let mut generator = ProposalGenerator::<TestBlock>::new(config.clone(), TestNodeId(1));

        config.max_block_txns = 5000;
        config.include_timestamp = false;

        generator.update_config(config.clone());

        assert_eq!(generator.config().max_block_txns, 5000);
        assert!(!generator.config().include_timestamp);
    }
}
