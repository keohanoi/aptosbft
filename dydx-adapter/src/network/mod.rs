// P2P Network Module for AptosBFT-dYdX Integration
//
// This module provides a TCP-based P2P network implementation for the
// AptosBFT consensus protocol, enabling validators to communicate
// proposals, votes, and timeout messages.
//
// Architecture:
// - TcpNetwork: Concrete implementation of ConsensusNetwork trait
// - PeerConfig: Configuration for validator peers from genesis
// - Message serialization using bincode

pub mod tcp_network;
pub mod peer_config;

pub use tcp_network::{TcpNetwork, NetworkConfig, NetworkError, NetworkMessage, RetryConfig};
pub use peer_config::{PeerConfig, load_peers_from_genesis, load_peers_from_genesis_file};

use consensus_traits::network::{ConsensusMessage, ConsensusNetwork};
use crate::types::{DydxBlock, DydxVote, DydxNodeId};
use std::path::PathBuf;
use anyhow::Result;

/// Create a TCP network instance from genesis configuration.
///
/// This is a convenience function that loads peer configuration from
/// the genesis file and creates a fully configured TcpNetwork.
///
/// # Arguments
///
/// * `genesis_path` - Path to the genesis JSON file
/// * `self_id` - This validator's node ID
/// * `listen_address` - Address to listen on (e.g., "0.0.0.0:9001")
///
/// # Example
///
/// ```ignore
/// use dydx_adapter::network::create_network;
///
/// let network = create_network(
///     &PathBuf::from("/path/to/genesis.json"),
///     my_validator_id,
///     "0.0.0.0:9001",
/// )?;
/// ```
pub async fn create_network(
    genesis_path: &PathBuf,
    self_id: DydxNodeId,
    listen_address: String,
) -> Result<TcpNetwork<DydxBlock, DydxVote>> {
    let peers = load_peers_from_genesis_file(genesis_path)?;

    let config = NetworkConfig {
        self_id,
        listen_address,
        peers: peers.into_iter().map(|p| (p.validator_id, p.address)).collect(),
        buffer_size: 1024 * 1024, // 1MB default buffer
    };

    TcpNetwork::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // This test ensures all public exports are available
        // It's a compile-time check that the module structure is correct
    }
}
