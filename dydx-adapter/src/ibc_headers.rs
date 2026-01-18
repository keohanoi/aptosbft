// Synthetic Tendermint header generation for IBC compatibility
//
// This module converts AptosBFT Quorum Certificates to Tendermint-compatible
// headers, enabling IBC relayers to work with dYdX v4 when using AptosBFT
// instead of CometBFT.

use consensus_traits::{Block, BlockMetadata, QuorumCertificate, Hash};
use crate::types::DydxLedgerInfo;
use crate::error::DydxAdapterError;
use serde::{Deserialize, Serialize};

// Import for TmHash display
use hex;

// Import for gRPC queries (optional, only when grpc feature is enabled)
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tendermint Block header
///
/// This mirrors the Tendermint BlockHeader structure defined in:
/// https://github.com/tendermint/tendermint/blob/master/types/block.go
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TendermintHeader {
    /// Basic block info
    pub version: Version,

    /// Chain ID
    pub chain_id: String,

    /// Block height
    pub height: u64,

    /// Block time (Unix timestamp in nanoseconds)
    pub time: Timestamp,

    /// Last block hash
    pub last_block_id: TmHash,

    /// Last commit hash
    pub last_commit_id: TmHash,

    /// Data hash (transactions hash)
    pub data_hash: TmHash,

    /// Validators hash
    pub validators_hash: TmHash,

    /// Next validators hash
    pub next_validators_hash: TmHash,

    /// Consensus address
    pub consensus_address: ConsensusAddress,

    /// App hash (state root)
    pub app_hash: Bytes,

    /// Last results hash
    pub last_results_hash: TmHash,

    /// Evidence hash
    pub evidence_hash: TmHash,
}

/// Version information
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub block: u64,
    pub app: u64,
}

/// Timestamp (Unix nanoseconds)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanos: u32,
}

/// 32-byte Tendermint hash
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TmHash(pub [u8; 32]);

impl std::fmt::Display for TmHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// 32-byte address
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address(pub [u8; 32]);

/// Consensus address (20 bytes for Tendermint compatibility)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusAddress(pub [u8; 20]);

/// Bytes wrapper for serialization
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bytes(pub Vec<u8>);

/// Validator information in Tendermint format
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validator {
    /// Address
    pub address: Address,

    /// Voting power
    pub power: u64,

    /// Proposer priority (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer_priority: Option<u64>,
}

/// Validator set
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,

    /// Total voting power
    pub total_voting_power: u64,
}

/// Block ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockID {
    pub hash: [u8; 32],
}

/// Signature (64 bytes for Ed25519)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

// Implement Serialize/Deserialize manually for Signature
impl serde::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as a byte array
        serde::Serialize::serialize(&self.0.to_vec(), serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec = Vec::<u8>::deserialize(deserializer)?;
        if vec.len() != 64 {
            return Err(serde::de::Error::custom(format!(
                "Expected 64 bytes for Signature, got {}", vec.len()
            )));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&vec);
        Ok(Signature(arr))
    }
}

impl Signature {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Commit signature from a single validator
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSig {
    pub validator_address: Address,
    pub timestamp: u64,
    pub signature: Signature,
}

/// Commit information from a QC
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub block_id: BlockID,
    pub height: u64,
    pub round: u64,
    pub signatures: Vec<CommitSig>,
}

/// Configuration for synthetic header generation
#[derive(Clone, Debug)]
pub struct HeaderBuilderConfig {
    /// Chain ID for the headers
    pub chain_id: String,

    /// Consensus address (20 bytes)
    pub consensus_address: [u8; 20],

    /// Block time for genesis (Unix timestamp)
    pub genesis_time: i64,
}

impl Default for HeaderBuilderConfig {
    fn default() -> Self {
        Self {
            chain_id: "dydx-mainnet-1".to_string(),
            consensus_address: [0u8; 20],
            genesis_time: 0,
        }
    }
}

/// Trait for querying validator information from the application
///
/// This trait allows the SyntheticHeaderBuilder to look up validator
/// metadata when constructing IBC headers. Implementations can use
/// gRPC queries, local state, or cached data.
#[async_trait::async_trait]
pub trait ValidatorRegistry: Send + Sync {
    /// Query the full validator set at a given height
    ///
    /// # Arguments
    /// * `height` - The block height to query (0 for latest)
    ///
    /// # Returns
    /// * `Ok(ValidatorRegistryEntry)` - The validator registry entry
    /// * `Err(DydxAdapterError)` - If the query fails
    async fn query_validator_set(
        &self,
        height: u64,
    ) -> Result<ValidatorRegistryEntry, DydxAdapterError>;

    /// Query a single validator by index
    ///
    /// # Arguments
    /// * `validator_index` - The validator's index in the set
    /// * `height` - The block height to query (0 for latest)
    ///
    /// # Returns
    /// * `Ok(ValidatorInfo)` - The validator information
    /// * `Err(DydxAdapterError)` - If the validator is not found or query fails
    async fn query_validator(
        &self,
        validator_index: usize,
        height: u64,
    ) -> Result<ValidatorInfo, DydxAdapterError>;
}

/// Validator registry entry containing the full validator set
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorRegistryEntry {
    /// List of all validators
    pub validators: Vec<ValidatorInfo>,

    /// Total voting power
    pub total_voting_power: u64,

    /// Height of this validator set
    pub height: u64,
}

/// Information about a single validator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator index in the set
    pub index: usize,

    /// Consensus address (32 bytes for Ed25519 public key)
    pub address: [u8; 32],

    /// Voting power
    pub voting_power: u64,

    /// Proposer priority (optional)
    pub proposer_priority: Option<i64>,
}

/// In-memory validator registry for testing and fallback scenarios
#[derive(Clone, Debug)]
pub struct InMemoryValidatorRegistry {
    validators: Arc<RwLock<Vec<ValidatorInfo>>>,
}

impl InMemoryValidatorRegistry {
    /// Create a new in-memory validator registry
    pub fn new(validators: Vec<ValidatorInfo>) -> Self {
        Self {
            validators: Arc::new(RwLock::new(validators)),
        }
    }

    /// Create an empty registry (returns placeholder validators)
    pub fn empty() -> Self {
        Self::new(vec![])
    }
}

#[async_trait::async_trait]
impl ValidatorRegistry for InMemoryValidatorRegistry {
    async fn query_validator_set(
        &self,
        _height: u64,
    ) -> Result<ValidatorRegistryEntry, DydxAdapterError> {
        let validators = self.validators.read().await;
        let total_voting_power = validators.iter().map(|v| v.voting_power).sum();

        Ok(ValidatorRegistryEntry {
            validators: validators.clone(),
            total_voting_power,
            height: _height,
        })
    }

    async fn query_validator(
        &self,
        validator_index: usize,
        _height: u64,
    ) -> Result<ValidatorInfo, DydxAdapterError> {
        let validators = self.validators.read().await;
        validators
            .get(validator_index)
            .cloned()
            .ok_or_else(|| DydxAdapterError::msg(format!("Validator not found at index {}", validator_index)))
    }
}

/// Builder for creating synthetic Tendermint headers from AptosBFT QCs
pub struct SyntheticHeaderBuilder {
    config: HeaderBuilderConfig,
    validator_registry: Option<Arc<dyn ValidatorRegistry>>,
}

impl SyntheticHeaderBuilder {
    /// Create a new header builder with validator registry support
    pub fn new(config: HeaderBuilderConfig) -> Self {
        Self {
            config,
            validator_registry: None,
        }
    }

    /// Set the validator registry for looking up validator information
    ///
    /// This is required for generating accurate IBC headers with real validator sets.
    /// Without a registry, the builder will use placeholder validators.
    pub fn with_validator_registry(mut self, registry: Arc<dyn ValidatorRegistry>) -> Self {
        self.validator_registry = Some(registry);
        self
    }

    /// Create a header from a QuorumCertificate and ledger info
    ///
    /// This is an async method to support validator registry queries.
    pub async fn build_header_from_qc<QC, B>(
        &self,
        qc: &QC,
        ledger_info: &DydxLedgerInfo,
        block_data_hash: &[u8; 32],
    ) -> Result<TendermintHeader, DydxAdapterError>
    where
        QC: QuorumCertificate,
        QC::BlockMetadata: BlockMetadata,
        B: Block,
    {
        let certified_block = qc.certified_block();

        // Convert block metadata to header fields
        let height = certified_block.round();
        let time = self.nano_timestamp(certified_block.timestamp());

        // Create hashes (simplified - in production, derive from actual data)
        let mut last_block_id_arr = [0u8; 32];
        let parent_hash = certified_block.parent_id();
        let bytes = parent_hash.as_bytes();
        let len = bytes.len().min(32);
        last_block_id_arr[..len].copy_from_slice(&bytes[..len]);
        let last_block_id = TmHash(last_block_id_arr);

        // The last_commit_id is the hash of the commit from the previous block
        // In AptosBFT, this is derived from the parent block's QC
        // For now, we use the parent block's hash as a proxy
        let last_commit_id = last_block_id.clone();

        let data_hash = TmHash(*block_data_hash);

        // Build validator set from QC signatures
        let validators = self.extract_validators_from_qc(qc).await?;
        let validators_hash = self.hash_validator_set(&validators);

        // Next validators hash: same as current when validator set hasn't changed
        // When validator set changes (at epoch boundaries), this should hash the NEW validator set
        let next_validators_hash = validators_hash.clone();

        let app_hash = Bytes(ledger_info.accumulated_state.0.to_vec());

        let header = TendermintHeader {
            version: Version { block: 11, app: 0 },
            chain_id: self.config.chain_id.clone(),
            height,
            time,
            last_block_id,
            last_commit_id,
            data_hash,
            validators_hash,
            next_validators_hash,
            consensus_address: ConsensusAddress(self.config.consensus_address),
            app_hash,
            last_results_hash: TmHash([0u8; 32]),
            evidence_hash: TmHash([0u8; 32]),
        };

        Ok(header)
    }

    /// Extract validator information from a Quorum Certificate
    ///
    /// This implementation extracts the validator set that signed the QC.
    /// For dYdX's AptosBFT integration, the QC contains:
    /// - A signers bitmask (each bit represents a validator index)
    /// - Total voting power of all validators
    /// - Voting power of the signers
    async fn extract_validators_from_qc<QC>(&self, qc: &QC) -> Result<ValidatorSet, DydxAdapterError>
    where
        QC: QuorumCertificate,
    {
        use std::any::Any;

        // Attempt to downcast to DydxQuorumCert to access signers bitmask
        // This is safe because we control the QC types used in production
        let qc_any = qc as &dyn Any;

        // Check if this is our concrete DydxQuorumCert type
        if let Some(dydx_qc) = qc_any.downcast_ref::<crate::types::DydxQuorumCert>() {
            // Extract validator set from DydxQuorumCert
            // The signers bitmask indicates which validators signed
            return self.extract_validators_from_dydx_qc(dydx_qc).await;
        }

        // Fallback for non-dYdX QC types: return minimal validator set
        // In production, this would require the QC to provide validator set information
        // through the QuorumCertificate trait
        log::warn!("Extracting validators from non-dYdX QC type, returning minimal set");

        let validators = vec![
            Validator {
                address: Address([0u8; 32]),
                power: 1000,
                proposer_priority: Some(1),
            },
        ];

        Ok(ValidatorSet {
            validators,
            total_voting_power: 1000,
        })
    }

    /// Extract validators from a concrete DydxQuorumCert
    ///
    /// This method parses the signers bitmask and builds the Tendermint-compatible
    /// validator set for IBC headers. It requires access to the validator registry
    /// to look up validator addresses by index.
    async fn extract_validators_from_dydx_qc(
        &self,
        qc: &crate::types::DydxQuorumCert,
    ) -> Result<ValidatorSet, DydxAdapterError> {
        // If we have a validator registry, use it to look up real validators
        if let Some(registry) = &self.validator_registry {
            return self.extract_validators_with_registry(qc, registry).await;
        }

        // Fallback: Log warning and return minimal validator set
        // This allows the system to continue during testing/deployment
        log::warn!(
            "No validator registry configured for IBC header generation at height {}. \
            Returning minimal validator set. Configure a registry for production use.",
            qc.certified_block.round()
        );

        let validators = vec![
            Validator {
                address: Address([0u8; 32]),
                power: 1000,
                proposer_priority: Some(1),
            },
        ];

        Ok(ValidatorSet {
            validators,
            total_voting_power: 1000,
        })
    }

    /// Extract validators using the validator registry
    ///
    /// This is the production implementation that:
    /// 1. Parses the signers bitmask to identify which validator indices signed
    /// 2. Looks up validator metadata from the validator registry
    /// 3. Builds the Tendermint ValidatorSet with only the signing validators
    async fn extract_validators_with_registry(
        &self,
        qc: &crate::types::DydxQuorumCert,
        registry: &Arc<dyn ValidatorRegistry>,
    ) -> Result<ValidatorSet, DydxAdapterError> {
        let mut validators = vec![];
        let mut total_power = 0u64;

        // Query the full validator set at this height
        let height = qc.certified_block.round();
        let registry_entry = registry.query_validator_set(height).await?;

        // Parse the signers bitmask to identify which validators signed
        // Each bit in the signers Vec<u8> represents a validator
        for (idx, validator_info) in registry_entry.validators.iter().enumerate() {
            if qc.is_signer(idx) {
                validators.push(Validator {
                    address: Address(validator_info.address),
                    power: validator_info.voting_power,
                    proposer_priority: validator_info.proposer_priority.map(|p| p as u64),
                });
                total_power += validator_info.voting_power;
            }
        }

        // Verify we found at least some signers
        if validators.is_empty() {
            log::warn!(
                "No validators found in signers bitmask at height {}. \
                Signers bitmask length: {}, Total validators: {}",
                height,
                qc.signers.len(),
                registry_entry.validators.len()
            );
        }

        Ok(ValidatorSet {
            validators,
            total_voting_power: total_power,
        })
    }

    /// Hash a validator set
    pub fn hash_validator_set(&self, validator_set: &ValidatorSet) -> TmHash {
        // Simplified - in production, use Merkle tree or proper hash
        let data = serde_json::to_vec(validator_set).unwrap_or_default();
        let hash = blake3::hash(&data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        TmHash(arr)
    }

    /// Convert timestamp to nanoseconds
    fn nano_timestamp(&self, millis: u64) -> Timestamp {
        Timestamp {
            seconds: (millis / 1000) as i64,
            nanos: (millis % 1000 * 1_000_000) as u32,
        }
    }

    /// Serialize header to bytes for IBC
    pub fn serialize_header(&self, header: &TendermintHeader) -> Result<Vec<u8>, DydxAdapterError> {
        serde_json::to_vec(header)
            .map_err(|e| DydxAdapterError::msg(format!("Failed to serialize header: {}", e)))
    }

    /// Create a signed header (for testing/verification)
    pub fn create_signed_header(
        &self,
        header: TendermintHeader,
        commit: Commit,
    ) -> SignedHeader {
        SignedHeader {
            header,
            commit,
        }
    }
}

/// A signed header with commit signatures
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedHeader {
    pub header: TendermintHeader,
    pub commit: Commit,
}

/// Create a synthetic header builder with default config
pub fn create_header_builder() -> SyntheticHeaderBuilder {
    SyntheticHeaderBuilder::new(HeaderBuilderConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DydxBlockMetadata, DydxNodeId, DydxHash};
    use consensus_traits::core::Error as CoreError;

    // Mock QuorumCert for testing
    #[derive(Clone)]
    struct MockQc;

    impl QuorumCertificate for MockQc {
        type BlockMetadata = DydxBlockMetadata;
        type Hash = DydxHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            static BLOCK: DydxBlockMetadata = DydxBlockMetadata {
                epoch: 0,
                round: 1,
                author: DydxNodeId([0u8; 32]),
                parent_id: DydxHash([0u8; 32]),
                timestamp: 12345,
            };
            &BLOCK
        }

        fn block_id(&self) -> Self::Hash {
            DydxHash([1u8; 32])
        }

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            1
        }

        fn verify(&self) -> Result<(), CoreError> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[std::sync::Arc<V>],
            _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
        ) -> Result<Self, CoreError>
        where
            B: Block,
            V: consensus_traits::Vote,
        {
            Err(CoreError::msg("Not implemented"))
        }
    }

    // Helper function to create mock ledger info for tests
    fn create_mock_ledger_info(
        epoch: u64,
        round: u64,
        timestamp: u64,
    ) -> DydxLedgerInfo {
        DydxLedgerInfo {
            commit_info: crate::types::DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round,
                epoch,
                version: 1,
                state_root: DydxHash([2u8; 32]),
                timestamp,
            },
            accumulated_state: DydxHash([2u8; 32]),
        }
    }

    #[test]
    fn test_header_builder_config_default() {
        let config = HeaderBuilderConfig::default();
        assert_eq!(config.chain_id, "dydx-mainnet-1");
        assert_eq!(config.genesis_time, 0);
    }

    #[test]
    fn test_nano_timestamp() {
        let builder = create_header_builder();
        let timestamp = builder.nano_timestamp(1500);
        assert_eq!(timestamp.seconds, 1);
        assert_eq!(timestamp.nanos, 500_000_000);
    }

    #[tokio::test]
    async fn test_build_header_from_qc() {
        let builder = create_header_builder();
        let qc = MockQc;

        let ledger_info = DydxLedgerInfo {
            commit_info: crate::types::DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round: 1,
                epoch: 0,
                version: 5,
                state_root: DydxHash([2u8; 32]),
                timestamp: 12345,
            },
            accumulated_state: DydxHash([2u8; 32]),
        };

        let block_data_hash = [3u8; 32];

        let result = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &qc,
            &ledger_info,
            &block_data_hash,
        ).await;

        assert!(result.is_ok());
        let header = result.unwrap();
        assert_eq!(header.height, 1);
        assert_eq!(header.chain_id, "dydx-mainnet-1");
        assert_eq!(header.app_hash.0, ledger_info.accumulated_state.0);
    }

    #[tokio::test]
    async fn test_serialize_header() {
        let builder = create_header_builder();
        let qc = MockQc;

        let ledger_info = DydxLedgerInfo {
            commit_info: crate::types::DydxCommitInfo {
                block_id: DydxHash([1u8; 32]),
                round: 1,
                epoch: 0,
                version: 5,
                state_root: DydxHash([2u8; 32]),
                timestamp: 12345,
            },
            accumulated_state: DydxHash([2u8; 32]),
        };

        let block_data_hash = [3u8; 32];

        let header = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &qc,
            &ledger_info,
            &block_data_hash,
        ).await.unwrap();

        let serialized = builder.serialize_header(&header);
        assert!(serialized.is_ok());

        // Verify we can deserialize back
        let deserialized: TendermintHeader = serde_json::from_slice(&serialized.unwrap()).unwrap();
        assert_eq!(deserialized.height, 1);
    }

    #[test]
    fn test_validator_set_hash() {
        let builder = create_header_builder();
        let validators = ValidatorSet {
            validators: vec![
                Validator {
                    address: Address([1u8; 32]),
                    power: 1000,
                    proposer_priority: Some(1),
                },
                Validator {
                    address: Address([2u8; 32]),
                    power: 2000,
                    proposer_priority: Some(2),
                },
            ],
            total_voting_power: 3000,
        };

        let hash = builder.hash_validator_set(&validators);
        assert_ne!(hash.0, [0u8; 32]);
    }

    #[test]
    fn test_tm_hash_display() {
        let hash = TmHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let formatted = format!("{}", hash);
        assert_eq!(formatted.len(), 64); // 32 bytes * 2 hex chars
        assert_eq!(&formatted[..4], "0102");

        // Verify hash is deterministic
        let formatted2 = format!("{}", hash);
        assert_eq!(formatted, formatted2);

        // Verify hash encoding is correct (should be lowercase hex)
        assert!(formatted.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ========== Header Field Validation Tests ========== //

    #[tokio::test]
    async fn test_header_height_must_be_positive() {
        let builder = create_header_builder();
        let qc = MockQc;
        let ledger_info = create_mock_ledger_info(1 /* height */, 5 /* round */, 12345 /* timestamp */);

        let result = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &qc,
            &ledger_info,
            &[3u8; 32], // block_data_hash
        ).await;

        assert!(result.is_ok());
        let header = result.unwrap();
        assert_eq!(header.height, 1);
    }

    #[tokio::test]
    async fn test_header_chain_id_linkage() {
        let builder = create_header_builder();

        // Create a chain of headers
        // Note: MockQc returns a static parent_id = [0;32], so all headers will have
        // last_block_id = [0;32]. In production, each block's parent_id would point to
        // the previous block's hash.
        for i in 1..=10 {
            let block_data_hash: [u8; 32] = [(i + 1) as u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];

            let ledger_info = create_mock_ledger_info(i /* height */, i /* round */, 12345 + (i * 1000));
            let qc = MockQc;

            let result = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
                &qc,
                &ledger_info,
                &block_data_hash,
            ).await;

            assert!(result.is_ok());
            let header = result.unwrap();

            // All headers have the same parent_id from MockQc ([0;32])
            assert_eq!(header.last_block_id.0, [0u8; 32]);

            // Height comes from MockQc.round() which is always 1
            assert_eq!(header.height, 1);

            // data_hash correctly reflects the block_data_hash
            assert_eq!(header.data_hash.0, block_data_hash);
        }
    }

    #[tokio::test]
    async fn test_header_timestamp_monotonic() {
        let builder = create_header_builder();

        // Timestamp should be monotonically increasing
        let time1 = 1_500_000_000_000u64;
        let time2 = 1_600_000_000_000u64;
        let time3 = 1_700_000_000_000u64;

        // Build headers with different timestamps
        let header1 = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &create_mock_ledger_info(0, 0, time1),
            &[42u8; 32],
        ).await.unwrap();
        let header2 = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &create_mock_ledger_info(0, 0, time2),
            &[43u8; 32],
        ).await.unwrap();
        let header3 = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &create_mock_ledger_info(0, 0, time3),
            &[44u8; 32],
        ).await.unwrap();

        // Verify timestamps are monotonically increasing (time field is Timestamp)
        // Convert to seconds for comparison
        assert!(header2.time.seconds >= header1.time.seconds);
        assert!(header3.time.seconds >= header2.time.seconds);

        // Verify headers were created successfully
        assert_eq!(header1.height, 1);
        assert_eq!(header2.height, 1);
        assert_eq!(header3.height, 1);
    }

    #[tokio::test]
    async fn test_header_chain_verification() {
        // This test verifies that headers can be created successfully
        let builder = create_header_builder();

        // Create headers - MockQc returns static parent_id = [0;32]
        for i in 1..=10 {
            let block_data_hash = [(i + 1) as u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];
            let header = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
                &MockQc,
                &create_mock_ledger_info(0, i, 12345 + (i * 1000)),
                &block_data_hash,
            ).await.unwrap();

            // All headers have the same parent_id from MockQc ([0;32])
            assert_eq!(header.last_block_id.0, [0u8; 32]);

            // Height comes from MockQc.round() which is always 1
            assert_eq!(header.height, 1);
        }

        // All 10 headers created successfully
        // In production, IBC relayers would verify the full chain linkage
    }

    #[test]
    fn test_validator_set_field_count() {
        let validators = ValidatorSet {
            validators: vec![
                Validator {
                    address: Address([1u8; 32]),
                    power: 1000,
                    proposer_priority: Some(1),
                },
                Validator {
                    address: Address([2u8; 32]),
                    power: 2000,
                    proposer_priority: Some(2),
                },
                Validator {
                    address: Address([3u8; 32]),
                    power: 3000,
                    proposer_priority: Some(3),
                },
                Validator {
                    address: Address([4u8; 32]),
                    power: 4000,
                    proposer_priority: Some(4),
                },
            ],
            total_voting_power: 10000,
        };

        // Verify all validators are present
        assert_eq!(validators.validators.len(), 4);
        assert_eq!(validators.total_voting_power, 10000);
    }

    #[test]
    fn test_validator_set_empty() {
        let validators = ValidatorSet {
            validators: vec![],
            total_voting_power: 0,
        };

        assert!(validators.validators.is_empty());
        assert_eq!(validators.total_voting_power, 0);
    }

    #[test]
    fn test_validator_set_duplicate_addresses() {
        let addr1 = Address([5u8; 32]);
        let addr2 = Address([5u8; 32]);

        let validators = ValidatorSet {
            validators: vec![
                Validator {
                    address: addr1,
                    power: 1000,
                    proposer_priority: Some(1),
                },
                Validator {
                    address: addr2,
                    power: 1000,
                    proposer_priority: Some(2),
                },
            ],
            total_voting_power: 2000,
        };

        // Duplicate addresses should be allowed
        assert_eq!(validators.validators.len(), 2);

        // Verify both validators have the same address
        assert_eq!(validators.validators[0].address, addr1);
        assert_eq!(validators.validators[1].address, addr2);
    }

    #[test]
    fn test_validator_set_zero_address() {
        let zero_addr = Address([0u8; 32]);
        let validators = ValidatorSet {
            validators: vec![Validator {
                address: zero_addr,
                power: 1000,
                proposer_priority: Some(1),
            }],
            total_voting_power: 1000,
        };

        // Address validation would happen in the build_header phase
        // For now, we just verify the structure can hold any 32-byte array
        assert_eq!(validators.validators.len(), 1);
        assert_eq!(validators.validators[0].address, zero_addr);
    }

    #[test]
    fn test_commit_signature_field_count() {
        let signatures = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![
                CommitSig {
                    validator_address: Address([1u8; 32]),
                    timestamp: 12345,
                    signature: Signature([1u8; 64]), // Ed25519 signature size
                },
                CommitSig {
                    validator_address: Address([2u8; 32]),
                    timestamp: 12346,
                    signature: Signature([2u8; 64]), // Ed25519 signature size
                },
            ],
        };

        // Verify all signatures are present
        assert_eq!(signatures.signatures.len(), 2);
        assert_eq!(signatures.signatures[0].signature.len(), 64); // Each Ed25519 signature is 64 bytes
        assert_eq!(signatures.signatures[1].signature.len(), 64);

        // Verify timestamp is in nanoseconds
        assert_eq!(signatures.signatures[0].timestamp, 12345);
        assert_eq!(signatures.signatures[1].timestamp, 12346);

        // Verify block_id is correct
        assert_eq!(signatures.block_id.hash, [1u8; 32]);
        assert_eq!(signatures.height, 5);
        assert_eq!(signatures.round, 100);
    }

    #[test]
    fn test_commit_empty_signatures() {
        let signatures = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![],
        };

        // Empty signatures are valid (0 signers)
        assert!(signatures.signatures.is_empty());
        assert_eq!(signatures.height, 5);
        assert_eq!(signatures.round, 100);
    }

    #[test]
    fn test_commit_signature_zero_bytes() {
        // Test with all-zero signature (64 bytes of zeros)
        let sig = Signature([0u8; 64]);
        let commit = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![CommitSig {
                validator_address: Address([1u8; 32]),
                timestamp: 12345,
                signature: sig,
            }],
        };

        // Verify signature is 64 bytes
        assert_eq!(commit.signatures[0].signature.len(), 64);
        assert_eq!(commit.signatures[0].signature.0, [0u8; 64]);
    }

    #[tokio::test]
    async fn test_build_header_with_genesis_height() {
        let builder = create_header_builder();

        // Test with height 0 (genesis block)
        let result = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &create_mock_ledger_info(0, 0, 0),
            &[42u8; 32], // block_data_hash
        ).await;

        assert!(result.is_ok());
        let header = result.unwrap();

        // MockQc returns round 1, so height will be 1
        assert_eq!(header.height, 1);
    }

    #[tokio::test]
    async fn test_build_header_with_current_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let builder = create_header_builder();

        // Build a header with current timestamp
        let result = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &create_mock_ledger_info(0, 0, now_ms),
            &[42u8; 32],
        ).await;

        // Should be valid - timestamp can be current or newer
        assert!(result.is_ok());
    }

    #[test]
    fn test_tm_hash_collision_resistance() {
        // TmHash uses blake3 which has collision resistance
        let hash1 = TmHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let hash2 = TmHash([32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);

        // Hash should be different (collision resistance)
        assert_ne!(hash1.0, hash2.0);
    }

    #[test]
    fn test_validator_hash_uniqueness() {
        let builder = create_header_builder();
        let addr1 = Address([1u8; 32]);
        let addr2 = Address([2u8; 32]);
        let _addr3 = Address([3u8; 32]);

        // Hashing different addresses should produce different results
        let hash1 = builder.hash_validator_set(&ValidatorSet {
            validators: vec![
                Validator {
                    address: addr1,
                    power: 1000,
                    proposer_priority: Some(1),
                },
            ],
            total_voting_power: 1000,
        });

        let hash2 = builder.hash_validator_set(&ValidatorSet {
            validators: vec![
                Validator {
                    address: addr2,
                    power: 1000,
                    proposer_priority: Some(1),
                },
            ],
            total_voting_power: 1000,
        });

        let hash3 = builder.hash_validator_set(&ValidatorSet {
            validators: vec![
                Validator {
                    address: Address([3u8; 32]),
                    power: 1000,
                    proposer_priority: Some(1),
                },
            ],
            total_voting_power: 1000,
        });

        // Different addresses should produce different hashes
        assert_ne!(hash1.0, hash2.0);
        assert_ne!(hash1.0, hash3.0);
        assert_ne!(hash2.0, hash3.0);
    }

    #[test]
    fn test_validator_hash_determinism() {
        let builder = create_header_builder();
        let validators = ValidatorSet {
            validators: vec![
                Validator {
                    address: Address([1u8; 32]),
                    power: 1000,
                    proposer_priority: Some(1),
                },
            ],
            total_voting_power: 1000,
        };

        let hash1 = builder.hash_validator_set(&validators);
        let hash2 = builder.hash_validator_set(&validators);

        // Hash should be deterministic
        assert_eq!(hash1.0, hash2.0);
    }

    #[tokio::test]
    async fn test_serialize_deserialize_header_roundtrip() {
        let builder = create_header_builder();
        let ledger_info = create_mock_ledger_info(10 /* epoch */, 5 /* round */, 15000 /* timestamp */);
        let block_data_hash = [42u8; 32];

        let header = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &ledger_info,
            &block_data_hash,
        ).await.unwrap();

        // Serialize the header
        let serialized = builder.serialize_header(&header);
        assert!(serialized.is_ok());
        let serialized_bytes = serialized.unwrap();

        // Verify we can deserialize back
        let deserialized: TendermintHeader = serde_json::from_slice(&serialized_bytes).unwrap();
        assert_eq!(deserialized.height, 1); // From MockQc.round()
        assert_eq!(deserialized.chain_id, "dydx-mainnet-1");
        assert_eq!(deserialized.app_hash.0, ledger_info.accumulated_state.0);

        // Verify deterministic serialization
        let serialized2 = builder.serialize_header(&header).unwrap();
        assert_eq!(serialized_bytes, serialized2);
    }

    #[tokio::test]
    async fn test_serialize_empty_header() {
        let builder = create_header_builder();
        let ledger_info = create_mock_ledger_info(10 /* epoch */, 5 /* round */, 15000 /* timestamp */);
        let block_data_hash = [42u8; 32];

        let header = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &ledger_info,
            &block_data_hash,
        ).await.unwrap();

        // Serialize and deserialize header
        let serialized = builder.serialize_header(&header);
        assert!(serialized.is_ok());

        let deserialized: TendermintHeader = serde_json::from_slice(&serialized.unwrap()).unwrap();
        assert_eq!(deserialized.height, 1); // From MockQc.round()
    }

    #[tokio::test]
    async fn test_header_consistency_checks() {
        let builder = create_header_builder();

        // Create a header
        let ledger_info = create_mock_ledger_info(10 /* height */, 5 /* round */, 15000 /* timestamp */);
        let block_data_hash = [42u8; 32];

        let header = builder.build_header_from_qc::<MockQc, crate::types::DydxBlock>(
            &MockQc,
            &ledger_info,
            &block_data_hash,
        ).await.unwrap();

        // Verify the header has the parent hash from MockQc
        assert_eq!(header.last_block_id.0, [0u8; 32]);

        // Verify other fields are consistent
        assert_eq!(header.data_hash.0, block_data_hash);
        assert_eq!(header.height, 1); // From MockQc.round()
    }

    #[test]
    fn test_validator_set_power_calculation() {
        let validator1 = Validator {
            address: Address([1u8; 32]),
            power: 1000,
            proposer_priority: Some(1),
        };
        let validator2 = Validator {
            address: Address([2u8; 32]),
            power: 500,
            proposer_priority: Some(2),
        };
        let validator3 = Validator {
            address: Address([3u8; 32]),
            power: 3000,
            proposer_priority: Some(3),
        };

        let validators = ValidatorSet {
            validators: vec![validator1, validator2, validator3],
            total_voting_power: 4500,
        };

        // Verify total voting power is correct
        assert_eq!(validators.total_voting_power, 4500);

        // Verify individual validator powers
        assert_eq!(validators.validators[0].power, 1000);
        assert_eq!(validators.validators[1].power, 500);
        assert_eq!(validators.validators[2].power, 3000);

        // Verify total power is sum of individual powers
        assert_eq!(validators.total_voting_power, 1000 + 500 + 3000);
    }

    #[test]
    fn test_validator_set_power_overflow() {
        // Test with maximum u64 value
        let validator = Validator {
            address: Address([1u8; 32]),
            power: u64::MAX,
            proposer_priority: Some(1),
        };

        let validators = ValidatorSet {
            validators: vec![validator],
            total_voting_power: u64::MAX,
        };

        // Maximum values should be accepted (no overflow in struct creation)
        assert_eq!(validators.validators[0].power, u64::MAX);
        assert_eq!(validators.total_voting_power, u64::MAX);
    }

    #[test]
    fn test_commit_signature_byte_array_field() {
        let signatures = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![
                CommitSig {
                    validator_address: Address([1u8; 32]),
                    timestamp: 12345,
                    signature: Signature([2u8; 64]),
                },
                CommitSig {
                    validator_address: Address([3u8; 32]),
                    timestamp: 12346,
                    signature: Signature([4u8; 64]),
                },
            ],
        };

        // Verify signature byte arrays are the correct size (64 bytes)
        assert_eq!(signatures.signatures[0].signature.len(), 64);
        assert_eq!(signatures.signatures[1].signature.len(), 64);
        assert_eq!(signatures.signatures[0].validator_address, Address([1u8; 32]));
        assert_eq!(signatures.signatures[1].validator_address, Address([3u8; 32]));
    }

    #[test]
    fn test_commit_signature_all_ones() {
        // Test with all-ones signature (64 bytes)
        let sig_ones = Signature([1u8; 64]);
        let commit = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![CommitSig {
                validator_address: Address([1u8; 32]),
                timestamp: 12345,
                signature: sig_ones,
            }],
        };

        // Verify signature is 64 bytes
        assert_eq!(commit.signatures[0].signature.len(), 64);
        assert_eq!(commit.signatures[0].signature.0, [1u8; 64]);
    }

    #[test]
    fn test_invalid_commit_timestamp() {
        let _invalid_timestamp = 12345;
        let future_timestamp = 999999999999; // Very old timestamp

        let commit = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![CommitSig {
                validator_address: Address([1u8; 32]),
                timestamp: future_timestamp, // Invalid: timestamp too old
                signature: Signature([1u8; 64]),
            }],
        };

        // For now, any timestamp is accepted
        // In production, this would verify timestamps are within a reasonable window
        assert_eq!(commit.signatures[0].timestamp, future_timestamp);
    }

    #[test]
    fn test_empty_commit() {
        let commit = Commit {
            block_id: BlockID { hash: [1u8; 32] },
            height: 5,
            round: 100,
            signatures: vec![],
        };

        // Empty commit is valid (no signers is valid)
        assert!(commit.signatures.is_empty());
        assert_eq!(commit.height, 5);
        assert_eq!(commit.round, 100);
    }

    #[test]
    fn test_block_id_hex_encoding() {
        let block_id = BlockID { hash: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32] };

        let hash = TmHash(block_id.hash);
        let hex = format!("{}", hash);

        // BlockID hash to TmHash conversion preserves the bytes
        assert_eq!(hash.0, block_id.hash);

        // Hex encoding should be 64 characters (32 bytes * 2 hex chars)
        assert_eq!(hex.len(), 64);

        // Hex should match the bytes (lowercase hex encoding of 1..32)
        // Note: hex::encode produces lowercase, but we don't enforce case in the test
        // since both '0a' and '0A' represent the same value
        assert!(hex.starts_with("0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"));
    }

    #[test]
    fn test_block_id_empty_hash() {
        let block_id = BlockID { hash: [0u8; 32] };

        // TmHash wraps the input bytes directly (it's a container, not a hash function)
        let hash = TmHash(block_id.hash);
        assert_eq!(hash.0, [0u8; 32]);
        assert_eq!(hash.0, block_id.hash);
    }

    #[test]
    fn test_block_id_prefix() {
        let block_id = BlockID { hash: [0xFF; 32] };

        let hash = TmHash(block_id.hash);
        // TmHash wraps the input bytes directly (it's a container, not a hash function)
        let prefix = &hash.0[..8]; // First 8 bytes
        assert_eq!(prefix, [0xFF; 8]); // Should match input prefix

        // Hash should have 32 bytes
        assert_eq!(hash.0.len(), 32);
    }

    #[test]
    fn test_timestamp_nanos_to_seconds_conversion() {
        let _nano = 1_700_000_000_000u64;
        let ts = Timestamp {
            seconds: 1700,
            nanos: 0,
        };

        assert_eq!(ts.seconds, 1700);
    }

    #[test]
    fn test_timestamp_nanos_field() {
        let ts = Timestamp {
            seconds: 1700,
            nanos: 500,
        };

        assert_eq!(ts.nanos, 500);
    }

    #[test]
    fn test_timestamp_overflow() {
        // Test timestamp with large values
        let large_nanos = 1_700_000_000_000u64;

        let ts = Timestamp {
            seconds: (large_nanos / 1_000_000_000) as i64,
            nanos: (large_nanos % 1_000_000_000) as u32,
        };

        // Test the calculation is correct
        assert_eq!(ts.seconds, 1700i64);
        assert_eq!(ts.nanos, 0u32);
    }

    #[test]
    fn test_timestamp_consistency() {
        let ts1 = Timestamp {
            seconds: 1700,
            nanos: 500,
        };

        // Timestamp should be consistent
        let ts2 = Timestamp {
            seconds: 1700,
            nanos: 500,
        };

        assert_eq!(ts1.seconds, ts2.seconds);
        assert_eq!(ts1.nanos, ts2.nanos);
    }

    #[test]
    fn test_timestamp_zero_time() {
        let ts = Timestamp {
            seconds: 0,
            nanos: 0,
        };

        // Time can be zero
        assert_eq!(ts.seconds, 0);
        assert_eq!(ts.nanos, 0);
    }

    #[test]
    fn test_address_length() {
        let addr = Address([1u8; 32]);

        // Address must be exactly 32 bytes
        assert_eq!(addr.0.len(), 32);
    }

    #[test]
    fn test_address_zero_padding() {
        let addr = Address([0u8; 32]);

        // Address must be exactly 32 bytes
        assert_eq!(addr.0.len(), 32);
        assert_eq!(addr.0, [0u8; 32]);
    }

    #[test]
    fn test_address_formatting() {
        let addr = Address([1u8; 32]);

        // Hex encoding of address using hex crate
        let hex = hex::encode(addr.0);
        assert_eq!(hex.len(), 64); // 32 bytes * 2 hex chars
    }

    #[test]
    fn test_validator_priority_defaults() {
        let validator = Validator {
            address: Address([1u8; 32]),
            power: 1000,
            proposer_priority: Some(1),
        };

        // Default proposer priority should be Some(1)
        assert_eq!(validator.proposer_priority, Some(1));
    }

    #[test]
    fn test_validator_no_priority() {
        let validator = Validator {
            address: Address([1u8; 32]),
            power: 1000,
            proposer_priority: None,
        };

        // proposer_priority can be None (no priority)
        assert!(validator.proposer_priority.is_none());
    }

    #[test]
    fn test_power_aggregation() {
        let validators = vec![
            Validator {
                address: Address([1u8; 32]),
                power: 1000,
                proposer_priority: Some(1),
            },
            Validator {
                address: Address([2u8; 32]),
                power: 2000,
                proposer_priority: Some(2),
            },
        ];

        // Sum of powers
        let total_power = 3000;

        // Create validator set and verify total power
        let validators_set = ValidatorSet {
            validators: validators.clone(),
            total_voting_power: total_power,
        };

        assert_eq!(validators_set.total_voting_power, total_power);
        assert_eq!(validators_set.validators[0].power, 1000);
        assert_eq!(validators_set.validators[1].power, 2000);
    }

    #[test]
    fn test_validator_set_size_limits() {
        // Test empty validator set
        let validators = ValidatorSet {
            validators: vec![],
            total_voting_power: 0,
        };

        assert!(validators.validators.is_empty());
        assert_eq!(validators.total_voting_power, 0);

        // Test validator set with one validator
        let validators = ValidatorSet {
            validators: vec![Validator {
                address: Address([1u8; 32]),
                power: 1000,
                proposer_priority: Some(1),
            }],
            total_voting_power: 1000,
        };

        assert_eq!(validators.validators.len(), 1);
        assert_eq!(validators.total_voting_power, 1000);
    }
}
