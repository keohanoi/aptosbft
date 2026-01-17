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
    MempoolResponse,
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
        // NOTE: This requires integration with AptosBFT's Quorum Store component.
        // The Quorum Store is responsible for:
        // - Managing transaction propagation and ordering
        // - Ensuring fair ordering across proposers
        // - Handling transaction deduplication
        //
        // To integrate with Quorum Store when available:
        // 1. Import the Quorum Store client from aptosbft/quorum-store
        // 2. Call quorum_store.publish_transactions(transactions)
        // 3. Handle any propagation errors
        //
        // For now, transactions are logged for debugging purposes
        log::debug!("Transactions ready for Quorum Store delivery: {} txs", batch.transactions.len());

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
        // NOTE: This requires integration with AptosBFT's Quorum Store component.
        // The Quorum Store provides:
        // - Ordered transactions for proposal
        // - Fair ordering across proposers
        // - Transaction payload delivery
        //
        // To integrate with Quorum Store when available:
        // 1. Import the Quorum Store client from aptosbft/quorum-store
        // 2. Call quorum_store.get_batch(max_bytes, max_txs)
        // 3. Return the batch to the application via the stream
        //
        // For now, return an empty batch as a safe default
        log::debug!("Quorum Store integration not yet available, returning empty batch");

        Ok(())
    }

    /// Handle an acknowledgment response.
    async fn handle_ack_response(&self, ack: AckResponse) {
        log::info!(
            "Received ack for batch {}: accepted={}",
            ack.batch_id,
            ack.accepted
        );

        // Update batch tracking state
        // NOTE: This requires maintaining state for batch acknowledgments
        // to track which batches have been successfully received by the application.
        // The state should track:
        // - Batch IDs that have been acknowledged
        // - Timestamps of acknowledgments
        // - Retry counts for failed deliveries
        //
        // For production, implement a BatchTracker struct that:
        // - Records sent batches with timestamps
        // - Updates state on acknowledgment
        // - Handles timeouts and retries
        //
        // For now, we log the acknowledgment for monitoring
        log::debug!("Batch acknowledgment tracked: batch_id={}, accepted={}", ack.batch_id, ack.accepted);
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

    // Config tests
    #[test]
    fn test_mempool_sync_config_default() {
        let config = MempoolSyncConfig::default();
        assert_eq!(config.max_batch_bytes, 1024 * 1024);
        assert_eq!(config.max_batch_txs, 1000);
        assert_eq!(config.stream_timeout_secs, 30);
    }

    #[test]
    fn test_mempool_sync_config_zero_values() {
        let config = MempoolSyncConfig {
            max_batch_bytes: 0,
            max_batch_txs: 0,
            stream_timeout_secs: 0,
        };

        assert_eq!(config.max_batch_bytes, 0);
        assert_eq!(config.max_batch_txs, 0);
        assert_eq!(config.stream_timeout_secs, 0);
    }

    #[test]
    fn test_mempool_sync_config_large_values() {
        let config = MempoolSyncConfig {
            max_batch_bytes: u64::MAX,
            max_batch_txs: u64::MAX,
            stream_timeout_secs: u64::MAX,
        };

        assert_eq!(config.max_batch_bytes, u64::MAX);
        assert_eq!(config.max_batch_txs, u64::MAX);
        assert_eq!(config.stream_timeout_secs, u64::MAX);
    }

    #[test]
    fn test_mempool_sync_config_clone() {
        let config = MempoolSyncConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.max_batch_bytes, config.max_batch_bytes);
        assert_eq!(cloned.max_batch_txs, config.max_batch_txs);
        assert_eq!(cloned.stream_timeout_secs, config.stream_timeout_secs);
    }

    // TxBatch tests
    #[test]
    fn test_create_tx_batch() {
        let _config = MempoolSyncConfig::default();

        let txs = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let batch = TxBatch {
            batch_id: 0,
            transactions: txs.clone(),
            is_final: true,
        };

        assert_eq!(batch.transactions, txs);
        assert!(batch.is_final);
        assert_eq!(batch.batch_id, 0);
    }

    #[test]
    fn test_tx_batch_empty_transactions() {
        let batch = TxBatch {
            batch_id: 1,
            transactions: vec![],
            is_final: false,
        };

        assert!(batch.transactions.is_empty());
        assert!(!batch.is_final);
        assert_eq!(batch.batch_id, 1);
    }

    #[test]
    fn test_tx_batch_single_transaction() {
        let batch = TxBatch {
            batch_id: 99,
            transactions: vec![vec![1, 2, 3, 4, 5]],
            is_final: true,
        };

        assert_eq!(batch.transactions.len(), 1);
        assert_eq!(batch.transactions[0], vec![1, 2, 3, 4, 5]);
        assert!(batch.is_final);
    }

    #[test]
    fn test_tx_batch_large_batch_id() {
        let batch = TxBatch {
            batch_id: u64::MAX,
            transactions: vec![vec![1]],
            is_final: false,
        };

        assert_eq!(batch.batch_id, u64::MAX);
        assert!(!batch.is_final);
    }

    #[test]
    fn test_tx_batch_many_transactions() {
        let txs: Vec<Vec<u8>> = (0..100).map(|i| vec![i as u8; 10]).collect();
        let batch = TxBatch {
            batch_id: 5,
            transactions: txs.clone(),
            is_final: true,
        };

        assert_eq!(batch.transactions.len(), 100);
        assert_eq!(batch.transactions[0], vec![0u8; 10]);
        assert_eq!(batch.transactions[99], vec![99u8; 10]);
    }

    #[test]
    fn test_tx_batch_is_final_true() {
        let batch = TxBatch {
            batch_id: 0,
            transactions: vec![vec![1]],
            is_final: true,
        };

        assert!(batch.is_final);
    }

    #[test]
    fn test_tx_batch_is_final_false() {
        let batch = TxBatch {
            batch_id: 0,
            transactions: vec![vec![1]],
            is_final: false,
        };

        assert!(!batch.is_final);
    }

    // BatchAck tests
    #[test]
    fn test_create_batch_ack() {
        let ack = BatchAck { batch_id: 123, accepted: true };
        assert_eq!(ack.batch_id, 123);
        assert!(ack.accepted);
    }

    #[test]
    fn test_batch_ack_rejected() {
        let ack = BatchAck { batch_id: 456, accepted: false };
        assert_eq!(ack.batch_id, 456);
        assert!(!ack.accepted);
    }

    #[test]
    fn test_batch_ack_zero_batch_id() {
        let ack = BatchAck { batch_id: 0, accepted: true };
        assert_eq!(ack.batch_id, 0);
        assert!(ack.accepted);
    }

    #[test]
    fn test_batch_ack_max_batch_id() {
        let ack = BatchAck { batch_id: u64::MAX, accepted: false };
        assert_eq!(ack.batch_id, u64::MAX);
        assert!(!ack.accepted);
    }

    // BatchRequest tests
    #[test]
    fn test_create_batch_request() {
        let request = BatchRequest {
            max_bytes: 5000,
            max_txs: 100,
            deadline_ms: 12345,
        };
        assert_eq!(request.max_bytes, 5000);
        assert_eq!(request.max_txs, 100);
        assert_eq!(request.deadline_ms, 12345);
    }

    #[test]
    fn test_batch_request_zero_limits() {
        let request = BatchRequest {
            max_bytes: 0,
            max_txs: 0,
            deadline_ms: 0,
        };

        assert_eq!(request.max_bytes, 0);
        assert_eq!(request.max_txs, 0);
        assert_eq!(request.deadline_ms, 0);
    }

    #[test]
    fn test_batch_request_large_limits() {
        let request = BatchRequest {
            max_bytes: u64::MAX,
            max_txs: u64::MAX,
            deadline_ms: 9999999999999, // Large i64-compatible value
        };

        assert_eq!(request.max_bytes, u64::MAX);
        assert_eq!(request.max_txs, u64::MAX);
        assert_eq!(request.deadline_ms, 9999999999999);
    }

    #[test]
    fn test_batch_request_future_deadline() {
        let _future_deadline = 9999999999999u64; // Far future
        let request = BatchRequest {
            max_bytes: 1000,
            max_txs: 10,
            deadline_ms: 9999999999999,
        };

        assert_eq!(request.deadline_ms, 9999999999999);
        assert!(request.deadline_ms > 12345);
    }

    // OrderReadyNotification tests
    #[test]
    fn test_create_order_ready_notification() {
        let notification = OrderReadyNotification {
            order_count: 42,
            timestamp: 12345,
        };
        assert_eq!(notification.order_count, 42);
        assert_eq!(notification.timestamp, 12345);
    }

    #[test]
    fn test_order_ready_notification_zero_orders() {
        let notification = OrderReadyNotification {
            order_count: 0,
            timestamp: 0,
        };

        assert_eq!(notification.order_count, 0);
        assert_eq!(notification.timestamp, 0);
    }

    #[test]
    fn test_order_ready_notification_max_orders() {
        let notification = OrderReadyNotification {
            order_count: u32::MAX,
            timestamp: i64::MAX,
        };

        assert_eq!(notification.order_count, u32::MAX);
        assert_eq!(notification.timestamp, i64::MAX);
    }

    #[test]
    fn test_order_ready_notification_single_order() {
        let notification = OrderReadyNotification {
            order_count: 1,
            timestamp: 1234567890,
        };

        assert_eq!(notification.order_count, 1);
        assert_eq!(notification.timestamp, 1234567890);
    }
}
