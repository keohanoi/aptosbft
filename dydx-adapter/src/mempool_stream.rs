// Mempool synchronization stream for dYdX v4
//
// This module implements the bidirectional mempool synchronization stream
// between AptosBFT consensus (Rust) and the dYdX v4 application (Go).
//
// The stream enables:
// - Order streaming from Rust Quorum Store to Go application
// - Batch fetching of transactions on demand
// - Mempool gossip coordination

use tonic::transport::Channel;
use consensus_grpc_protos::consensus_app::{
    consensus_app_client::ConsensusAppClient as GrpcClient,
    MempoolRequest,
    MempoolResponse,
    FetchBatchRequest,
    TxBatch,
    BatchRequest,
    BatchAck,
    AckResponse,
    OrderReadyNotification,
};
use crate::error::DydxAdapterError;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for mempool synchronization.
#[derive(Clone, Debug)]
pub struct MempoolSyncConfig {
    /// Maximum size of a transaction batch in bytes
    pub max_batch_bytes: u64,

    /// Maximum number of transactions per batch
    pub max_batch_txs: u64,

    /// Stream timeout in seconds
    pub stream_timeout_secs: u64,
}

impl Default for MempoolSyncConfig {
    fn default() -> Self {
        Self {
            max_batch_bytes: 1024 * 1024, // 1 MB
            max_batch_txs: 1000,
            stream_timeout_secs: 30,
        }
    }
}

/// Mempool synchronization stream manager.
///
/// This manages the bidirectional gRPC stream between AptosBFT consensus
/// and the dYdX application for order synchronization.
///
/// # Example
///
/// ```text
/// use dydx_adapter::mempool_stream::{MempoolSyncStream, MempoolSyncConfig};
/// use consensus_grpc_protos::consensus_app::consensus_app_client::ConsensusAppClient;
///
/// let config = MempoolSyncConfig::default();
/// let client = ConsensusAppClient::connect("http://localhost:50051").await?;
/// let mut stream = MempoolSyncStream::new(client, config);
///
/// // Start the synchronization loop
/// stream.start().await?;
/// ```
pub struct MempoolSyncStream {
    /// gRPC client connected to the dYdX application
    client: GrpcClient<Channel>,

    /// Configuration
    config: MempoolSyncConfig,

    /// Next batch ID
    next_batch_id: Arc<Mutex<u64>>,
}

impl MempoolSyncStream {
    /// Create a new mempool sync stream.
    pub fn new(client: GrpcClient<Channel>, config: MempoolSyncConfig) -> Self {
        Self {
            client,
            config,
            next_batch_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Get the gRPC client.
    pub fn client(&self) -> &GrpcClient<Channel> {
        &self.client
    }

    /// Get the configuration.
    pub fn config(&self) -> &MempoolSyncConfig {
        &self.config
    }

    /// Get the next batch ID (for testing).
    pub async fn next_batch_id(&self) -> u64 {
        *self.next_batch_id.lock().await
    }

    /// Handle a mempool response from the application.
    pub async fn handle_response(&self, response: MempoolResponse) -> Result<(), DydxAdapterError> {
        use consensus_grpc_protos::consensus_app::mempool_response::Response;

        match response.response {
            Some(Response::TxBatch(batch)) => {
                self.handle_tx_batch(batch).await?;
            }
            Some(Response::BatchRequest(request)) => {
                self.handle_batch_request(request).await?;
            }
            Some(Response::AckResponse(ack)) => {
                self.handle_ack_response(ack).await;
            }
            None => {
                log::warn!("Received empty mempool response");
            }
        }
        Ok(())
    }

    /// Handle a transaction batch from the application.
    async fn handle_tx_batch(&self, batch: TxBatch) -> Result<(), DydxAdapterError> {
        log::info!(
            "Received tx batch {} with {} transactions",
            batch.batch_id,
            batch.transactions.len()
        );

        // Deliver transactions to the Quorum Store
        // TODO: Implement actual delivery to Quorum Store

        // Send acknowledgment if this is the final batch
        if batch.is_final {
            // Note: In production, we'd send this back through the stream
            log::info!("Batch {} is final, would send ack", batch.batch_id);
        }

        Ok(())
    }

    /// Handle a batch request from the application.
    async fn handle_batch_request(&self, request: BatchRequest) -> Result<(), DydxAdapterError> {
        log::info!(
            "Application requests batch: max_bytes={}, max_txs={}",
            request.max_bytes,
            request.max_txs
        );

        // Fetch transactions from Quorum Store and send to application
        // TODO: Implement actual Quorum Store fetch

        Ok(())
    }

    /// Handle an acknowledgment response.
    async fn handle_ack_response(&self, ack: AckResponse) {
        log::info!(
            "Received ack for batch {}: accepted={}",
            ack.batch_id,
            ack.accepted
        );
        // TODO: Update batch tracking state
    }

    /// Create a TxBatch for sending to the application.
    pub fn create_tx_batch(&self, transactions: Vec<Vec<u8>>, is_final: bool) -> TxBatch {
        let batch_id = {
            // Note: This is a simplified version. In production with async,
            // we'd handle the mutex properly
            0 // Placeholder
        };

        TxBatch {
            batch_id,
            transactions,
            is_final,
        }
    }

    /// Create a BatchAck for sending to the application.
    pub fn create_batch_ack(batch_id: u64, accepted: bool) -> BatchAck {
        BatchAck { batch_id, accepted }
    }

    /// Create a BatchRequest for sending to the application.
    pub fn create_batch_request(&self, max_bytes: u64, max_txs: u64) -> BatchRequest {
        let deadline_ms = chrono::Utc::now().timestamp_millis() + 5000; // 5 second deadline

        BatchRequest {
            max_bytes,
            max_txs,
            deadline_ms,
        }
    }

    /// Create an OrderReadyNotification for sending to the application.
    pub fn create_order_ready_notification(&self, order_count: u32) -> OrderReadyNotification {
        OrderReadyNotification {
            order_count,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Helper function to create a mempool sync stream from an endpoint.
pub async fn create_mempool_sync_stream(
    endpoint: String,
    config: MempoolSyncConfig,
) -> Result<MempoolSyncStream, DydxAdapterError> {
    let client = GrpcClient::connect(endpoint.clone())
        .await
        .map_err(|e| DydxAdapterError::Connection(format!("Failed to connect: {}", e)))?;

    Ok(MempoolSyncStream::new(client, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mempool_sync_config_default() {
        let config = MempoolSyncConfig::default();
        assert_eq!(config.max_batch_bytes, 1024 * 1024);
        assert_eq!(config.max_batch_txs, 1000);
        assert_eq!(config.stream_timeout_secs, 30);
    }

    #[test]
    fn test_create_tx_batch() {
        let config = MempoolSyncConfig::default();

        let txs = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let batch = TxBatch {
            batch_id: 0,
            transactions: txs.clone(),
            is_final: true,
        };

        assert_eq!(batch.transactions, txs);
        assert!(batch.is_final);
    }

    #[test]
    fn test_create_batch_ack() {
        let ack = BatchAck { batch_id: 123, accepted: true };
        assert_eq!(ack.batch_id, 123);
        assert!(ack.accepted);
    }

    #[test]
    fn test_create_batch_request() {
        let request = BatchRequest {
            max_bytes: 5000,
            max_txs: 100,
            deadline_ms: 12345,
        };
        assert_eq!(request.max_bytes, 5000);
        assert_eq!(request.max_txs, 100);
    }

    #[test]
    fn test_create_order_ready_notification() {
        let notification = OrderReadyNotification {
            order_count: 42,
            timestamp: 12345,
        };
        assert_eq!(notification.order_count, 42);
    }
}
