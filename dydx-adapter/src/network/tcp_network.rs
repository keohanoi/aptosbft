// TCP Network Implementation for AptosBFT Consensus
//
// This module provides a concrete implementation of the ConsensusNetwork trait
// using TCP sockets for P2P communication between validators.
//
// Architecture:
// - TcpListener: Accepts incoming connections from peers
// - TcpStream: Manages persistent connections to each peer
// - Message serialization: bincode with 4-byte length prefix
// - Async I/O: tokio for non-blocking network operations

use crate::types::DydxNodeId;
use consensus_traits::block::{Block, Vote};
use consensus_traits::network::{ConsensusMessage, ConsensusNetwork};
use std::collections::HashMap;
use std::sync::Arc;
use std::marker::PhantomData;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock, Mutex};
use anyhow::{Result, anyhow};

/// Network configuration for the TCP P2P layer.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// This validator's node ID
    pub self_id: DydxNodeId,

    /// Address to bind to for listening (e.g., "0.0.0.0:9001")
    pub listen_address: String,

    /// Map of peer node IDs to their addresses
    pub peers: HashMap<DydxNodeId, String>,

    /// Buffer size for message serialization
    pub buffer_size: usize,
}

impl NetworkConfig {
    /// Create a new network configuration.
    pub fn new(self_id: DydxNodeId, listen_address: String) -> Self {
        Self {
            self_id,
            listen_address,
            peers: HashMap::new(),
            buffer_size: 1024 * 1024, // 1MB default
        }
    }

    /// Add a peer to the configuration.
    pub fn add_peer(&mut self, id: DydxNodeId, address: String) {
        self.peers.insert(id, address);
    }

    /// Get the list of peer addresses.
    pub fn peer_addresses(&self) -> Vec<String> {
        self.peers.values().cloned().collect()
    }
}

/// Network error type.
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Connection error to peer {peer:?}: {error}")]
    Connection { peer: DydxNodeId, error: String },

    #[error("Connection timeout to peer {peer:?} after {timeout}s")]
    ConnectionTimeout { peer: DydxNodeId, timeout: u64 },

    #[error("Peer disconnected: {0:?}")]
    PeerDisconnected(DydxNodeId),

    #[error("Peer not found: {0:?}")]
    PeerNotFound(DydxNodeId),

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Network not running")]
    NotRunning,

    #[error("Max retry attempts exceeded for peer {peer:?}")]
    MaxRetriesExceeded { peer: DydxNodeId },

    #[error("Peer {peer:?} is unhealthy: {reason}")]
    UnhealthyPeer { peer: DydxNodeId, reason: String },
}

/// Serializable network message wrapper.
///
/// This enum wraps the different types of consensus messages in a
/// serializable format for transmission over the network.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    /// Block proposal
    Proposal(Vec<u8>),

    /// Vote
    Vote(Vec<u8>),

    /// New round timeout
    NewRound { round: u64 },
}

/// Peer health status for tracking connection quality.
#[derive(Debug, Clone)]
struct PeerHealth {
    /// Number of consecutive connection failures
    consecutive_failures: u32,

    /// Last successful connection timestamp
    last_success: Option<std::time::Instant>,

    /// Whether this peer is marked as unhealthy (circuit breaker open)
    is_unhealthy: bool,

    /// When this peer can be retried (if unhealthy)
    retry_after: Option<std::time::Instant>,
}

impl Default for PeerHealth {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            last_success: None,
            is_unhealthy: false,
            retry_after: None,
        }
    }
}

/// Retry configuration for network operations.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Base delay between retries (in milliseconds)
    pub base_delay_ms: u64,

    /// Maximum delay between retries (in milliseconds)
    pub max_delay_ms: u64,

    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,

    /// Number of failures before marking peer as unhealthy
    pub unhealthy_threshold: u32,

    /// How long to wait before retrying an unhealthy peer (seconds)
    pub unhealthy_retry_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            unhealthy_threshold: 3,
            unhealthy_retry_delay_secs: 30,
        }
    }
}

/// TCP-based P2P network implementation for AptosBFT consensus.
///
/// This implements the ConsensusNetwork trait using TCP sockets for
/// validator-to-validator communication.
///
/// # Type Parameters
///
/// * `B` - Block type (must be serializable)
/// * `V` - Vote type (must be serializable)
#[allow(dead_code)]
pub struct TcpNetwork<B, V> {
    /// Our validator ID
    self_id: DydxNodeId,

    /// Peer configurations
    peers: Arc<RwLock<HashMap<DydxNodeId, String>>>,

    /// Active connections to peers (wrapped in Mutex for mutable access)
    connections: Arc<RwLock<HashMap<DydxNodeId, Arc<Mutex<TcpStream>>>>>,

    /// Peer health tracking
    peer_health: Arc<RwLock<HashMap<DydxNodeId, PeerHealth>>>,

    /// Retry configuration
    retry_config: RetryConfig,

    /// Incoming message channel (stores raw bytes to avoid trait bound issues)
    incoming_sender: mpsc::UnboundedSender<Vec<u8>>,
    incoming_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>>,

    /// Network configuration
    config: NetworkConfig,

    /// Whether the network is running
    running: Arc<RwLock<bool>>,

    /// Phantom data to pin type parameters
    _phantom: PhantomData<(B, V)>,
}

impl<B, V> TcpNetwork<B, V>
where
    B: Block + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    V: Vote<Block = B> + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    /// Create a new TCP network.
    ///
    /// # Arguments
    ///
    /// * `config` - Network configuration
    ///
    /// # Returns
    ///
    /// A new TcpNetwork instance
    pub async fn new(config: NetworkConfig) -> Result<Self> {
        let (incoming_sender, incoming_receiver) = mpsc::unbounded_channel();

        let self_id = config.self_id;
        let peers = Arc::new(RwLock::new(config.peers.clone()));
        let connections = Arc::new(RwLock::new(HashMap::new()));
        let peer_health = Arc::new(RwLock::new(HashMap::new()));
        let running = Arc::new(RwLock::new(false));
        let retry_config = RetryConfig::default();

        Ok(Self {
            self_id,
            peers,
            connections,
            peer_health,
            retry_config,
            incoming_sender,
            incoming_receiver: Arc::new(Mutex::new(Some(incoming_receiver))),
            config,
            running,
            _phantom: PhantomData,
        })
    }

    /// Start the network listener.
    ///
    /// This begins accepting incoming connections from peers and
    /// processing messages.
    pub async fn start(&mut self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(()); // Already running
        }
        *running = true;
        drop(running);

        log::info!("Starting TCP network on {}", self.config.listen_address);

        // Start the listener task
        let listener = TcpListener::bind(&self.config.listen_address).await
            .map_err(|e| anyhow!("Failed to bind to {}: {}", self.config.listen_address, e))?;

        log::info!("Listening for P2P connections on {}", self.config.listen_address);

        // Spawn the listener task
        let incoming_sender = self.incoming_sender.clone();
        let connections = self.connections.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            Self::listener_task(listener, incoming_sender, connections, running).await;
        });

        // Connect to all peers
        self.connect_to_peers().await?;

        Ok(())
    }

    /// Stop the network.
    pub async fn stop(&mut self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        // Close all connections
        let mut connections = self.connections.write().await;
        connections.clear();

        log::info!("TCP network stopped");
        Ok(())
    }

    /// Connect to all configured peers.
    async fn connect_to_peers(&self) -> Result<()> {
        let peers = self.peers.read().await;
        let mut connections = self.connections.write().await;

        for (peer_id, address) in peers.iter() {
            // Skip ourselves
            if peer_id == &self.self_id {
                continue;
            }

            log::info!("Connecting to peer {:?} at {}", peer_id, address);

            match TcpStream::connect(address).await {
                Ok(stream) => {
                    log::info!("Connected to peer at {}", address);
                    connections.insert(*peer_id, Arc::new(Mutex::new(stream)));
                }
                Err(e) => {
                    log::warn!("Failed to connect to peer at {}: {}", address, e);
                    // Continue trying other peers
                }
            }
        }

        Ok(())
    }

    /// Background task for accepting incoming connections.
    async fn listener_task(
        listener: TcpListener,
        incoming_sender: mpsc::UnboundedSender<Vec<u8>>,
        _connections: Arc<RwLock<HashMap<DydxNodeId, Arc<Mutex<TcpStream>>>>>,
        running: Arc<RwLock<bool>>,
    ) {
        log::info!("Network listener task started");

        while *running.read().await {
            // Accept with timeout
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(1),
                listener.accept()
            ).await {
                Ok(Ok((stream, addr))) => {
                    log::info!("Accepted connection from {}", addr);

                    // Spawn a task to handle this connection
                    let sender = incoming_sender.clone();
                    let stream_mutex = Arc::new(Mutex::new(stream));
                    let running_clone = running.clone();

                    tokio::spawn(async move {
                        Self::handle_connection(stream_mutex, addr.to_string(), sender, running_clone).await;
                    });
                }
                Ok(Err(e)) => {
                    log::error!("Error accepting connection: {}", e);
                }
                Err(_) => {
                    // Timeout - check if we should stop
                }
            }
        }

        log::info!("Network listener task stopped");
    }

    /// Handle a single peer connection.
    async fn handle_connection(
        stream: Arc<Mutex<TcpStream>>,
        peer_addr: String,
        incoming_sender: mpsc::UnboundedSender<Vec<u8>>,
        running: Arc<RwLock<bool>>,
    ) {
        log::debug!("Handling connection from {}", peer_addr);

        while *running.read().await {
            // Lock the stream for reading
            let mut stream = stream.lock().await;

            // Read message length (4 bytes)
            let mut len_bytes = [0u8; 4];
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                stream.read_exact(&mut len_bytes)
            ).await {
                Ok(Ok(_n)) => {}
                Ok(Err(e)) => {
                    log::error!("Error reading message length from {}: {}", peer_addr, e);
                    break;
                }
                Err(_) => {
                    // Timeout - check if we should stop
                    if !*running.read().await {
                        break;
                    }
                    continue;
                }
            }

            let msg_len = u32::from_be_bytes(len_bytes) as usize;

            // Safety check on message size
            if msg_len > 10 * 1024 * 1024 { // 10MB max
                log::error!("Message too large from {}: {} bytes", peer_addr, msg_len);
                break;
            }

            // Read message body
            let mut msg_bytes = vec![0u8; msg_len];
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                stream.read_exact(&mut msg_bytes)
            ).await {
                Ok(Ok(_n)) => {}
                Ok(Err(e)) => {
                    log::error!("Error reading message body from {}: {}", peer_addr, e);
                    break;
                }
                Err(_) => {
                    // Timeout
                    continue;
                }
            }

            log::trace!("Received {} bytes from {}", msg_bytes.len(), peer_addr);

            // Release the lock before sending
            drop(stream);

            if let Err(e) = incoming_sender.send(msg_bytes) {
                log::error!("Failed to send message to incoming channel: {}", e);
                break;
            }
        }

        log::debug!("Connection handler for {} stopped", peer_addr);
    }

    /// Send a message to a specific peer.
    async fn send_to_peer(&self, peer_id: DydxNodeId, msg_bytes: &[u8]) -> Result<(), NetworkError> {
        let connections = self.connections.read().await;

        let stream_mutex = connections.get(&peer_id)
            .ok_or_else(|| NetworkError::PeerNotFound(peer_id))?;

        // Length prefix (4 bytes big-endian)
        let len_bytes = (msg_bytes.len() as u32).to_be_bytes();

        // Lock the stream for writing
        let mut stream = stream_mutex.lock().await;

        stream.write_all(&len_bytes).await
            .map_err(|e| NetworkError::Connection { peer: peer_id, error: e.to_string() })?;

        stream.write_all(msg_bytes).await
            .map_err(|e| NetworkError::Connection { peer: peer_id, error: e.to_string() })?;

        stream.flush().await
            .map_err(|e| NetworkError::Connection { peer: peer_id, error: e.to_string() })?;

        log::trace!("Sent {} bytes to peer {:?}", msg_bytes.len(), peer_id);

        Ok(())
    }

    /// Convert ConsensusMessage to serializable NetworkMessage
    fn to_network_message(msg: &ConsensusMessage<B, V>) -> Result<NetworkMessage, NetworkError> {
        match msg {
            ConsensusMessage::Proposal(block) => {
                let bytes = bincode::serialize(block)
                    .map_err(|e| NetworkError::Serialization(e.to_string()))?;
                Ok(NetworkMessage::Proposal(bytes))
            }
            ConsensusMessage::Vote(vote) => {
                let bytes = bincode::serialize(vote)
                    .map_err(|e| NetworkError::Serialization(e.to_string()))?;
                Ok(NetworkMessage::Vote(bytes))
            }
            ConsensusMessage::NewRound { round } => {
                Ok(NetworkMessage::NewRound { round: *round })
            }
            _ => {
                // BlockRequest/BlockResponse not yet implemented
                Ok(NetworkMessage::NewRound { round: 0 })
            }
        }
    }

    /// Convert NetworkMessage back to ConsensusMessage
    fn from_network_message(msg: NetworkMessage) -> Result<ConsensusMessage<B, V>, NetworkError> {
        match msg {
            NetworkMessage::Proposal(bytes) => {
                let block = bincode::deserialize(&bytes)
                    .map_err(|e| NetworkError::Serialization(e.to_string()))?;
                Ok(ConsensusMessage::Proposal(Box::new(block)))
            }
            NetworkMessage::Vote(bytes) => {
                let vote = bincode::deserialize(&bytes)
                    .map_err(|e| NetworkError::Serialization(e.to_string()))?;
                Ok(ConsensusMessage::Vote(Box::new(vote)))
            }
            NetworkMessage::NewRound { round } => {
                Ok(ConsensusMessage::NewRound { round })
            }
        }
    }
}

#[async_trait]
impl<B, V> ConsensusNetwork<B, V> for TcpNetwork<B, V>
where
    B: Block + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    V: Vote<Block = B> + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    type NodeId = DydxNodeId;
    type Error = NetworkError;

    async fn send_proposal(
        &self,
        to: Self::NodeId,
        proposal: B,
    ) -> Result<(), Self::Error> {
        let message = ConsensusMessage::Proposal(Box::new(proposal));
        let network_msg = Self::to_network_message(&message)?;
        let msg_bytes = bincode::serialize(&network_msg)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;
        self.send_to_peer(to, &msg_bytes).await
    }

    async fn broadcast_proposal(&self, proposal: B) -> Result<(), Self::Error> {
        let message = ConsensusMessage::Proposal(Box::new(proposal));
        let network_msg = Self::to_network_message(&message)?;
        let msg_bytes = bincode::serialize(&network_msg)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;

        let peers = self.peers.read().await;
        for peer_id in peers.keys() {
            if peer_id == &self.self_id {
                continue;
            }

            // Send to each peer (ignore errors for broadcast)
            let _ = self.send_to_peer(*peer_id, &msg_bytes).await;
        }

        Ok(())
    }

    async fn send_vote(
        &self,
        to: Self::NodeId,
        vote: V,
    ) -> Result<(), Self::Error> {
        let message = ConsensusMessage::Vote(Box::new(vote));
        let network_msg = Self::to_network_message(&message)?;
        let msg_bytes = bincode::serialize(&network_msg)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;
        self.send_to_peer(to, &msg_bytes).await
    }

    async fn broadcast_vote(&self, vote: V) -> Result<(), Self::Error> {
        let message = ConsensusMessage::Vote(Box::new(vote));
        let network_msg = Self::to_network_message(&message)?;
        let msg_bytes = bincode::serialize(&network_msg)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;

        let peers = self.peers.read().await;
        for peer_id in peers.keys() {
            if peer_id == &self.self_id {
                continue;
            }

            // Send to each peer (ignore errors for broadcast)
            let _ = self.send_to_peer(*peer_id, &msg_bytes).await;
        }

        Ok(())
    }

    async fn receive_message(&self) -> Result<ConsensusMessage<B, V>, Self::Error> {
        // Lock the mutex to get access to the receiver
        let mut receiver_guard = self.incoming_receiver.lock().await;
        let receiver = receiver_guard.as_mut()
            .ok_or(NetworkError::ChannelClosed)?;

        let msg_bytes = receiver.recv().await
            .ok_or(NetworkError::ChannelClosed)?;

        let network_msg: NetworkMessage = bincode::deserialize(&msg_bytes)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;

        Self::from_network_message(network_msg)
    }

    fn validator_peers(&self) -> Vec<Self::NodeId> {
        // This is a synchronous method, so we can't await the lock
        // Return empty for now - in practice, this would cache the peer list
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_new() {
        let id = DydxNodeId([1u8; 32]);
        let config = NetworkConfig::new(id, "0.0.0.0:9001".to_string());

        assert_eq!(config.self_id, id);
        assert_eq!(config.listen_address, "0.0.0.0:9001");
        assert!(config.peers.is_empty());
    }

    #[test]
    fn test_network_config_add_peer() {
        let id = DydxNodeId([1u8; 32]);
        let mut config = NetworkConfig::new(id, "0.0.0.0:9001".to_string());

        let peer_id = DydxNodeId([2u8; 32]);
        config.add_peer(peer_id, "127.0.0.1:9002".to_string());

        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.peers.get(&peer_id), Some(&"127.0.0.1:9002".to_string()));
    }

    #[test]
    fn test_network_config_peer_addresses() {
        let id = DydxNodeId([1u8; 32]);
        let mut config = NetworkConfig::new(id, "0.0.0.0:9001".to_string());

        config.add_peer(DydxNodeId([2u8; 32]), "127.0.0.1:9002".to_string());
        config.add_peer(DydxNodeId([3u8; 32]), "127.0.0.1:9003".to_string());

        let addresses = config.peer_addresses();
        assert_eq!(addresses.len(), 2);
        assert!(addresses.contains(&"127.0.0.1:9002".to_string()));
        assert!(addresses.contains(&"127.0.0.1:9003".to_string()));
    }

    #[test]
    fn test_network_error_display() {
        let test_peer = DydxNodeId([1u8; 32]);
        let err = NetworkError::Connection { peer: test_peer, error: "test error".to_string() };
        assert!(format!("{}", err).contains("Connection error"));

        let err = NetworkError::PeerNotFound(DydxNodeId([1u8; 32]));
        assert!(format!("{}", err).contains("Peer not found"));
    }

    #[test]
    fn test_network_message_serialization() {
        let msg = NetworkMessage::NewRound { round: 5 };
        let bytes = bincode::serialize(&msg).unwrap();
        let decoded: NetworkMessage = bincode::deserialize(&bytes).unwrap();

        match decoded {
            NetworkMessage::NewRound { round } => {
                assert_eq!(round, 5);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
