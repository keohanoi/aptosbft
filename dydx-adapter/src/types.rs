// Type definitions for dYdX-specific AptosBFT integration
//
// This module implements the consensus traits for dYdX v4, mapping AptosBFT
// consensus types to dYdX application state.
//
// The implementation follows the pattern from consensus-core testing mock types.

use std::fmt;
use consensus_traits::{
    Block as BlockTrait,
    BlockMetadata,
    Transaction as TransactionTrait,
    Vote as VoteTrait,
    VoteData,
    LedgerInfo,
    CommitInfo,
    Hash as HashTrait,
    NodeId as NodeIdTrait,
    Signature as SignatureTrait,
    PublicKey,
    QuorumCertificate,
    ValidatorVerifier,
};
use consensus_traits::core::{Error as CoreError, VerifyError};
use serde::{Deserialize, Serialize};

/// dYdX hash type (32-byte blake3 hash)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DydxHash(pub [u8; 32]);

impl DydxHash {
    /// Create a new hash from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a hash from raw data
    pub fn from_data(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        Self(arr)
    }

    /// Zero hash (for genesis/initial state)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl HashTrait for DydxHash {
    fn zero() -> Self {
        Self::zero()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != 32 {
            return Err(CoreError::msg(format!("Invalid hash length: {}", bytes.len())));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for DydxHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// dYdX node identifier (validator address)
///
/// Uses a fixed-size array for Copy trait requirement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DydxNodeId(pub [u8; 32]);

impl DydxNodeId {
    /// Create a new node ID from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a node ID from a slice (pads with zeros if needed)
    pub fn from_slice(slice: &[u8]) -> Self {
        let mut arr = [0u8; 32];
        let len = slice.len().min(32);
        arr[..len].copy_from_slice(&slice[..len]);
        Self(arr)
    }
}

impl HashTrait for DydxNodeId {
    fn zero() -> Self {
        Self([0u8; 32])
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != 32 {
            return Err(CoreError::msg(format!("Invalid node ID length: {}", bytes.len())));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl NodeIdTrait for DydxNodeId {
    fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != 32 {
            return Err(CoreError::msg(format!("Invalid node ID length: {}", bytes.len())));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for DydxNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// dYdX aggregated signature type (placeholder)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxAggregatedSignature {
    /// The aggregated signature bytes
    pub bytes: Vec<u8>,
    /// Bitmask of which validators signed
    pub mask: Vec<u8>,
}

/// dYdX signature type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DydxSignature {
    /// The signature bytes
    pub bytes: Vec<u8>,
}

impl DydxSignature {
    /// Create a new signature
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl SignatureTrait for DydxSignature {
    type VerifyError = VerifyError;
    type PublicKey = DydxPublicKey;
    type Aggregated = DydxAggregatedSignature;

    fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
        // TODO: Implement actual signature verification
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        Ok(Self::new(bytes.to_vec()))
    }

    fn aggregate<'a>(signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, CoreError> {
        // TODO: Implement BLS-style signature aggregation
        let sigs: Vec<_> = signatures.collect();
        Ok(DydxAggregatedSignature {
            bytes: sigs.iter().flat_map(|s| s.bytes.clone()).collect(),
            mask: vec![],
        })
    }
}

/// dYdX public key type
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxPublicKey {
    /// The public key bytes
    pub bytes: Vec<u8>,
}

impl PublicKey for DydxPublicKey {
    fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        Ok(Self { bytes: bytes.to_vec() })
    }
}

/// dYdX block metadata (protocol-level information)
///
/// Contains the consensus-relevant information about a block separate from
/// the transaction data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxBlockMetadata {
    /// Epoch number
    pub epoch: u64,
    /// Round within the epoch
    pub round: u64,
    /// Block proposer (validator)
    pub author: DydxNodeId,
    /// Hash of the parent block
    pub parent_id: DydxHash,
    /// Block timestamp (unix milliseconds)
    pub timestamp: u64,
}

impl BlockMetadata for DydxBlockMetadata {
    type QuorumCert = DydxQuorumCert;
    type NodeId = DydxNodeId;
    type Hash = DydxHash;

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

/// A dYdX block containing transactions for execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxBlock {
    /// Block metadata
    pub metadata: DydxBlockMetadata,
    /// Transactions to execute
    pub transactions: Vec<DydxTransaction>,
    /// Optional block signature
    pub signature: Option<DydxSignature>,
}

impl DydxBlock {
    /// Create a new dYdX block
    pub fn new(
        epoch: u64,
        round: u64,
        author: DydxNodeId,
        parent_id: DydxHash,
        timestamp: u64,
        transactions: Vec<DydxTransaction>,
    ) -> Self {
        let metadata = DydxBlockMetadata {
            epoch,
            round,
            author,
            parent_id,
            timestamp,
        };
        Self {
            metadata,
            transactions,
            signature: None,
        }
    }

    /// Create a genesis block
    pub fn genesis(author: DydxNodeId) -> Self {
        Self::new(
            0,
            0,
            author,
            DydxHash::zero(),
            0,
            vec![],
        )
    }
}

impl BlockTrait for DydxBlock {
    type Transaction = DydxTransaction;
    type Metadata = DydxBlockMetadata;
    type Signature = DydxSignature;
    type Hash = DydxHash;

    fn id(&self) -> Self::Hash {
        // Derive ID from metadata
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.metadata.round.to_le_bytes());
        hasher.update(&self.metadata.epoch.to_le_bytes());
        hasher.update(&self.metadata.author.0);
        hasher.update(&self.metadata.parent_id.0);
        let hash = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        DydxHash(arr)
    }

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn transactions(&self) -> &[Self::Transaction] {
        &self.transactions
    }

    fn signature(&self) -> Option<&Self::Signature> {
        self.signature.as_ref()
    }

    fn parent_id(&self) -> Self::Hash {
        self.metadata.parent_id
    }

    fn is_genesis(&self) -> bool {
        self.metadata.epoch() == 0 && self.metadata.round() == 0
    }

    fn is_nil(&self) -> bool {
        self.transactions.is_empty()
    }

    fn verify_signature(&self) -> Result<(), CoreError> {
        match &self.signature {
            Some(_) => Ok(()), // TODO: Implement actual verification
            None => Err(CoreError::msg("No signature")),
        }
    }

    fn new_genesis() -> Self {
        Self::genesis(DydxNodeId([0u8; 32]))
    }
}

/// A dYdX transaction containing application-specific data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxTransaction {
    /// Raw transaction bytes
    pub data: Vec<u8>,
}

impl DydxTransaction {
    /// Create a new transaction from raw bytes
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Get the transaction hash
    pub fn compute_hash(&self) -> DydxHash {
        DydxHash::from_data(&self.data)
    }
}

impl TransactionTrait for DydxTransaction {
    type Hash = DydxHash;

    fn hash(&self) -> Self::Hash {
        self.compute_hash()
    }

    fn size(&self) -> usize {
        self.data.len()
    }
}

/// dYdX commit information
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxCommitInfo {
    /// Block ID
    pub block_id: DydxHash,
    /// Round number
    pub round: u64,
    /// Epoch number
    pub epoch: u64,
    /// Version (transaction count)
    pub version: u64,
    /// State root (app hash)
    pub state_root: DydxHash,
    /// Timestamp
    pub timestamp: u64,
}

impl CommitInfo for DydxCommitInfo {
    type Hash = DydxHash;

    fn block_id(&self) -> Self::Hash {
        self.block_id
    }

    fn round(&self) -> u64 {
        self.round
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

/// dYdX ledger information after executing a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxLedgerInfo {
    /// Commit information
    pub commit_info: DydxCommitInfo,
    /// Accumulated state root
    pub accumulated_state: DydxHash,
}

impl LedgerInfo for DydxLedgerInfo {
    type Hash = DydxHash;
    type CommitInfo = DydxCommitInfo;

    fn commit_info(&self) -> &Self::CommitInfo {
        &self.commit_info
    }

    fn epoch(&self) -> u64 {
        self.commit_info.epoch()
    }

    fn round(&self) -> u64 {
        self.commit_info.round()
    }

    fn accumulated_state(&self) -> &Self::Hash {
        &self.accumulated_state
    }
}

/// dYdX quorum certificate (placeholder implementation)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxQuorumCert {
    /// Certified block metadata
    pub certified_block: DydxBlockMetadata,
    /// Aggregated signature
    pub aggregated_signature: DydxAggregatedSignature,
    /// Signers bitmask
    pub signers: Vec<u8>,
}

impl QuorumCertificate for DydxQuorumCert {
    type BlockMetadata = DydxBlockMetadata;
    type Hash = DydxHash;

    fn certified_block(&self) -> &Self::BlockMetadata {
        &self.certified_block
    }

    fn block_id(&self) -> Self::Hash {
        // Derive from certified_block
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.certified_block.round.to_le_bytes());
        hasher.update(&self.certified_block.epoch.to_le_bytes());
        hasher.update(&self.certified_block.author.0);
        let hash = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        DydxHash(arr)
    }

    fn epoch(&self) -> u64 {
        self.certified_block.epoch()
    }

    fn round(&self) -> u64 {
        self.certified_block.round()
    }

    fn verify(&self) -> Result<(), CoreError> {
        // TODO: Implement QC verification
        Ok(())
    }

    fn from_votes<B, V>(
        _votes: &[std::sync::Arc<V>],
        _aggregated_signature: <V::Signature as SignatureTrait>::Aggregated,
    ) -> Result<Self, CoreError>
    where
        B: BlockTrait,
        V: VoteTrait,
    {
        // TODO: Implement QC formation from votes
        Err(CoreError::msg("Not implemented"))
    }
}

/// dYdX vote data
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxVoteData {
    /// Block being voted on
    pub proposed_block: DydxBlockMetadata,
    /// Parent block
    pub parent_block: DydxBlockMetadata,
}

impl VoteData for DydxVoteData {
    type BlockMetadata = DydxBlockMetadata;

    fn proposed_block(&self) -> &Self::BlockMetadata {
        &self.proposed_block
    }

    fn parent_block(&self) -> &Self::BlockMetadata {
        &self.parent_block
    }

    fn verify(&self) -> Result<(), CoreError> {
        // Basic consistency check
        if self.proposed_block.round() != self.parent_block.round() + 1 {
            return Err(CoreError::msg("Round increment invalid"));
        }
        Ok(())
    }
}

/// A dYdX vote in the AptosBFT consensus protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxVote {
    /// Voter's node ID
    pub author: DydxNodeId,
    /// Vote data
    pub vote_data: DydxVoteData,
    /// Ledger info
    pub ledger_info: DydxLedgerInfo,
    /// Vote signature
    pub signature: DydxSignature,
    /// Vote extension (e.g., oracle prices)
    pub extension: Vec<u8>,
}

impl VoteTrait for DydxVote {
    type Block = DydxBlock;
    type Hash = DydxHash;
    type VoteData = DydxVoteData;
    type NodeId = DydxNodeId;
    type LedgerInfo = DydxLedgerInfo;
    type Signature = DydxSignature;

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
        self.vote_data.proposed_block().parent_id
    }

    fn round(&self) -> u64 {
        self.vote_data.proposed_block().round()
    }

    fn extension(&self) -> &[u8] {
        &self.extension
    }

    fn verify<B, VV>(&self, _verifier: &VV) -> Result<(), CoreError>
    where
        B: BlockTrait,
        VV: ValidatorVerifier<B, Self>,
    {
        // TODO: Implement vote verification
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_from_data() {
        let data = b"test data";
        let hash = DydxHash::from_data(data);
        assert_ne!(hash.0, [0u8; 32]);
    }

    #[test]
    fn test_hash_zero() {
        let hash = DydxHash::zero();
        assert_eq!(hash.0, [0u8; 32]);
    }

    #[test]
    fn test_node_id_from_slice() {
        let bytes = vec![1u8; 16];
        let id = DydxNodeId::from_slice(&bytes);
        assert_eq!(id.0[..16], bytes[..16]);
        assert_eq!(id.0[16..], [0u8; 16]);
    }

    #[test]
    fn test_block_genesis() {
        let author = DydxNodeId([42u8; 32]);
        let genesis = DydxBlock::genesis(author);
        assert!(genesis.is_genesis());
        assert!(genesis.is_nil());
        assert_eq!(genesis.metadata.epoch, 0);
        assert_eq!(genesis.metadata.round, 0);
    }

    #[test]
    fn test_block_id_derivation() {
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([4u8; 32]),
            1000,
            vec![],
        );
        let id = block.id();
        assert_ne!(id.0, [0u8; 32]);
    }

    #[test]
    fn test_transaction_hash() {
        let tx = DydxTransaction::new(vec![1, 2, 3, 4, 5]);
        let hash = tx.hash();
        assert_eq!(hash.0.len(), 32);
        assert_eq!(tx.size(), 5);
    }

    #[test]
    fn test_ledger_info() {
        let commit_info = DydxCommitInfo {
            block_id: DydxHash([1u8; 32]),
            round: 100,
            epoch: 1,
            version: 5,
            state_root: DydxHash([2u8; 32]),
            timestamp: 12345,
        };
        let ledger_info = DydxLedgerInfo {
            commit_info,
            accumulated_state: DydxHash([3u8; 32]),
        };

        assert_eq!(ledger_info.epoch(), 1);
        assert_eq!(ledger_info.round(), 100);
        assert_eq!(ledger_info.commit_info().version(), 5);
    }

    #[test]
    fn test_vote_data_verify() {
        let vote_data = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 2,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 1,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };
        assert!(vote_data.verify().is_ok());
    }

    #[test]
    fn test_vote_data_verify_invalid() {
        let vote_data = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 5, // Wrong round
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 3, // Parent round 3, child should be 4
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };
        assert!(vote_data.verify().is_err());
    }

    #[test]
    fn test_signature_aggregate() {
        let sig1 = DydxSignature::new(vec![1, 2, 3]);
        let sig2 = DydxSignature::new(vec![4, 5, 6]);
        let aggregated = DydxSignature::aggregate(vec![&sig1, &sig2].into_iter()).unwrap();
        assert_eq!(aggregated.bytes, vec![1, 2, 3, 4, 5, 6]);
    }
}
