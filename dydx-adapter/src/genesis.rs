// Genesis and State Migration Utilities
//
// This module provides utilities for migrating from CometBFT to AptosBFT,
// including parsing CometBFT genesis files and converting validator sets.

use crate::types::{CometBFTValidatorSet, CometBFTValidator, ValidatorSetInfo};
use crate::error::DydxAdapterError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// CometBFT genesis file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CometBFTGenesis {
    /// Chain ID
    #[serde(rename = "chain_id")]
    pub chain_id: String,

    /// Genesis time
    pub genesis_time: String,

    /// Initial height
    #[serde(default = "default_initial_height")]
    pub initial_height: i64,

    /// Validators
    #[serde(default)]
    pub validators: Vec<CometBFTGenesisValidator>,

    /// App state (raw JSON)
    #[serde(default)]
    pub app_state: serde_json::Value,

    /// Consensus parameters
    #[serde(default)]
    pub consensus_params: Option<ConsensusParams>,
}

fn default_initial_height() -> i64 {
    1
}

/// CometBFT validator from genesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CometBFTGenesisValidator {
    /// Validator address
    pub address: String,

    /// PubKey
    pub pub_key: Option<PubKey>,

    /// Voting power
    #[serde(default)]
    pub power: i64,

    /// Proposer priority
    pub proposer_priority: Option<i64>,
}

/// Public key structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubKey {
    /// Key type (e.g., "ed25519")
    #[serde(rename = "type")]
    pub key_type: String,

    /// Key value (base64 encoded)
    pub value: String,
}

/// Consensus parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusParams {
    /// Block parameters
    pub block: Option<BlockParams>,

    /// Evidence parameters
    pub evidence: Option<EvidenceParams>,

    /// Validator parameters
    pub validator: Option<ValidatorParams>,
}

/// Block parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockParams {
    /// Maximum bytes in a block
    pub max_bytes: String,

    /// Maximum gas in a block
    pub max_gas: String,

    /// Time between blocks (in nanoseconds)
    #[serde(rename = "time_iota_ms")]
    pub time_iota_ms: String,
}

/// Evidence parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceParams {
    /// Maximum age in blocks
    #[serde(rename = "max_age_num_blocks")]
    pub max_age_num_blocks: String,

    /// Maximum age in duration
    #[serde(rename = "max_age_duration")]
    pub max_age_duration: String,
}

/// Validator parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorParams {
    /// Maximum number of validators
    #[serde(rename = "max_validators")]
    pub max_validators: u32,
}

impl CometBFTGenesis {
    /// Parse a CometBFT genesis file from JSON
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, DydxAdapterError> {
        let content = fs::read_to_string(path)
            .map_err(|e| DydxAdapterError::msg(format!("Failed to read genesis file: {}", e)))?;

        Self::from_json(&content)
    }

    /// Parse a CometBFT genesis from JSON string
    pub fn from_json(json: &str) -> Result<Self, DydxAdapterError> {
        serde_json::from_str(json)
            .map_err(|e| DydxAdapterError::msg(format!("Failed to parse genesis JSON: {}", e)))
    }

    /// Convert to AptosBFT validator set info
    pub fn to_validator_set_info(&self) -> Result<ValidatorSetInfo, DydxAdapterError> {
        let validators = self.validators.iter().map(|v| CometBFTValidator {
            address: v.address.clone(),
            voting_power: v.power.max(0) as u64,
            proposer_priority: v.proposer_priority,
        }).collect();

        let validator_set = CometBFTValidatorSet { validators };
        ValidatorSetInfo::from_cometbft_validators(&validator_set)
    }

    /// Get the validator map (address -> (index, voting_power))
    pub fn validator_map(&self) -> Result<HashMap<Vec<u8>, (usize, u64)>, DydxAdapterError> {
        let mut map = HashMap::new();

        for (idx, val) in self.validators.iter().enumerate() {
            // Decode hex address
            let address_bytes = hex::decode(&val.address)
                .map_err(|e| DydxAdapterError::msg(format!("Invalid hex address: {}", e)))?;

            map.insert(address_bytes, (idx, val.power.max(0) as u64));
        }

        Ok(map)
    }

    /// Get total voting power
    pub fn total_voting_power(&self) -> u64 {
        self.validators.iter()
            .map(|v| v.power.max(0) as u64)
            .sum()
    }
}

/// Genesis migration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigratedGenesis {
    /// Original chain ID
    pub chain_id: String,

    /// Validators
    pub validators: ValidatorSetInfo,

    /// Total voting power
    pub total_voting_power: u64,

    /// Initial epoch
    pub initial_epoch: u64,

    /// Initial round
    pub initial_round: u64,

    /// Genesis timestamp (parsed)
    pub genesis_timestamp_ms: u64,

    /// App state (preserved)
    pub app_state: serde_json::Value,
}

impl CometBFTGenesis {
    /// Migrate to AptosBFT genesis format
    pub fn migrate(&self) -> Result<MigratedGenesis, DydxAdapterError> {
        // Convert validator set
        let validators = self.to_validator_set_info()?;
        let total_voting_power = validators.total_voting_power;

        // Parse genesis timestamp
        let genesis_timestamp_ms = self.parse_timestamp()?;

        // Use initial_height from genesis, defaulting to 1
        let _initial_height = if self.initial_height > 0 {
            self.initial_height as u64
        } else {
            1
        };

        Ok(MigratedGenesis {
            chain_id: self.chain_id.clone(),
            validators,
            total_voting_power,
            initial_epoch: 0, // AptosBFT starts at epoch 0
            initial_round: 0, // AptosBFT starts at round 0
            genesis_timestamp_ms,
            app_state: self.app_state.clone(),
        })
    }

    /// Parse the genesis timestamp
    fn parse_timestamp(&self) -> Result<u64, DydxAdapterError> {
        // CometBFT uses RFC3339 format (e.g., "2024-01-01T00:00:00Z")
        // Try to parse it, handling potential format variations
        use chrono::{DateTime, Utc};

        // First try strict RFC3339 parsing
        if let Ok(dt) = DateTime::parse_from_rfc3339(&self.genesis_time) {
            return Ok(dt.timestamp_millis() as u64);
        }

        // Fallback: try parsing with seconds timestamp
        if let Ok(secs) = self.genesis_time.parse::<i64>() {
            return Ok(secs as u64 * 1000);
        }

        // If all else fails, use current time as fallback
        Ok(Utc::now().timestamp_millis() as u64)
    }

    /// Create validator set directly from genesis
    pub fn create_validator_set(&self) -> Result<CometBFTValidatorSet, DydxAdapterError> {
        let validators = self.validators.iter().map(|v| CometBFTValidator {
            address: v.address.clone(),
            voting_power: v.power.max(0) as u64,
            proposer_priority: v.proposer_priority,
        }).collect();

        Ok(CometBFTValidatorSet { validators })
    }
}

/// Write migrated genesis to file
pub fn write_migrated_genesis<P: AsRef<Path>>(
    migrated: &MigratedGenesis,
    output_path: P,
) -> Result<(), DydxAdapterError> {
    let json = serde_json::to_string_pretty(migrated)
        .map_err(|e| DydxAdapterError::msg(format!("Failed to serialize migrated genesis: {}", e)))?;

    fs::write(output_path, json)
        .map_err(|e| DydxAdapterError::msg(format!("Failed to write migrated genesis: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DydxNodeId;

    const SAMPLE_GENESIS: &str = r#"{
        "chain_id": "dydx-mainnet-1",
        "genesis_time": "2024-01-01T00:00:00Z",
        "initial_height": 1,
        "validators": [
            {
                "address": "ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD",
                "pub_key": {
                    "type": "ed25519",
                    "value": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                },
                "power": 100,
                "proposer_priority": 0
            },
            {
                "address": "123412341234123412341234123412341234123412341234123412341234",
                "pub_key": {
                    "type": "ed25519",
                    "value": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                },
                "power": 50,
                "proposer_priority": -1
            }
        ],
        "app_state": {
            "accounts": []
        },
        "consensus_params": {
            "block": {
                "max_bytes": "22020096",
                "max_gas": "81500000",
                "time_iota_ms": "1000"
            },
            "evidence": {
                "max_age_num_blocks": "100000",
                "max_age_duration": "172800000000000"
            },
            "validator": {
                "max_validators": 100
            }
        }
    }"#;

    #[test]
    fn test_parse_genesis_from_json() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();

        assert_eq!(genesis.chain_id, "dydx-mainnet-1");
        assert_eq!(genesis.initial_height, 1);
        assert_eq!(genesis.validators.len(), 2);

        // Check first validator
        let v1 = &genesis.validators[0];
        assert_eq!(v1.address, "ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD");
        assert_eq!(v1.power, 100);
        assert_eq!(v1.proposer_priority, Some(0));

        // Check second validator
        let v2 = &genesis.validators[1];
        assert_eq!(v2.power, 50);
        assert_eq!(v2.proposer_priority, Some(-1));
    }

    #[test]
    fn test_genesis_to_validator_set_info() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let info = genesis.to_validator_set_info().unwrap();

        assert_eq!(info.validators.len(), 2);
        assert_eq!(info.total_voting_power, 150); // 100 + 50
        // Validators are sorted by address alphabetically
        assert_eq!(info.voting_powers, vec![50, 100]);
    }

    #[test]
    fn test_genesis_total_voting_power() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        assert_eq!(genesis.total_voting_power(), 150);
    }

    #[test]
    fn test_genesis_migrate() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let migrated = genesis.migrate().unwrap();

        assert_eq!(migrated.chain_id, "dydx-mainnet-1");
        assert_eq!(migrated.total_voting_power, 150);
        assert_eq!(migrated.initial_epoch, 0);
        assert_eq!(migrated.initial_round, 0);
        assert!(migrated.genesis_timestamp_ms > 0);
    }

    #[test]
    fn test_genesis_create_validator_set() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let validator_set = genesis.create_validator_set().unwrap();

        assert_eq!(validator_set.validators.len(), 2);

        // create_validator_set() preserves the original order from genesis file
        let v1 = &validator_set.validators[0];
        assert_eq!(v1.address, "ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD");
        assert_eq!(v1.voting_power, 100);
        assert_eq!(v1.proposer_priority, Some(0));
    }

    #[test]
    fn test_genesis_with_negative_power() {
        let json = r#"{
            "chain_id": "test-chain",
            "genesis_time": "2024-01-01T00:00:00Z",
            "validators": [
                {
                    "address": "ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD",
                    "power": -50,
                    "proposer_priority": 0
                }
            ]
        }"#;

        let genesis = CometBFTGenesis::from_json(json).unwrap();
        let validator_set = genesis.create_validator_set().unwrap();

        // Negative power should be converted to 0
        assert_eq!(validator_set.validators[0].voting_power, 0);
    }

    #[test]
    fn test_parse_genesis_missing_optional_fields() {
        let json = r#"{
            "chain_id": "test-chain",
            "genesis_time": "2024-01-01T00:00:00Z",
            "validators": []
        }"#;

        let genesis = CometBFTGenesis::from_json(json).unwrap();
        assert_eq!(genesis.chain_id, "test-chain");
        assert_eq!(genesis.initial_height, 1); // default
        assert!(genesis.validators.is_empty());
    }

    #[test]
    fn test_genesis_serialization_roundtrip() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let json = serde_json::to_string(&genesis).unwrap();
        let parsed = CometBFTGenesis::from_json(&json).unwrap();

        assert_eq!(parsed.chain_id, genesis.chain_id);
        assert_eq!(parsed.validators.len(), genesis.validators.len());
    }

    #[test]
    fn test_validator_set_info_from_cometbft() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let validator_set = genesis.create_validator_set().unwrap();
        let info = ValidatorSetInfo::from_cometbft_validators(&validator_set).unwrap();

        assert_eq!(info.validators.len(), 2);
        assert_eq!(info.total_voting_power, 150);
        // Validators are sorted by address alphabetically
        assert_eq!(info.voting_powers, vec![50, 100]);
    }

    #[test]
    fn test_validator_set_info_to_hash_map() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let info = genesis.to_validator_set_info().unwrap();
        let map = info.to_hash_map();

        assert_eq!(map.len(), 2);

        // Check that voting powers match
        let mut total_power = 0u64;
        for power in map.values() {
            total_power += power;
        }
        assert_eq!(total_power, 150);
    }

    #[test]
    fn test_validator_set_info_validation() {
        // Test with mismatched lengths
        let validators = vec![
            DydxNodeId([1u8; 32]),
            DydxNodeId([2u8; 32]),
        ];
        let voting_powers = vec![100u64]; // Only 1 power for 2 validators

        let result = ValidatorSetInfo::new(
            validators,
            voting_powers,
            200, // Wrong total
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_genesis_with_consensus_params() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();

        assert!(genesis.consensus_params.is_some());
        let params = genesis.consensus_params.as_ref().unwrap();

        assert!(params.block.is_some());
        assert!(params.evidence.is_some());
        assert!(params.validator.is_some());
    }

    #[test]
    fn test_migrated_genesis_serialization() {
        let genesis = CometBFTGenesis::from_json(SAMPLE_GENESIS).unwrap();
        let migrated = genesis.migrate().unwrap();

        let json = serde_json::to_string(&migrated).unwrap();
        let parsed: MigratedGenesis = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.chain_id, migrated.chain_id);
        assert_eq!(parsed.total_voting_power, migrated.total_voting_power);
    }
}
