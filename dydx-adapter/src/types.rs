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
    proposer::{ProposerElection, ProposerInfo, ReputationTracker},
    core::Hash as CoreHash,
};
use consensus_traits::core::{Error as CoreError, VerifyError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::error::DydxAdapterError;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DydxNodeId(pub [u8; 32]);

impl DydxNodeId {
    /// Create a new node ID from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a node ID from a slice (pads with zeros if needed)
    pub fn from_slice(slice: &[u8]) -> Result<Self, CoreError> {
        if slice.len() > 32 {
            return Err(CoreError::msg(format!(
                "Node ID too long: {} bytes (max 32)",
                slice.len()
            )));
        }
        let mut arr = [0u8; 32];
        arr[..slice.len()].copy_from_slice(slice);
        Ok(Self(arr))
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

    /// Verify this signature against a message and public key using Ed25519
    pub fn verify_ed25519(&self, message: &[u8], public_key: &DydxPublicKey) -> Result<(), VerifyError> {
        use ed25519_dalek::{Verifier, Signature, VerifyingKey};

        // Ed25519 signatures must be exactly 64 bytes
        if self.bytes.len() != 64 {
            return Err(VerifyError::InvalidSignature);
        }

        // Ed25519 public keys must be exactly 32 bytes
        if public_key.bytes.len() != 32 {
            return Err(VerifyError::InvalidSignature);
        }

        // Convert bytes to ed25519-dalek types
        let sig_bytes: [u8; 64] = self.bytes
            .as_slice()
            .try_into()
            .map_err(|_| VerifyError::InvalidSignature)?;
        let signature = Signature::from_bytes(&sig_bytes);

        let pk_bytes: [u8; 32] = public_key.bytes
            .as_slice()
            .try_into()
            .map_err(|_| VerifyError::InvalidSignature)?;
        let verifying_key = VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;

        // Verify the signature
        verifying_key
            .verify(message, &signature)
            .map_err(|_| VerifyError::InvalidSignature)?;

        Ok(())
    }
}

impl SignatureTrait for DydxSignature {
    type VerifyError = VerifyError;
    type PublicKey = DydxPublicKey;
    type Aggregated = DydxAggregatedSignature;

    fn verify(&self, message: &[u8], public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
        self.verify_ed25519(message, public_key)
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
            Some(sig) => {
                // Verify the block signature against the proposer's public key
                // In production, you would:
                // 1. Get the proposer's public key from the validator set
                // 2. Serialize the block data (metadata + transactions)
                // 3. Verify the signature
                let block_data = self.id().as_bytes().to_vec();
                let proposer_pk = DydxPublicKey {
                    bytes: self.metadata.author.0.to_vec(),
                };
                sig.verify(&block_data, &proposer_pk)
                    .map_err(|e| CoreError::msg(format!("Signature verification failed: {:?}", e)))?;
                Ok(())
            }
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

/// dYdX quorum certificate
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxQuorumCert {
    /// Certified block metadata
    pub certified_block: DydxBlockMetadata,
    /// Aggregated signature
    pub aggregated_signature: DydxAggregatedSignature,
    /// Signers bitmask (each bit represents a validator address)
    pub signers: Vec<u8>,
    /// Total voting power of all validators
    pub total_voting_power: u64,
    /// Voting power of the signers
    pub signers_voting_power: u64,
}

impl DydxQuorumCert {
    /// Calculate the voting power threshold (2/3 of total)
    pub const VOTING_POWER_THRESHOLD_NUMERATOR: u64 = 2;
    pub const VOTING_POWER_THRESHOLD_DENOMINATOR: u64 = 3;

    /// Create a new quorum certificate
    pub fn new(
        certified_block: DydxBlockMetadata,
        aggregated_signature: DydxAggregatedSignature,
        signers: Vec<u8>,
        total_voting_power: u64,
        signers_voting_power: u64,
    ) -> Self {
        Self {
            certified_block,
            aggregated_signature,
            signers,
            total_voting_power,
            signers_voting_power,
        }
    }

    /// Check if the QC has sufficient voting power (>2/3)
    pub fn has_quorum(&self) -> bool {
        if self.total_voting_power == 0 {
            return false;
        }
        let threshold = (self.total_voting_power * Self::VOTING_POWER_THRESHOLD_NUMERATOR)
            / Self::VOTING_POWER_THRESHOLD_DENOMINATOR;
        self.signers_voting_power > threshold
    }

    /// Count the number of signers from the bitmask
    pub fn signer_count(&self) -> usize {
        self.signers
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum()
    }

    /// Check if a validator (by index) signed this QC
    pub fn is_signer(&self, validator_index: usize) -> bool {
        let byte_index = validator_index / 8;
        let bit_index = validator_index % 8;
        if byte_index >= self.signers.len() {
            return false;
        }
        (self.signers[byte_index] & (1 << bit_index)) != 0
    }

    /// Form a Quorum Certificate from a collection of votes.
    ///
    /// This is the concrete implementation for DydxVote types, which aggregates
    /// individual votes into a single Quorum Certificate that can be used for
    /// consensus decisions.
    ///
    /// # Arguments
    /// * `votes` - Slice of Arc-wrapped DydxVote objects to aggregate
    /// * `validator_set` - Mapping of validator addresses to their (index, voting_power)
    ///
    /// # Returns
    /// * `Ok(DydxQuorumCert)` - If quorum is reached (>2/3 voting power)
    /// * `Err(CoreError)` - If votes are invalid or quorum not reached
    pub fn form_from_votes(
        votes: &[std::sync::Arc<DydxVote>],
        validator_set: &std::collections::HashMap<Vec<u8>, (usize, u64)>,
    ) -> Result<Self, CoreError> {
        if votes.is_empty() {
            return Err(CoreError::msg("No votes provided"));
        }

        // Calculate total voting power from the entire validator set
        let total_voting_power: u64 = validator_set.values().map(|(_, power)| power).sum();

        if total_voting_power == 0 {
            return Err(CoreError::msg("Validator set has zero voting power"));
        }

        // All votes must be for the same block (same epoch and round)
        let first_vote = &votes[0];
        let target_epoch = first_vote.vote_data.proposed_block.epoch;
        let target_round = first_vote.vote_data.proposed_block.round;

        // Verify all votes are for the same block and collect validator info
        let mut validator_indices = Vec::new();
        let mut signers_voting_power = 0u64;
        let mut signatures = Vec::new();

        for vote in votes {
            // Verify vote is for the target block
            if vote.vote_data.proposed_block.epoch != target_epoch
                || vote.vote_data.proposed_block.round != target_round
            {
                return Err(CoreError::msg(format!(
                    "Vote mismatch: expected epoch={} round={}, got epoch={} round={}",
                    target_epoch,
                    target_round,
                    vote.vote_data.proposed_block.epoch,
                    vote.vote_data.proposed_block.round
                )));
            }

            // Find validator index from the validator set
            let author_bytes = vote.author.0.to_vec();
            let (validator_index, voting_power) = validator_set
                .get(&author_bytes)
                .ok_or_else(|| CoreError::msg(format!("Unknown validator: {:?}", vote.author.0)))?;

            validator_indices.push(*validator_index);
            signers_voting_power += voting_power;

            // Collect signature for aggregation
            signatures.push(vote.signature.bytes.clone());
        }

        // Aggregate signatures by concatenation (Ed25519 doesn't support BLS aggregation)
        let aggregated_bytes = signatures.into_iter().flatten().collect();
        let aggregated_signature = DydxAggregatedSignature {
            bytes: aggregated_bytes,
            mask: vec![], // Will be populated below
        };

        // Build signers bitmask
        let max_index = validator_indices.iter().cloned().max().unwrap_or(0);
        let num_bytes = (max_index / 8) + 1;
        let mut signers = vec![0u8; num_bytes];

        for validator_index in validator_indices {
            let byte_index = validator_index / 8;
            let bit_index = validator_index % 8;
            if byte_index < signers.len() {
                signers[byte_index] |= 1 << bit_index;
            }
        }

        // Create the quorum certificate
        let qc = DydxQuorumCert {
            certified_block: first_vote.vote_data.proposed_block.clone(),
            aggregated_signature,
            signers,
            total_voting_power,
            signers_voting_power,
        };

        // Verify quorum is reached
        if !qc.has_quorum() {
            return Err(CoreError::msg(format!(
                "Insufficient voting power: {} / {} (need > 2/3)",
                qc.signers_voting_power, qc.total_voting_power
            )));
        }

        Ok(qc)
    }
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
        // Verify that the QC has sufficient voting power (>2/3)
        if !self.has_quorum() {
            return Err(CoreError::msg(format!(
                "Insufficient voting power: {} / {} (need > 2/3)",
                self.signers_voting_power, self.total_voting_power
            )));
        }

        // Verify the signers bitmask is consistent with the voting power
        if self.signers.is_empty() && self.signers_voting_power > 0 {
            return Err(CoreError::msg("Signers bitmask is empty but voting power is non-zero"));
        }

        // Verify the aggregated signature has the expected size
        // For Ed25519, we expect N * 64 bytes where N is the number of signers
        let expected_sig_size = self.signer_count() * 64;
        if self.aggregated_signature.bytes.len() < expected_sig_size {
            return Err(CoreError::msg(format!(
                "Aggregated signature too small: expected {} bytes, got {}",
                expected_sig_size,
                self.aggregated_signature.bytes.len()
            )));
        }

        Ok(())
    }

    fn from_votes<B, V>(
        votes: &[std::sync::Arc<V>],
        _aggregated_signature: <V::Signature as SignatureTrait>::Aggregated,
    ) -> Result<Self, CoreError>
    where
        B: BlockTrait,
        V: VoteTrait,
    {
        if votes.is_empty() {
            return Err(CoreError::msg("No votes provided"));
        }

        // NOTE: This is a simplified implementation that assumes votes are DydxVote.
        // In production, you would need proper type constraints or a downcast.
        // For now, this provides the structure for future implementation.

        // For generic implementation, we'd need to extract data via trait methods
        // but since our types are concrete, we document this limitation:
        Err(CoreError::msg(
            "from_votes requires DydxVote types - use DydxQuorumCert::form_from_votes for concrete implementation"
        ))

        // Future implementation would:
        // 1. Extract block metadata from votes[0].vote_data().proposed_block()
        // 2. Extract/convert aggregated signature
        // 3. Build signers bitmask
        // 4. Return DydxQuorumCert { certified_block, aggregated_signature, signers }
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

impl DydxVote {
    /// Serialize the vote for signing (excludes the signature field)
    pub fn serialize_for_signing(&self) -> Vec<u8> {
        // In production, this would serialize:
        // - author (node ID)
        // - vote_data (proposed and parent block metadata)
        // - ledger_info
        // - extension
        // For now, use a simple serialization via the block hash
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.author.0);
        hasher.update(&self.vote_data.proposed_block.epoch.to_le_bytes());
        hasher.update(&self.vote_data.proposed_block.round.to_le_bytes());
        hasher.update(&self.vote_data.proposed_block.parent_id.0);
        hasher.update(&self.extension);
        hasher.finalize().as_bytes().to_vec()
    }
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

    fn verify<B, VV>(&self, verifier: &VV) -> Result<(), CoreError>
    where
        B: BlockTrait,
        VV: ValidatorVerifier<B, Self>,
    {
        // 1. Verify the vote data (round linkage, etc.)
        <DydxVoteData as VoteData>::verify(&self.vote_data)?;

        // 2. Verify the signature
        let vote_bytes = DydxVote::serialize_for_signing(self);
        let pk = DydxPublicKey {
            bytes: self.author.0.to_vec(),
        };
        self.signature.verify(&vote_bytes, &pk)
            .map_err(|e| CoreError::msg(format!("Vote signature verification failed: {:?}", e)))?;

        // 3. Verify the voter is a valid validator for this epoch
        // This would check against the validator verifier
        verifier.verify_vote(self)?;

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

        // Verify hash is not zero
        assert_ne!(hash.0, [0u8; 32]);

        // Verify hash is deterministic
        let hash2 = DydxHash::from_data(data);
        assert_eq!(hash.0, hash2.0);

        // Verify different data produces different hash
        let different_data = b"different data";
        let hash3 = DydxHash::from_data(different_data);
        assert_ne!(hash.0, hash3.0);

        // Verify hash length is correct
        assert_eq!(hash.0.len(), 32);
    }

    #[test]
    fn test_hash_zero() {
        let hash = DydxHash::zero();
        assert_eq!(hash.0, [0u8; 32]);

        // Verify zero hash is consistent
        let hash2 = DydxHash::zero();
        assert_eq!(hash.0, hash2.0);

        // Verify non-zero data produces different hash
        let hash3 = DydxHash::from_data(b"test");
        assert_ne!(hash.0, hash3.0);
    }

    #[test]
    fn test_hash_display() {
        let hash = DydxHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let display = format!("{}", hash);

        // Verify hex encoding
        assert_eq!(display, "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");
    }

    #[test]
    fn test_node_id_from_slice() {
        let bytes = vec![1u8; 16];
        let id = DydxNodeId::from_slice(&bytes).unwrap();

        // Verify first 16 bytes are copied
        assert_eq!(id.0[..16], bytes[..16]);

        // Verify remaining 16 bytes are zero-padded
        assert_eq!(id.0[16..], [0u8; 16]);

        // Verify full 32-byte length
        assert_eq!(id.0.len(), 32);
    }

    #[test]
    fn test_node_id_from_slice_full() {
        let bytes = vec![5u8; 32];
        let id = DydxNodeId::from_slice(&bytes).unwrap();

        // Verify all bytes are copied
        assert_eq!(id.0, bytes[..]);
    }

    #[test]
    fn test_node_id_from_slice_empty() {
        let bytes = vec![];
        let id = DydxNodeId::from_slice(&bytes).unwrap();

        // Verify all bytes are zero when empty input
        assert_eq!(id.0, [0u8; 32]);
    }

    #[test]
    fn test_node_id_display() {
        let id = DydxNodeId([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let display = format!("{}", id);

        // Verify hex encoding
        assert_eq!(display, "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");
    }

    #[test]
    fn test_block_genesis() {
        let author = DydxNodeId([42u8; 32]);
        let genesis = DydxBlock::genesis(author);

        // Verify genesis block properties
        assert!(genesis.is_genesis());
        assert!(genesis.is_nil());
        assert_eq!(genesis.metadata.epoch, 0);
        assert_eq!(genesis.metadata.round, 0);
        assert_eq!(genesis.metadata.author, author);
        assert!(genesis.transactions.is_empty());
        assert!(genesis.signature.is_none());
    }

    #[test]
    fn test_block_new() {
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            1000,
            vec![DydxTransaction::new(vec![1, 2, 3])],
        );

        // Verify block properties
        assert_eq!(block.metadata.epoch, 1);
        assert_eq!(block.metadata.round, 5);
        assert_eq!(block.metadata.timestamp, 1000);
        assert_eq!(block.metadata.parent_id, DydxHash([2u8; 32]));
        assert_eq!(block.transactions.len(), 1);
        assert_eq!(block.transactions[0].data, vec![1, 2, 3]);

        // Verify not genesis
        assert!(!block.is_genesis());
        assert!(!block.is_nil());
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

        // Verify block ID is non-zero
        assert_ne!(id.0, [0u8; 32]);

        // Verify block ID is deterministic
        let id2 = block.id();
        assert_eq!(id.0, id2.0);
    }

    #[test]
    fn test_block_verify_signature_without_signature() {
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            1000,
            vec![],
        );

        // Should fail when no signature
        assert!(block.verify_signature().is_err());
    }

    #[test]
    fn test_block_with_signature() {
        let mut block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            1000,
            vec![],
        );

        // Add a signature (64 bytes for Ed25519)
        block.signature = Some(DydxSignature::new(vec![42u8; 64]));

        // Verify signature exists
        assert!(block.signature.is_some());
        assert_eq!(block.signature.as_ref().unwrap().bytes.len(), 64);
    }

    #[test]
    fn test_transaction_hash() {
        let tx = DydxTransaction::new(vec![1, 2, 3, 4, 5]);
        let hash = tx.hash();

        // Verify hash length
        assert_eq!(hash.0.len(), 32);

        // Verify transaction size
        assert_eq!(tx.size(), 5);

        // Verify hash is deterministic
        let hash2 = tx.hash();
        assert_eq!(hash.0, hash2.0);
    }

    #[test]
    fn test_transaction_empty() {
        let tx = DydxTransaction::new(vec![]);
        assert_eq!(tx.size(), 0);

        // Empty transaction should still have a hash
        let hash = tx.hash();
        assert_eq!(hash.0.len(), 32);
    }

    #[test]
    fn test_transaction_large() {
        let large_data = vec![7u8; 1_000_000]; // 1MB
        let tx = DydxTransaction::new(large_data.clone());
        assert_eq!(tx.size(), 1_000_000);
        assert_eq!(tx.data, large_data);
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

        // Verify ledger info methods
        assert_eq!(ledger_info.epoch(), 1);
        assert_eq!(ledger_info.round(), 100);
        assert_eq!(ledger_info.commit_info().version(), 5);
        assert_eq!(ledger_info.commit_info().block_id(), DydxHash([1u8; 32]));
        assert_eq!(ledger_info.accumulated_state, DydxHash([3u8; 32]));
    }

    #[test]
    fn test_ledger_info_fields() {
        let block_id = DydxHash([10u8; 32]);
        let state_root = DydxHash([20u8; 32]);
        let _accumulated_state = DydxHash([30u8; 32]);

        let commit_info = DydxCommitInfo {
            block_id,
            round: 500,
            epoch: 10,
            version: 1000,
            state_root,
            timestamp: 99999,
        };

        // Verify all fields
        assert_eq!(commit_info.block_id, block_id);
        assert_eq!(commit_info.round, 500);
        assert_eq!(commit_info.epoch, 10);
        assert_eq!(commit_info.version, 1000);
        assert_eq!(commit_info.state_root, state_root);
        assert_eq!(commit_info.timestamp, 99999);
    }

    #[test]
    fn test_vote_data_verify_valid_round_increment() {
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
    fn test_vote_data_verify_same_epoch() {
        let vote_data = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 5,
                round: 100,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 1000,
            },
            parent_block: DydxBlockMetadata {
                epoch: 5,
                round: 99,
                author: DydxNodeId([1u8; 32]),
                parent_id: DydxHash([2u8; 32]),
                timestamp: 900,
            },
        };
        assert!(vote_data.verify().is_ok());
    }

    #[test]
    fn test_vote_data_verify_invalid_round_increment() {
        let vote_data = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 5, // Wrong round - should be parent round + 1
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
        let result = vote_data.verify();
        assert!(result.is_err(), "Vote data verification should fail for invalid round increment");

        // Verify error message contains useful information
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Round increment invalid") || err_msg.contains("round"));
    }

    #[test]
    fn test_vote_data_verify_round_not_incremented() {
        let vote_data = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 1, // Same round as parent - should be parent round + 1
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
        assert!(vote_data.verify().is_err());
    }

    #[test]
    fn test_vote_data_verify_decremented_round() {
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
                round: 5, // Parent round greater than proposed - invalid
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 150,
            },
        };
        assert!(vote_data.verify().is_err());
    }

    #[test]
    fn test_signature_aggregate_empty() {
        let signatures: Vec<&DydxSignature> = vec![];
        let result = DydxSignature::aggregate(signatures.into_iter());

        // Should handle empty iterator gracefully
        assert!(result.is_ok());
        let aggregated = result.unwrap();
        assert!(aggregated.bytes.is_empty());
        assert!(aggregated.mask.is_empty());
    }

    #[test]
    fn test_signature_aggregate_single() {
        let sig1 = DydxSignature::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let aggregated = DydxSignature::aggregate(vec![&sig1].into_iter()).unwrap();

        assert_eq!(aggregated.bytes, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(aggregated.mask.is_empty());
    }

    #[test]
    fn test_signature_aggregate_multiple() {
        let sig1 = DydxSignature::new(vec![1, 2, 3]);
        let sig2 = DydxSignature::new(vec![4, 5, 6]);
        let sig3 = DydxSignature::new(vec![7, 8, 9]);
        let aggregated = DydxSignature::aggregate(vec![&sig1, &sig2, &sig3].into_iter()).unwrap();

        assert_eq!(aggregated.bytes, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_signature_aggregate_deterministic() {
        let sig1 = DydxSignature::new(vec![1, 2, 3]);
        let sig2 = DydxSignature::new(vec![4, 5, 6]);

        let aggregated1 = DydxSignature::aggregate(vec![&sig1, &sig2].into_iter()).unwrap();
        let aggregated2 = DydxSignature::aggregate(vec![&sig1, &sig2].into_iter()).unwrap();

        assert_eq!(aggregated1.bytes, aggregated2.bytes);
        assert_eq!(aggregated1.mask, aggregated2.mask);
    }

    #[test]
    fn test_public_key_from_bytes() {
        let bytes = vec![1u8; 32];
        let pk = DydxPublicKey::from_bytes(&bytes).unwrap();

        assert_eq!(pk.bytes, bytes);
    }

    #[test]
    fn test_public_key_to_bytes() {
        let bytes = vec![5u8; 32];
        let pk = DydxPublicKey::from_bytes(&bytes).unwrap();
        let converted = pk.to_bytes();

        assert_eq!(converted, bytes);
    }

    #[test]
    fn test_public_key_wrong_size() {
        let bytes = vec![1u8; 16]; // Wrong size for Ed25519
        let pk = DydxPublicKey::from_bytes(&bytes).unwrap();

        // Should accept any size in from_bytes (no validation on creation)
        assert_eq!(pk.bytes, bytes);
    }

    #[test]
    fn test_block_metadata() {
        let metadata = DydxBlockMetadata {
            epoch: 10,
            round: 100,
            author: DydxNodeId([1u8; 32]),
            parent_id: DydxHash([2u8; 32]),
            timestamp: 5000,
        };

        assert_eq!(metadata.epoch(), 10);
        assert_eq!(metadata.round(), 100);
        assert_eq!(metadata.author, DydxNodeId([1u8; 32]));
        assert_eq!(metadata.parent_id, DydxHash([2u8; 32]));
        assert_eq!(metadata.timestamp, 5000);
    }

    #[test]
    fn test_quorum_cert_has_quorum() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![1, 2, 3, 4],
                mask: vec![0x01],
            },
            vec![0x01], // 1 signer
            100, // total voting power
            75,  // signers voting power (75% > 66.67%)
        );

        assert!(qc.has_quorum());
        assert_eq!(qc.signer_count(), 1);
        assert!(qc.is_signer(0));
        assert!(!qc.is_signer(1));
    }

    #[test]
    fn test_quorum_cert_no_quorum() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![1, 2, 3, 4],
                mask: vec![0x01],
            },
            vec![0x01], // 1 signer
            100, // total voting power
            50,  // signers voting power (50% < 66.67%)
        );

        assert!(!qc.has_quorum());
        assert_eq!(qc.signer_count(), 1);
    }

    #[test]
    fn test_quorum_cert_verify_sufficient_voting_power() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![1; 192], // 3 signatures * 64 bytes
                mask: vec![0x07], // 3 signers (bits 0, 1, 2 set)
            },
            vec![0x07], // 3 signers
            100, // total voting power
            67,  // signers voting power (67% > 66.67%)
        );

        // Should pass verification with sufficient voting power
        assert!(qc.verify().is_ok());
    }

    #[test]
    fn test_quorum_cert_verify_insufficient_voting_power() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![1; 128], // 2 signatures * 64 bytes
                mask: vec![0x03], // 2 signers (bits 0, 1 set)
            },
            vec![0x03], // 2 signers
            100, // total voting power
            66,  // signers voting power (66% = 66.67% threshold, not >)
        );

        // Should fail verification - need > 2/3, not >= 2/3
        let result = qc.verify();
        assert!(result.is_err());

        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Insufficient voting power"));
    }

    #[test]
    fn test_quorum_cert_verify_zero_total_power() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![],
                mask: vec![],
            },
            vec![],
            0, // total voting power
            0,  // signers voting power
        );

        // Should fail with zero total power
        let result = qc.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_quorum_cert_verify_empty_signers_nonzero_power() {
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![],
                mask: vec![],
            },
            vec![],
            100, // total voting power
            50,  // signers voting power but no signers bitmask
        );

        // Should fail - bitmask empty but voting power non-zero
        let result = qc.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_quorum_cert_signer_count() {
        // Test with 5 signers (bits 0, 1, 2, 3, 4 set)
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![],
                mask: vec![],
            },
            vec![0x1F], // binary: 00011111
            100,
            50,
        );

        assert_eq!(qc.signer_count(), 5);
        assert!(qc.is_signer(0));
        assert!(qc.is_signer(1));
        assert!(qc.is_signer(2));
        assert!(qc.is_signer(3));
        assert!(qc.is_signer(4));
        assert!(!qc.is_signer(5));
        assert!(!qc.is_signer(7));
    }

    #[test]
    fn test_quorum_cert_multi_byte_signers() {
        // Test with signers across multiple bytes
        let qc = DydxQuorumCert::new(
            DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            DydxAggregatedSignature {
                bytes: vec![],
                mask: vec![],
            },
            vec![0xFF, 0x01], // All 8 bits in first byte, 1 bit in second
            100,
            50,
        );

        assert_eq!(qc.signer_count(), 9);
        assert!(qc.is_signer(0));
        assert!(qc.is_signer(7));
        assert!(qc.is_signer(8));
        assert!(!qc.is_signer(9));
    }

    #[test]
    fn test_quorum_cert_form_from_votes_quorum_reached() {
        use std::sync::Arc;

        // Create validator set (3 validators with equal power)
        let mut validator_set = std::collections::HashMap::new();
        validator_set.insert(vec![1u8; 32], (0, 100)); // Validator 0: 100 power
        validator_set.insert(vec![2u8; 32], (1, 100)); // Validator 1: 100 power
        validator_set.insert(vec![3u8; 32], (2, 100)); // Validator 2: 100 power

        let block_metadata = DydxBlockMetadata {
            epoch: 1,
            round: 5,
            author: DydxNodeId([0u8; 32]),
            parent_id: DydxHash([0u8; 32]),
            timestamp: 100,
        };

        let vote_data = DydxVoteData {
            proposed_block: block_metadata.clone(),
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 4,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };

        let ledger_info = DydxLedgerInfo {
            commit_info: DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round: 5,
                epoch: 1,
                version: 1,
                state_root: DydxHash([2u8; 32]),
                timestamp: 100,
            },
            accumulated_state: DydxHash([2u8; 32]),
        };

        // Create votes from 3 validators (300 total power, > 2/3 threshold)
        let votes = vec![
            Arc::new(DydxVote {
                author: DydxNodeId([1u8; 32]),
                vote_data: vote_data.clone(),
                ledger_info: ledger_info.clone(),
                signature: DydxSignature { bytes: vec![0u8; 64] },
                extension: vec![],
            }),
            Arc::new(DydxVote {
                author: DydxNodeId([2u8; 32]),
                vote_data: vote_data.clone(),
                ledger_info: ledger_info.clone(),
                signature: DydxSignature { bytes: vec![0u8; 64] },
                extension: vec![],
            }),
            Arc::new(DydxVote {
                author: DydxNodeId([3u8; 32]),
                vote_data: vote_data.clone(),
                ledger_info: ledger_info.clone(),
                signature: DydxSignature { bytes: vec![0u8; 64] },
                extension: vec![],
            }),
        ];

        let result = DydxQuorumCert::form_from_votes(&votes, &validator_set);
        assert!(result.is_ok());

        let qc = result.unwrap();
        assert_eq!(qc.certified_block.epoch, 1);
        assert_eq!(qc.certified_block.round, 5);
        assert_eq!(qc.total_voting_power, 300);
        assert_eq!(qc.signers_voting_power, 300);
        assert!(qc.has_quorum());
        assert_eq!(qc.signer_count(), 3);
        assert!(qc.is_signer(0));
        assert!(qc.is_signer(1));
        assert!(qc.is_signer(2));
    }

    #[test]
    fn test_quorum_cert_form_from_votes_no_quorum() {
        use std::sync::Arc;

        // Create validator set (3 validators with equal power)
        let mut validator_set = std::collections::HashMap::new();
        validator_set.insert(vec![1u8; 32], (0, 100)); // Validator 0: 100 power
        validator_set.insert(vec![2u8; 32], (1, 100)); // Validator 1: 100 power
        validator_set.insert(vec![3u8; 32], (2, 100)); // Validator 2: 100 power

        let block_metadata = DydxBlockMetadata {
            epoch: 1,
            round: 5,
            author: DydxNodeId([0u8; 32]),
            parent_id: DydxHash([0u8; 32]),
            timestamp: 100,
        };

        let vote_data = DydxVoteData {
            proposed_block: block_metadata.clone(),
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 4,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };

        let ledger_info = DydxLedgerInfo {
            commit_info: DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round: 5,
                epoch: 1,
                version: 1,
                state_root: DydxHash([2u8; 32]),
                timestamp: 100,
            },
            accumulated_state: DydxHash([2u8; 32]),
        };

        // Create votes from only 1 validator (100 power, not > 2/3 of 300)
        let votes = vec![Arc::new(DydxVote {
            author: DydxNodeId([1u8; 32]),
            vote_data: vote_data.clone(),
            ledger_info: ledger_info.clone(),
            signature: DydxSignature { bytes: vec![0u8; 64] },
            extension: vec![],
        })];

        let result = DydxQuorumCert::form_from_votes(&votes, &validator_set);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient voting power"));
    }

    #[test]
    fn test_quorum_cert_form_from_votes_empty() {
        let validator_set = std::collections::HashMap::new();
        let votes: Vec<std::sync::Arc<DydxVote>> = vec![];

        let result = DydxQuorumCert::form_from_votes(&votes, &validator_set);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No votes provided"));
    }

    #[test]
    fn test_quorum_cert_form_from_votes_mismatch() {
        use std::sync::Arc;

        let mut validator_set = std::collections::HashMap::new();
        validator_set.insert(vec![1u8; 32], (0, 100));
        validator_set.insert(vec![2u8; 32], (1, 100));

        let vote_data_1 = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 4,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };

        let vote_data_2 = DydxVoteData {
            proposed_block: DydxBlockMetadata {
                epoch: 1,
                round: 6, // Different round!
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 100,
            },
            parent_block: DydxBlockMetadata {
                epoch: 1,
                round: 5,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 50,
            },
        };

        let ledger_info = DydxLedgerInfo {
            commit_info: DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round: 5,
                epoch: 1,
                version: 1,
                state_root: DydxHash([2u8; 32]),
                timestamp: 100,
            },
            accumulated_state: DydxHash([2u8; 32]),
        };

        let votes = vec![
            Arc::new(DydxVote {
                author: DydxNodeId([1u8; 32]),
                vote_data: vote_data_1,
                ledger_info: ledger_info.clone(),
                signature: DydxSignature { bytes: vec![0u8; 64] },
                extension: vec![],
            }),
            Arc::new(DydxVote {
                author: DydxNodeId([2u8; 32]),
                vote_data: vote_data_2, // Different round!
                ledger_info: ledger_info.clone(),
                signature: DydxSignature { bytes: vec![0u8; 64] },
                extension: vec![],
            }),
        ];

        let result = DydxQuorumCert::form_from_votes(&votes, &validator_set);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Vote mismatch"));
    }
}

/// =============================================================================
/// Genesis & State Migration Utilities
/// =============================================================================

/// CometBFT Validator Set (from Tendermint genesis)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CometBFTValidatorSet {
    pub validators: Vec<CometBFTValidator>,
}

/// CometBFT Validator information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CometBFTValidator {
    /// Validator address (32 bytes)
    pub address: String,

    /// Voting power
    pub voting_power: u64,

    /// Proposer priority (from genesis)
    pub proposer_priority: Option<i64>,
}

/// dYdX ProposerElection strategy
///
/// Uses the dYdX validator set with weighted round-robin based on voting power.
/// This ensures fair proposer selection proportional to stake weight.
#[derive(Clone)]
pub struct DydxProposerElection {
    /// Validator addresses mapped to their voting power
    validators: HashMap<DydxNodeId, u64>,

    /// Total voting power
    total_power: u64,

    /// Current round number
    current_round: u64,

    /// Initial round number
    initial_round: u64,
}

impl DydxProposerElection {
    /// Create a new dYdX proposer election strategy.
    pub fn new(validators: HashMap<DydxNodeId, u64>, total_power: u64) -> Self {
        Self {
            validators,
            total_power,
            current_round: 1,
            initial_round: 1,
        }
    }

    /// Get a validator by address.
    pub fn get_validator(&self, address: &DydxNodeId) -> Option<&u64> {
        self.validators.get(address)
    }

    /// Get the number of validators.
    pub fn num_validators(&self) -> usize {
        self.validators.len()
    }
}

impl ProposerElection for DydxProposerElection {
    type Round = u64;
    type Author = DydxNodeId;

    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
        if self.validators.is_empty() {
            return None;
        }

        if *round < self.initial_round {
            return None;
        }

        // Sort validators by address for deterministic proposer selection
        let mut validator_list: Vec<(&DydxNodeId, &u64)> = self.validators.iter().collect();
        validator_list.sort_by(|a, b| a.0.cmp(b.0));

        // Select proposer using weighted round-robin
        let mut cumulative_power = 0u64;
        let target_power = (*round) % self.total_power;

        for (address, power) in &validator_list {
            cumulative_power += *power;
            if cumulative_power > target_power {
                return Some(**address);
            }
        }

        // Fallback to first validator if something went wrong
        validator_list.first().map(|(addr, _power)| **addr)
    }
}

/// Validator set information for migration from CometBFT to AptosBFT
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorSetInfo {
    /// Validator addresses
    pub validators: Vec<DydxNodeId>,

    /// Voting power for each validator
    pub voting_powers: Vec<u64>,

    /// Total voting power
    pub total_voting_power: u64,

    /// Proposer priorities (optional)
    pub proposer_priorities: Option<Vec<i64>>,
}

impl ValidatorSetInfo {
    /// Create a new validator set info
    pub fn new(
        validators: Vec<DydxNodeId>,
        voting_powers: Vec<u64>,
        total_voting_power: u64,
        proposer_priorities: Option<Vec<i64>>,
    ) -> Result<Self, DydxAdapterError> {
        if validators.len() != voting_powers.len() {
            return Err(DydxAdapterError::msg(format!(
                "Validators and voting powers length mismatch: {} vs {}",
                validators.len(),
                voting_powers.len()
            )));
        }

        let total_power: u64 = voting_powers.iter().sum();

        if total_power != total_voting_power {
            return Err(DydxAdapterError::msg(format!(
                "Sum of voting powers ({}) doesn't match total ({})",
                total_power,
                total_voting_power
            )));
        }

        Ok(Self {
            validators,
            voting_powers,
            total_voting_power,
            proposer_priorities,
        })
    }

    /// Convert from CometBFT genesis validators
    pub fn from_cometbft_validators(
        comet_validators: &CometBFTValidatorSet,
    ) -> Result<Self, DydxAdapterError> {
        let mut validators = vec![];
        let mut voting_powers = vec![];
        let mut proposer_priorities = vec![];

        // Sort by address for determinism
        let mut sorted_validators = comet_validators.validators.clone();
        sorted_validators.sort_by(|a, b| a.address.cmp(&b.address));

        for (_idx, val) in sorted_validators.iter().enumerate() {
            // CometBFT addresses are hex-encoded, need to decode first
            let address_bytes = hex::decode(&val.address)
                .map_err(|e| DydxAdapterError::msg(format!("Invalid hex address: {}", e)))?;

            let address = DydxNodeId::from_slice(&address_bytes)?;
            validators.push(address);
            voting_powers.push(val.voting_power);
            proposer_priorities.push(val.proposer_priority.unwrap_or(0));
        }

        let total_voting_power: u64 = voting_powers.iter().sum();

        Ok(Self {
            validators,
            voting_powers,
            total_voting_power,
            proposer_priorities: Some(proposer_priorities),
        })
    }

    /// Convert to HashMap for ProposerElection
    pub fn to_hash_map(&self) -> HashMap<DydxNodeId, u64> {
        self.validators
            .iter()
            .zip(self.voting_powers.iter())
            .map(|(addr, power)| (*addr, *power))
            .collect()
    }

    /// Create weighted round-robin proposer
    pub fn create_proposer_election(&self) -> DydxProposerElection {
        DydxProposerElection::new(
            self.to_hash_map(),
            self.total_voting_power,
        )
    }
}
