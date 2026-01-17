// Synthetic Tendermint header generation for IBC compatibility
//
// This module converts AptosBFT Quorum Certificates to Tendermint-compatible
// headers, enabling IBC relayers to work with dYdX v4 when using AptosBFT
// instead of CometBFT.

use consensus_traits::{Block, BlockMetadata, QuorumCertificate, LedgerInfo, Hash};
use consensus_traits::core::Error as CoreError;
use crate::types::DydxLedgerInfo;
use crate::error::DydxAdapterError;
use serde::{Deserialize, Serialize};
use std::fmt;

// Import for TmHash display
use hex;

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// Commit information from a QC
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub block_id: TmHash,
    pub signatures: Vec<Vec<u8>>,
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

/// Builder for creating synthetic Tendermint headers from AptosBFT QCs
pub struct SyntheticHeaderBuilder {
    config: HeaderBuilderConfig,
}

impl SyntheticHeaderBuilder {
    /// Create a new header builder
    pub fn new(config: HeaderBuilderConfig) -> Self {
        Self { config }
    }

    /// Create a header from a QuorumCertificate and ledger info
    pub fn build_header_from_qc<QC, B>(
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

        let last_commit_id = TmHash([0u8; 32]); // TODO: Derive from previous commit

        let data_hash = TmHash(*block_data_hash);

        // Build validator set from QC signatures
        let validators = self.extract_validators_from_qc(qc)?;
        let validators_hash = self.hash_validator_set(&validators);

        let next_validators_hash = validators_hash.clone(); // TODO: Handle validator set changes

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
    fn extract_validators_from_qc<QC>(&self, _qc: &QC) -> Result<ValidatorSet, DydxAdapterError>
    where
        QC: QuorumCertificate,
    {
        // TODO: Extract validators from the QC
        // This requires parsing the QC's aggregated signature
        // to identify which validators signed

        // For now, return a placeholder validator set
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

    /// Hash a validator set
    fn hash_validator_set(&self, validator_set: &ValidatorSet) -> TmHash {
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

    #[test]
    fn test_build_header_from_qc() {
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
        );

        assert!(result.is_ok());
        let header = result.unwrap();
        assert_eq!(header.height, 1);
        assert_eq!(header.chain_id, "dydx-mainnet-1");
        assert_eq!(header.app_hash.0, ledger_info.accumulated_state().0);
    }

    #[test]
    fn test_serialize_header() {
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
        ).unwrap();

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
    }
}
