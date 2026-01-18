// Peer Configuration Module
//
// This module handles loading peer configuration from the genesis file,
// including validator addresses and their P2P network endpoints.

use crate::error::DydxAdapterError;
use crate::genesis::CometBFTGenesis;
use crate::types::DydxNodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::Result;

/// Peer configuration for a validator.
///
/// Contains the information needed to connect to a validator peer
/// in the P2P network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Validator's node ID (32-byte address)
    pub validator_id: DydxNodeId,

    /// P2P network address (e.g., "192.168.1.1:9001" or "node.example.com:9001")
    pub address: String,

    /// Voting power (for consensus)
    pub voting_power: u64,
}

impl PeerConfig {
    /// Create a new peer configuration.
    pub fn new(validator_id: DydxNodeId, address: String, voting_power: u64) -> Self {
        Self {
            validator_id,
            address,
            voting_power,
        }
    }

    /// Get the validator ID as a hex string.
    pub fn validator_id_hex(&self) -> String {
        hex::encode(self.validator_id.0)
    }
}

/// Extended genesis structure with network addresses.
///
/// This extends the standard CometBFT genesis to include P2P network
/// addresses for each validator. In production, these would be in a
/// separate peers.json file or added as an extension field to genesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkGenesis {
    /// The standard CometBFT genesis
    #[serde(flatten)]
    pub genesis: CometBFTGenesis,

    /// P2P network addresses for validators
    ///
    /// Maps validator address (hex) to network address (IP:port)
    #[serde(default)]
    pub p2p_addresses: HashMap<String, String>,
}

impl NetworkGenesis {
    /// Load from a JSON file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, DydxAdapterError> {
        let content = fs::read_to_string(path)
            .map_err(|e| DydxAdapterError::msg(format!("Failed to read network genesis: {}", e)))?;

        Self::from_json(&content)
    }

    /// Parse from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, DydxAdapterError> {
        serde_json::from_str(json)
            .map_err(|e| DydxAdapterError::msg(format!("Failed to parse network genesis JSON: {}", e)))
    }

    /// Load from a standard CometBFT genesis file.
    ///
    /// If the genesis file doesn't contain p2p_addresses, generates
    /// default addresses based on validator index (for local testing).
    pub fn from_cometbft_genesis(genesis: CometBFTGenesis) -> Self {
        let mut p2p_addresses = HashMap::new();

        // If genesis already has p2p_addresses, use them
        if let Ok(ext) = serde_json::from_value::<HashMap<String, String>>(genesis.app_state.clone()) {
            p2p_addresses = ext;
        }

        // Otherwise, generate default addresses for local testing
        if p2p_addresses.is_empty() {
            for (idx, validator) in genesis.validators.iter().enumerate() {
                // Default to localhost:9001+ for local testing
                let default_port = 9001 + idx as u16;
                p2p_addresses.insert(
                    validator.address.clone(),
                    format!("127.0.0.1:{}", default_port),
                );
            }
        }

        Self {
            genesis,
            p2p_addresses,
        }
    }
}

/// Load peer configurations from a network genesis structure.
///
/// # Arguments
///
/// * `network_genesis` - The network genesis with P2P addresses
///
/// # Returns
///
/// A vector of peer configurations, one for each validator
pub fn load_peers_from_genesis(network_genesis: &NetworkGenesis) -> Vec<PeerConfig> {
    network_genesis.genesis.validators
        .iter()
        .filter_map(|validator| {
            // Get the P2P address for this validator
            let address = network_genesis.p2p_addresses.get(&validator.address)?;

            // Decode validator address to DydxNodeId
            let validator_id = DydxNodeId::from_slice_opt(&validator.address)?;

            Some(PeerConfig::new(
                validator_id,
                address.clone(),
                validator.power.max(0) as u64,
            ))
        })
        .collect()
}

/// Load peer configurations from a genesis file.
///
/// This is a convenience function that combines loading the genesis file
/// and extracting peer configurations.
///
/// # Arguments
///
/// * `genesis_path` - Path to the genesis JSON file
///
/// # Returns
///
/// A vector of peer configurations
pub fn load_peers_from_genesis_file(genesis_path: &Path) -> Result<Vec<PeerConfig>, DydxAdapterError> {
    let genesis = CometBFTGenesis::from_file(genesis_path)?;
    let network_genesis = NetworkGenesis::from_cometbft_genesis(genesis);
    Ok(load_peers_from_genesis(&network_genesis))
}

/// Helper trait for converting hex string to DydxNodeId
trait DydxNodeIdExt {
    fn from_slice_opt(hex_str: &str) -> Option<DydxNodeId>;
}

impl DydxNodeIdExt for DydxNodeId {
    fn from_slice_opt(hex_str: &str) -> Option<DydxNodeId> {
        let bytes = hex::decode(hex_str).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Some(DydxNodeId(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENESIS_WITH_ADDRESSES: &str = r#"{
        "chain_id": "dydx-testnet-1",
        "genesis_time": "2024-01-01T00:00:00Z",
        "initial_height": 1,
        "validators": [
            {
                "address": "abcd000000000000000000000000000000000000000000000000000000000000",
                "pub_key": {
                    "type": "ed25519",
                    "value": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "power": 100,
                "proposer_priority": 0
            },
            {
                "address": "1234000000000000000000000000000000000000000000000000000000000000",
                "pub_key": {
                    "type": "ed25519",
                    "value": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB="
                },
                "power": 50,
                "proposer_priority": -1
            }
        ],
        "app_state": {},
        "p2p_addresses": {
            "abcd000000000000000000000000000000000000000000000000000000000000": "192.168.1.10:9001",
            "1234000000000000000000000000000000000000000000000000000000000000": "192.168.1.11:9001"
        }
    }"#;

    #[test]
    fn test_peer_config_new() {
        let id = DydxNodeId([1u8; 32]);
        let config = PeerConfig::new(id, "127.0.0.1:9001".to_string(), 100);

        assert_eq!(config.validator_id, id);
        assert_eq!(config.address, "127.0.0.1:9001");
        assert_eq!(config.voting_power, 100);
    }

    #[test]
    fn test_peer_config_validator_id_hex() {
        let id = DydxNodeId([0xAB; 32]);
        let config = PeerConfig::new(id, "127.0.0.1:9001".to_string(), 100);

        let hex_str = config.validator_id_hex();
        assert_eq!(hex_str, str::repeat("ab", 32));
    }

    #[test]
    fn test_network_genesis_from_json() {
        let network_genesis = NetworkGenesis::from_json(GENESIS_WITH_ADDRESSES).unwrap();

        assert_eq!(network_genesis.genesis.chain_id, "dydx-testnet-1");
        assert_eq!(network_genesis.p2p_addresses.len(), 2);
    }

    #[test]
    fn test_load_peers_from_genesis() {
        let network_genesis = NetworkGenesis::from_json(GENESIS_WITH_ADDRESSES).unwrap();
        let peers = load_peers_from_genesis(&network_genesis);

        assert_eq!(peers.len(), 2);

        // Check first peer
        let peer1 = &peers[0];
        assert_eq!(peer1.address, "192.168.1.10:9001");
        assert_eq!(peer1.voting_power, 100);

        // Check second peer
        let peer2 = &peers[1];
        assert_eq!(peer2.address, "192.168.1.11:9001");
        assert_eq!(peer2.voting_power, 50);
    }

    #[test]
    fn test_network_genesis_from_cometbft_genesis() {
        let cometbft_genesis = CometBFTGenesis::from_json(GENESIS_WITH_ADDRESSES).unwrap();
        let network_genesis = NetworkGenesis::from_cometbft_genesis(cometbft_genesis);

        // Should preserve the original genesis data
        assert_eq!(network_genesis.genesis.validators.len(), 2);

        // Should have default addresses generated
        assert_eq!(network_genesis.p2p_addresses.len(), 2);
    }

    #[test]
    fn test_load_peers_from_genesis_default_addresses() {
        let genesis_json = r#"{
            "chain_id": "test-chain",
            "genesis_time": "2024-01-01T00:00:00Z",
            "validators": [
                {
                    "address": "abcd000000000000000000000000000000000000000000000000000000000000",
                    "power": 100
                },
                {
                    "address": "1234000000000000000000000000000000000000000000000000000000000000",
                    "power": 50
                }
            ]
        }"#;

        let cometbft_genesis = CometBFTGenesis::from_json(genesis_json).unwrap();
        let network_genesis = NetworkGenesis::from_cometbft_genesis(cometbft_genesis);
        let peers = load_peers_from_genesis(&network_genesis);

        assert_eq!(peers.len(), 2);

        // Should have generated default addresses
        assert_eq!(peers[0].address, "127.0.0.1:9001");
        assert_eq!(peers[1].address, "127.0.0.1:9002");
    }

    #[test]
    fn test_dydx_node_id_from_slice_opt() {
        let hex_str = "abcd000000000000000000000000000000000000000000000000000000000000";
        let id = DydxNodeId::from_slice_opt(hex_str).unwrap();

        assert_eq!(id.0[0], 0xAB);
        assert_eq!(id.0[1], 0xCD);
        assert!(id.0[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn test_dydx_node_id_from_slice_opt_invalid() {
        // Invalid hex
        assert!(DydxNodeId::from_slice_opt("xyz").is_none());

        // Wrong length
        assert!(DydxNodeId::from_slice_opt("abcd").is_none());
    }

    #[test]
    fn test_peer_config_serialization() {
        let id = DydxNodeId([1u8; 32]);
        let config = PeerConfig::new(id, "127.0.0.1:9001".to_string(), 100);

        let json = serde_json::to_string(&config).unwrap();
        let decoded: PeerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.validator_id, id);
        assert_eq!(decoded.address, "127.0.0.1:9001");
        assert_eq!(decoded.voting_power, 100);
    }
}
