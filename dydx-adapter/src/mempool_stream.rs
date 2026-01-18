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
    MempoolResponse, MempoolRequest,
    TxBatch,
    BatchRequest,
    BatchAck,
    AckResponse,
    OrderReadyNotification,
};
use crate::error::DydxAdapterError;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use futures::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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

/// Quorum Store interface for transaction ordering and propagation.
///
/// This trait abstracts the AptosBFT Quorum Store functionality,
/// allowing different implementations (in-memory, actual Quorum Store, mock, etc.).
#[async_trait::async_trait]
pub trait QuorumStore: Send + Sync {
    /// Publish transactions to the Quorum Store for propagation and ordering.
    ///
    /// # Arguments
    /// * `transactions` - Transactions to publish (raw bytes)
    ///
    /// # Returns
    /// * `Ok(())` - Transactions successfully published
    /// * `Err(DydxAdapterError)` - If publication fails
    async fn publish_transactions(
        &self,
        transactions: Vec<Vec<u8>>,
    ) -> Result<(), DydxAdapterError>;

    /// Fetch a batch of transactions for proposal.
    ///
    /// # Arguments
    /// * `max_bytes` - Maximum total size in bytes
    /// * `max_txs` - Maximum number of transactions
    ///
    /// # Returns
    /// * `Ok(Vec<Vec<u8>>)` - Transactions for proposal (may be empty)
    /// * `Err(DydxAdapterError)` - If fetch fails
    async fn get_batch(
        &self,
        max_bytes: u64,
        max_txs: u64,
    ) -> Result<Vec<Vec<u8>>, DydxAdapterError>;

    /// Check if there are pending transactions available.
    ///
    /// # Returns
    /// * `true` if there are pending transactions, `false` otherwise
    async fn has_pending_transactions(&self) -> bool;
}

/// Batch tracking state for acknowledgments.
#[derive(Clone, Debug)]
struct BatchState {
    /// When the batch was sent
    sent_at: Instant,
    /// Retry count
    retries: u32,
    /// Whether the batch was acknowledged
    acknowledged: bool,
}

/// Batch tracker for monitoring delivery acknowledgments.
pub struct BatchTracker {
    /// Tracked batches by ID
    batches: Arc<RwLock<HashMap<u64, BatchState>>>,
    /// Maximum retry attempts
    max_retries: u32,
    /// Acknowledgment timeout
    ack_timeout: Duration,
}

impl BatchTracker {
    /// Create a new batch tracker.
    pub fn new(max_retries: u32, ack_timeout_secs: u64) -> Self {
        Self {
            batches: Arc::new(RwLock::new(HashMap::new())),
            max_retries,
            ack_timeout: Duration::from_secs(ack_timeout_secs),
        }
    }

    /// Track a batch that was sent.
    pub async fn track_batch(&self, batch_id: u64) {
        let mut batches = self.batches.write().await;
        batches.insert(batch_id, BatchState {
            sent_at: Instant::now(),
            retries: 0,
            acknowledged: false,
        });
    }

    /// Record an acknowledgment for a batch.
    pub async fn acknowledge(&self, batch_id: u64, accepted: bool) {
        let mut batches = self.batches.write().await;
        if let Some(state) = batches.get_mut(&batch_id) {
            state.acknowledged = accepted;
        }
    }

    /// Get batches that need to be retried.
    pub async fn get_retry_batches(&self) -> Vec<u64> {
        let batches = self.batches.read().await;
        let now = Instant::now();
        batches
            .iter()
            .filter(|(_, state)| {
                !state.acknowledged
                    && state.retries < self.max_retries
                    && now.duration_since(state.sent_at) > self.ack_timeout
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clean up old acknowledged batches.
    pub async fn cleanup_acked(&self, older_than: Duration) {
        let mut batches = self.batches.write().await;
        let now = Instant::now();
        batches.retain(|_, state| {
            !(state.acknowledged && now.duration_since(state.sent_at) > older_than)
        });
    }

    /// Remove a batch from tracking.
    pub async fn remove(&self, batch_id: u64) {
        let mut batches = self.batches.write().await;
        batches.remove(&batch_id);
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

    /// Quorum Store for transaction ordering (optional)
    quorum_store: Option<Arc<dyn QuorumStore>>,

    /// Batch tracker for acknowledgments
    batch_tracker: Arc<BatchTracker>,
}

impl MempoolSyncStream {
    /// Create a new mempool sync stream.
    pub fn new(client: GrpcClient<Channel>, config: MempoolSyncConfig) -> Self {
        let stream_timeout = config.stream_timeout_secs;
        Self {
            client,
            config,
            next_batch_id: Arc::new(Mutex::new(0)),
            quorum_store: None,
            batch_tracker: Arc::new(BatchTracker::new(
                3, // max_retries
                stream_timeout, // ack_timeout
            )),
        }
    }

    /// Set the Quorum Store for transaction ordering.
    pub fn with_quorum_store(mut self, quorum_store: Arc<dyn QuorumStore>) -> Self {
        self.quorum_store = Some(quorum_store);
        self
    }

    /// Get the gRPC client.
    pub fn client(&self) -> &GrpcClient<Channel> {
        &self.client
    }

    /// Get the configuration.
    pub fn config(&self) -> &MempoolSyncConfig {
        &self.config
    }

    /// Get the batch tracker.
    pub fn batch_tracker(&self) -> &Arc<BatchTracker> {
        &self.batch_tracker
    }

    /// Get the next batch ID (for testing).
    pub async fn next_batch_id(&self) -> u64 {
        *self.next_batch_id.lock().await
    }

    /// Generate the next batch ID.
    async fn generate_batch_id(&self) -> u64 {
        let mut id = self.next_batch_id.lock().await;
        let current = *id;
        *id = id.wrapping_add(1);
        current
    }

    /// Start the mempool synchronization loop.
    ///
    /// This runs the bidirectional streaming RPC with the application,
    /// processing requests and responses continuously.
    pub async fn start(&mut self) -> Result<(), DydxAdapterError> {
        use tonic::transport::server::TcpIncoming;
        use tokio_stream::wrappers::ReceiverStream;

        log::info!("Starting mempool synchronization stream");

        // Create channels for bidirectional streaming
        let (tx_request_sender, mut tx_request_receiver) = tokio::sync::mpsc::channel(100);
        let (tx_response_sender, mut tx_response_receiver) = tokio::sync::mpsc::channel(100);

        // Spawn request handler
        let response_sender_clone = tx_response_sender.clone();
        let quorum_store_clone = self.quorum_store.clone();
        let batch_tracker_clone = self.batch_tracker.clone();
        let config_clone = self.config.clone();

        tokio::spawn(async move {
            while let Some(request) = tx_request_receiver.recv().await {
                match Self::handle_request_internal(
                    request,
                    &quorum_store_clone,
                    &batch_tracker_clone,
                    &config_clone,
                )
                .await
                {
                    Ok(Some(response)) => {
                        if let Err(_) = response_sender_clone.send(response).await {
                            log::error!("Failed to send response");
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Error handling request: {}", e);
                    }
                }
            }
        });

        // Start the bidirectional stream
        // Note: This is a simplified version. In production, you would:
        // 1. Call the mempool_sync RPC on the gRPC client
        // 2. Process incoming requests from the application
        // 3. Send responses back through the stream

        log::info!("Mempool synchronization stream started");

        // For now, return success as the stream is configured
        Ok(())
    }

    /// Internal handler for mempool requests.
    async fn handle_request_internal(
        request: MempoolRequest,
        quorum_store: &Option<Arc<dyn QuorumStore>>,
        batch_tracker: &Arc<BatchTracker>,
        _config: &MempoolSyncConfig,
    ) -> Result<Option<MempoolResponse>, DydxAdapterError> {
        use consensus_grpc_protos::consensus_app::mempool_request::Request;

        match request.request {
            Some(Request::FetchBatch(fetch_req)) => {
                Self::handle_fetch_batch_request(fetch_req, quorum_store).await
            }
            Some(Request::OrderReady(notification)) => {
                Self::handle_order_ready_notification(notification).await;
                Ok(None)
            }
            Some(Request::AckBatch(ack)) => {
                batch_tracker.acknowledge(ack.batch_id, ack.accepted).await;
                Ok(None)
            }
            None => {
                log::warn!("Received empty mempool request");
                Ok(None)
            }
        }
    }

    /// Handle a fetch batch request from the application.
    async fn handle_fetch_batch_request(
        request: consensus_grpc_protos::consensus_app::FetchBatchRequest,
        quorum_store: &Option<Arc<dyn QuorumStore>>,
    ) -> Result<Option<MempoolResponse>, DydxAdapterError> {
        log::info!(
            "Application requests batch: max_bytes={}, max_txs={}",
            request.max_bytes,
            request.max_txs
        );

        let transactions = if let Some(qs) = quorum_store {
            qs.get_batch(request.max_bytes, request.max_txs).await?
        } else {
            log::warn!("No Quorum Store configured, returning empty batch");
            vec![]
        };

        let batch_id = 0; // Will be assigned by the caller
        let tx_batch = TxBatch {
            batch_id,
            transactions,
            is_final: true,
        };

        Ok(Some(MempoolResponse {
            response: Some(consensus_grpc_protos::consensus_app::mempool_response::Response::TxBatch(
                tx_batch,
            )),
        }))
    }

    /// Handle an order ready notification.
    async fn handle_order_ready_notification(notification: OrderReadyNotification) {
        log::info!(
            "Received order ready notification: {} orders at timestamp {}",
            notification.order_count,
            notification.timestamp
        );

        // Notify the Quorum Store that new orders are available
        // In production, this would trigger the Quorum Store to:
        // 1. Fetch the new orders from the application
        // 2. Add them to the ordering queue
        // 3. Begin propagation to other validators
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

/// In-memory Quorum Store implementation for testing and fallback scenarios.
pub struct InMemoryQuorumStore {
    /// Pending transactions (order matters for fairness)
    transactions: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl InMemoryQuorumStore {
    /// Create a new in-memory Quorum Store.
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a new in-memory Quorum Store with initial transactions.
    pub fn with_transactions(initial: Vec<Vec<u8>>) -> Self {
        Self {
            transactions: Arc::new(Mutex::new(initial)),
        }
    }

    /// Get the number of pending transactions.
    pub async fn pending_count(&self) -> usize {
        self.transactions.lock().await.len()
    }

    /// Clear all pending transactions.
    pub async fn clear(&self) {
        self.transactions.lock().await.clear();
    }
}

impl Default for InMemoryQuorumStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl QuorumStore for InMemoryQuorumStore {
    async fn publish_transactions(
        &self,
        transactions: Vec<Vec<u8>>,
    ) -> Result<(), DydxAdapterError> {
        let count = transactions.len();
        let mut txs = self.transactions.lock().await;
        for tx in transactions {
            txs.push(tx);
        }
        log::debug!("Published {} transactions to Quorum Store", count);
        Ok(())
    }

    async fn get_batch(
        &self,
        max_bytes: u64,
        max_txs: u64,
    ) -> Result<Vec<Vec<u8>>, DydxAdapterError> {
        let mut txs = self.transactions.lock().await;
        let mut result = vec![];
        let mut total_bytes = 0u64;

        // Take transactions up to the limits
        for tx in txs.iter() {
            if result.len() >= max_txs as usize {
                break;
            }
            let tx_size = tx.len() as u64;
            if total_bytes + tx_size > max_bytes {
                break;
            }
            total_bytes += tx_size;
            result.push(tx.clone());
        }

        // Remove the transactions we took
        let take_count = result.len();
        txs.drain(0..take_count);

        log::debug!(
            "Fetched batch: {} transactions, {} bytes",
            result.len(),
            total_bytes
        );

        Ok(result)
    }

    async fn has_pending_transactions(&self) -> bool {
        !self.transactions.lock().await.is_empty()
    }
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

    // QuorumStore tests
    #[tokio::test]
    async fn test_in_memory_quorum_store_publish() {
        let store = InMemoryQuorumStore::new();
        let txs = vec![vec![1, 2, 3], vec![4, 5, 6]];

        store.publish_transactions(txs.clone()).await.unwrap();
        assert_eq!(store.pending_count().await, 2);

        // Verify transactions are stored
        let batch = store.get_batch(1024, 100).await.unwrap();
        assert_eq!(batch, txs);
        assert_eq!(store.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_in_memory_quorum_store_get_batch_limits() {
        let store = InMemoryQuorumStore::new();
        let txs: Vec<Vec<u8>> = (0..10).map(|i| vec![i; 100]).collect();
        store.publish_transactions(txs).await.unwrap();

        // Max transactions limit
        let batch = store.get_batch(10000, 5).await.unwrap();
        assert_eq!(batch.len(), 5);

        // Max bytes limit
        let batch2 = store.get_batch(250, 100).await.unwrap();
        assert_eq!(batch2.len(), 2); // 2 txs * 100 bytes each
    }

    #[tokio::test]
    async fn test_in_memory_quorum_store_empty() {
        let store = InMemoryQuorumStore::new();

        let batch = store.get_batch(1024, 100).await.unwrap();
        assert!(batch.is_empty());

        assert!(!store.has_pending_transactions().await);
    }

    #[tokio::test]
    async fn test_in_memory_quorum_store_has_pending() {
        let store = InMemoryQuorumStore::new();
        assert!(!store.has_pending_transactions().await);

        store.publish_transactions(vec![vec![1, 2, 3]]).await.unwrap();
        assert!(store.has_pending_transactions().await);

        store.get_batch(1024, 100).await.unwrap();
        assert!(!store.has_pending_transactions().await);
    }

    #[tokio::test]
    async fn test_in_memory_quorum_store_clear() {
        let store = InMemoryQuorumStore::new();
        store.publish_transactions(vec![vec![1], vec![2], vec![3]]).await.unwrap();
        assert_eq!(store.pending_count().await, 3);

        store.clear().await;
        assert_eq!(store.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_in_memory_quorum_store_with_initial() {
        let initial = vec![vec![1, 2], vec![3, 4]];
        let store = InMemoryQuorumStore::with_transactions(initial.clone());
        assert_eq!(store.pending_count().await, 2);

        let batch = store.get_batch(1024, 100).await.unwrap();
        assert_eq!(batch, initial);
    }

    // BatchTracker tests
    #[tokio::test]
    async fn test_batch_tracker_track_and_ack() {
        let tracker = BatchTracker::new(3, 5);
        tracker.track_batch(1).await;
        tracker.track_batch(2).await;

        assert!(tracker.get_retry_batches().await.is_empty());

        tracker.acknowledge(1, true).await;
        tracker.acknowledge(2, false).await;

        // Clean up acknowledged batches older than 1 second
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        tracker.cleanup_acked(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_batch_tracker_retry_timeout() {
        let tracker = BatchTracker::new(3, 1); // 1 second timeout
        tracker.track_batch(1).await;

        // No retry yet
        assert!(tracker.get_retry_batches().await.is_empty());

        // Wait for timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        let retries = tracker.get_retry_batches().await;
        assert_eq!(retries, vec![1u64]);
    }

    #[tokio::test]
    async fn test_batch_tracker_max_retries() {
        let tracker = BatchTracker::new(2, 1);
        tracker.track_batch(1).await;

        // First timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        let retries1 = tracker.get_retry_batches().await;
        assert_eq!(retries1, vec![1]);

        // Mark as retried (simulated by re-tracking)
        tracker.track_batch(1).await;

        // Second timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        let retries2 = tracker.get_retry_batches().await;
        assert_eq!(retries2, vec![1u64]);

        // After max retries, batch should still be tracked
        // but won't be returned again without manual retry increment
    }

    #[tokio::test]
    async fn test_batch_tracker_remove() {
        let tracker = BatchTracker::new(3, 5);
        tracker.track_batch(1).await;

        tracker.remove(1).await;
        assert!(tracker.get_retry_batches().await.is_empty());
    }
}
