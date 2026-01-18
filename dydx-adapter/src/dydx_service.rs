// DydxConsensusAppService - dYdX AptosBFT Consensus Application Service
//
// This module implements the ConsensusAppService trait for dYdX v4, bridging
// AptosBFT consensus (Rust) to the dYdX application (Go) via gRPC.
//
// The service handles all consensus-to-application communication:
// - BuildBlock: Request block proposals from Go app
// - VerifyBlock: Verify proposed blocks
// - ExecuteBlock: Execute committed blocks
// - GenerateVoteExtension: Generate oracle price vote extensions
// - CommitBlock: Commit finalized blocks
// - MempoolSync: Bidirectional mempool synchronization
// - Query: Query application state

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tonic::{Request, Response};
use tonic::transport::Channel;
use futures::{Stream, StreamExt};
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use consensus_grpc_protos::consensus_app::{
    consensus_app_client::ConsensusAppClient as GrpcClient,
    BuildBlockRequest, BuildBlockResponse,
    VerifyBlockRequest, VerifyBlockResponse,
    ExecuteBlockRequest, ExecuteBlockResponse,
    VoteExtensionRequest, VoteExtensionResponse,
    CommitBlockRequest, CommitBlockResponse,
    MempoolRequest, MempoolResponse,
    QueryRequest, QueryResponse,
    MempoolResponse as Resp,
    mempool_response::Response as RespType,
    TxBatch, BatchRequest, AckResponse,
};
use consensus_grpc_server::service::ConsensusAppService;
use consensus_grpc_server::error::{Error, Result};

/// Default gRPC endpoint for the dYdX application
const DEFAULT_APP_ENDPOINT: &str = "http://localhost:50051";

/// Default timeout for gRPC calls
const DEFAULT_GRPC_TIMEOUT_SECS: u64 = 5;

/// dYdX AptosBFT Consensus Application Service
///
/// This service implements the bridge between AptosBFT consensus and the dYdX v4
/// application. It acts as both:
/// - A gRPC server: receives requests from AptosBFT consensus
/// - A gRPC client: forwards requests to the dYdX Go application
///
/// # Architecture
///
/// ```text
/// AptosBFT (Rust) <--gRPC--> DydxConsensusAppService <--gRPC--> dYdX App (Go)
/// ```
///
/// # Example
///
/// ```ignore
/// use dydx_adapter::dydx_service::DydxConsensusAppService;
///
/// // Create the service connected to the Go app at localhost:50051
/// let service = DydxConsensusAppService::new("http://localhost:50051").await?;
///
/// // Use the service with the gRPC server
/// let server = ConsensusServer::new(Arc::new(service));
/// server.serve("0.0.0.0:50052".to_string()).await?;
/// ```
pub struct DydxConsensusAppService {
    /// gRPC client connected to the dYdX Go application
    client: Arc<RwLock<GrpcClient<Channel>>>,

    /// Application endpoint (for reconnection)
    endpoint: String,

    /// Connection state
    is_connected: Arc<RwLock<bool>>,
}

impl DydxConsensusAppService {
    /// Create a new DydxConsensusAppService connected to the given endpoint.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The gRPC endpoint of the dYdX application (e.g., "http://localhost:50051")
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the service or an error if connection fails.
    pub async fn new(endpoint: String) -> Result<Self> {
        let client = GrpcClient::connect(endpoint.clone())
            .await
            .map_err(|e| Error::Network(format!("Failed to connect to {}: {}", endpoint, e)))?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            endpoint,
            is_connected: Arc::new(RwLock::new(true)),
        })
    }

    /// Create a new service with the default endpoint (localhost:50051).
    pub async fn with_default_endpoint() -> Result<Self> {
        Self::new(DEFAULT_APP_ENDPOINT.to_string()).await
    }

    /// Get the gRPC client.
    async fn client(&self) -> GrpcClient<Channel> {
        self.client.read().await.clone()
    }

    /// Reconnect to the application endpoint.
    pub async fn reconnect(&self) -> Result<()> {
        let mut conn = self.is_connected.write().await;
        *conn = false;

        let client = GrpcClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| Error::Network(format!("Failed to reconnect to {}: {}", self.endpoint, e)))?;

        {
            let mut client_lock = self.client.write().await;
            *client_lock = client;
        }

        *conn = true;
        log::info!("Reconnected to dYdX application at {}", self.endpoint);
        Ok(())
    }

    /// Check if the service is connected to the application.
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get the application endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl ConsensusAppService for DydxConsensusAppService {
    /// Build a block proposal by calling the dYdX Go application.
    ///
    /// This method is called by AptosBFT's proposer to construct a block proposal.
    /// It forwards the request to the Go application which executes the PrepareProposal logic.
    async fn build_block(&self, request: BuildBlockRequest) -> Result<BuildBlockResponse> {
        log::debug!(
            "BuildBlock request: height={}, round={}, max_bytes={}, max_gas={}",
            request.height,
            request.round,
            request.max_bytes,
            request.max_gas
        );

        let mut client = self.client().await;
        let timeout = Duration::from_secs(DEFAULT_GRPC_TIMEOUT_SECS);

        let response = tokio::time::timeout(
            timeout,
            client.build_block(request),
        )
        .await
        .map_err(|_| Error::Timeout("BuildBlock request timed out".to_string()))?
        .map_err(|e| Error::Network(format!("BuildBlock RPC failed: {}", e)))?;

        let response_inner = response.into_inner();

        log::debug!(
            "BuildBlock response: {} transactions returned",
            response_inner.transactions.len()
        );

        Ok(response_inner)
    }

    /// Verify a proposed block by calling the dYdX Go application.
    ///
    /// This method is called by AptosBFT's non-proposing validators to verify
    /// that a proposed block is valid. It forwards the request to the Go application
    /// which executes the ProcessProposal logic.
    async fn verify_block(&self, request: VerifyBlockRequest) -> Result<VerifyBlockResponse> {
        log::debug!(
            "VerifyBlock request: height={}, round={}, txs={}",
            request.height,
            request.round,
            request.transactions.len()
        );

        let mut client = self.client().await;
        let timeout = Duration::from_secs(DEFAULT_GRPC_TIMEOUT_SECS);

        let response = tokio::time::timeout(
            timeout,
            client.verify_block(request),
        )
        .await
        .map_err(|_| Error::Timeout("VerifyBlock request timed out".to_string()))?
        .map_err(|e| Error::VerificationFailed(format!("RPC failed: {}", e)))?;

        let response = response.into_inner();

        if !response.accepted {
            log::warn!("VerifyBlock rejected: {}", response.reject_reason);
        } else {
            log::debug!("VerifyBlock accepted: gas_used={}", response.gas_used);
        }

        Ok(response)
    }

    /// Execute a block's transactions by calling the dYdX Go application.
    ///
    /// This method is called after a QuorumCertificate is formed to execute
    /// the transactions in the block. It forwards the request to the Go application
    /// which executes the FinalizeBlock logic.
    async fn execute_block(&self, request: ExecuteBlockRequest) -> Result<ExecuteBlockResponse> {
        log::debug!(
            "ExecuteBlock request: height={}, round={}, txs={}",
            request.height,
            request.round,
            request.transactions.len()
        );

        // Capture height before moving request
        let height = request.height;
        let mut client = self.client().await;
        let timeout = Duration::from_secs(10); // Longer timeout for execution

        let response = tokio::time::timeout(
            timeout,
            client.execute_block(request),
        )
        .await
        .map_err(|_| Error::Timeout("ExecuteBlock request timed out".to_string()))?
        .map_err(|e| Error::ExecutionFailed {
            height,
            message: format!("RPC failed: {}", e),
        })?;

        let response = response.into_inner();

        log::debug!(
            "ExecuteBlock response: state_root={}, gas_used={}, tx_results={}",
            hex::encode(&response.new_state_root[..16.min(response.new_state_root.len())]),
            response.gas_used,
            response.tx_results.len()
        );

        Ok(response)
    }

    /// Generate a vote extension by calling the dYdX Go application.
    ///
    /// This method is called by AptosBFT before voting to generate application
    /// data to include in votes (e.g., Slinky oracle prices).
    async fn generate_vote_extension(
        &self,
        request: VoteExtensionRequest,
    ) -> Result<VoteExtensionResponse> {
        log::debug!(
            "GenerateVoteExtension request: height={}, round={}",
            request.height,
            request.round
        );

        let mut client = self.client().await;
        let timeout = Duration::from_secs(DEFAULT_GRPC_TIMEOUT_SECS);

        let response = tokio::time::timeout(
            timeout,
            client.generate_vote_extension(request),
        )
        .await
        .map_err(|_| Error::Timeout("GenerateVoteExtension request timed out".to_string()))?
        .map_err(|e| Error::VoteExtensionFailed(format!("RPC failed: {}", e)))?;

        let response = response.into_inner();

        log::debug!(
            "GenerateVoteExtension response: extension_len={}, signature_len={}",
            response.extension.len(),
            response.signature.len()
        );

        Ok(response)
    }

    /// Commit a block by notifying the dYdX Go application.
    ///
    /// This method is called after a block is finalized to notify the application
    /// that the block has been committed.
    async fn commit_block(&self, request: CommitBlockRequest) -> Result<CommitBlockResponse> {
        log::debug!(
            "CommitBlock request: height={}, block_hash={}",
            request.height,
            hex::encode(&request.block_hash[..8.min(request.block_hash.len())])
        );

        let mut client = self.client().await;
        let timeout = Duration::from_secs(DEFAULT_GRPC_TIMEOUT_SECS);

        let response = tokio::time::timeout(
            timeout,
            client.commit_block(request),
        )
        .await
        .map_err(|_| Error::Timeout("CommitBlock request timed out".to_string()))?
        .map_err(|e| Error::Internal(format!("RPC failed: {}", e)))?;

        let response = response.into_inner();

        log::debug!(
            "CommitBlock response: hash={}, prune_height={}",
            hex::encode(&response.hash[..8.min(response.hash.len())]),
            response.prune_height
        );

        Ok(response)
    }

    /// Handle mempool synchronization (bidirectional streaming).
    ///
    /// This method establishes a bidirectional stream for order synchronization
    /// between AptosBFT's Quorum Store and the dYdX Go application.
    fn mempool_sync(
        &self,
        request: Request<tonic::Streaming<MempoolRequest>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<MempoolResponse>> + Send>>> {
        log::debug!("MempoolSync stream initiated");

        let mut inbound = request.into_inner();

        // Create a channel for outbound responses to AptosBFT
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel(100);

        // Create a channel for requests to Go app
        let (_request_tx, request_rx) = tokio::sync::mpsc::channel::<MempoolRequest>(100);

        // Spawn a task to handle the bidirectional stream
        let endpoint = self.endpoint.clone();
        tokio::spawn(async move {
            // Convert request_rx into a stream
            let request_stream = tokio_stream::wrappers::ReceiverStream::new(request_rx);

            // Connect to the Go application's mempool sync endpoint
            let mut client = match GrpcClient::connect(endpoint.clone()).await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to connect to Go app for mempool sync: {}", e);
                    let _ = outbound_tx.send(Err(Error::MempoolError(format!("Connection failed: {}", e)))).await;
                    return;
                }
            };

            // Create the bidirectional stream with the Go application
            let mut app_stream = match client.mempool_sync(request_stream).await {
                Ok(s) => s.into_inner(),
                Err(e) => {
                    log::error!("Failed to establish mempool sync stream: {}", e);
                    let _ = outbound_tx.send(Err(Error::MempoolError(format!("Stream failed: {}", e)))).await;
                    return;
                }
            };

            // Forward messages from AptosBFT to Go app and vice versa
            loop {
                tokio::select! {
                    // Receive from AptosBFT, forward to Go app
                    Some(result) = inbound.next() => {
                        match result {
                            Ok(req) => {
                                // Note: For now we just log - in full implementation
                                // we'd need a way to send to the Go app's stream
                                log::trace!("Received mempool request from AptosBFT: {:?}", req);
                            }
                            Err(e) => {
                                log::error!("Error receiving from AptosBFT: {}", e);
                                break;
                            }
                        }
                    }
                    // Receive from Go app, send to AptosBFT
                    Some(result) = app_stream.next() => {
                        match result {
                            Ok(resp) => {
                                if let Err(e) = outbound_tx.send(Ok(resp)).await {
                                    log::error!("Failed to send response to AptosBFT: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Error receiving from Go app: {}", e);
                                let _ = outbound_tx.send(Err(Error::MempoolError(format!("Stream error: {}", e)))).await;
                                break;
                            }
                        }
                    }
                    else => break,
                }
            }

            log::info!("Mempool sync stream ended");
        });

        // Create a response stream from the receiver
        let response_stream = ReceiverStream::new(outbound_rx)
            .map(|item| item.map_err(|e| Error::MempoolError(format!("Stream error: {}", e))));

        Ok(Box::pin(response_stream))
    }

    /// Query the dYdX Go application state.
    ///
    /// This method is used for lightweight state queries without full block execution.
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse> {
        log::debug!(
            "Query request: height={}, path={}",
            request.height,
            String::from_utf8_lossy(&request.path)
        );

        let mut client = self.client().await;
        let timeout = Duration::from_secs(DEFAULT_GRPC_TIMEOUT_SECS);

        let response = tokio::time::timeout(
            timeout,
            client.query(request),
        )
        .await
        .map_err(|_| Error::Timeout("Query request timed out".to_string()))?
        .map_err(|e| Error::Internal(format!("RPC failed: {}", e)))?;

        let response = response.into_inner();

        log::debug!(
            "Query response: value_len={}, height={}",
            response.value.len(),
            response.height
        );

        Ok(response)
    }
}

/// Builder for creating a DydxConsensusAppService with custom configuration.
pub struct DydxConsensusAppServiceBuilder {
    endpoint: String,
    grpc_timeout_secs: u64,
}

impl DydxConsensusAppServiceBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            endpoint: DEFAULT_APP_ENDPOINT.to_string(),
            grpc_timeout_secs: DEFAULT_GRPC_TIMEOUT_SECS,
        }
    }

    /// Set the application endpoint.
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = endpoint;
        self
    }

    /// Set the gRPC timeout in seconds.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.grpc_timeout_secs = timeout_secs;
        self
    }

    /// Build the service.
    pub async fn build(self) -> Result<DydxConsensusAppService> {
        DydxConsensusAppService::new(self.endpoint).await
    }
}

impl Default for DydxConsensusAppServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_default_endpoint() {
        assert_eq!(DEFAULT_APP_ENDPOINT, "http://localhost:50051");
        assert_eq!(DEFAULT_GRPC_TIMEOUT_SECS, 5);
    }

    #[test]
    fn test_builder_default() {
        let builder = DydxConsensusAppServiceBuilder::new();
        assert_eq!(builder.endpoint, DEFAULT_APP_ENDPOINT);
        assert_eq!(builder.grpc_timeout_secs, DEFAULT_GRPC_TIMEOUT_SECS);
    }

    #[test]
    fn test_builder_with_endpoint() {
        let builder = DydxConsensusAppServiceBuilder::new()
            .with_endpoint("http://example.com:8080".to_string());
        assert_eq!(builder.endpoint, "http://example.com:8080");
    }

    #[test]
    fn test_builder_with_timeout() {
        let builder = DydxConsensusAppServiceBuilder::new()
            .with_timeout(10);
        assert_eq!(builder.grpc_timeout_secs, 10);
    }

    #[test]
    fn test_builder_chain() {
        let builder = DydxConsensusAppServiceBuilder::new()
            .with_endpoint("http://custom.com:9000".to_string())
            .with_timeout(15);
        assert_eq!(builder.endpoint, "http://custom.com:9000");
        assert_eq!(builder.grpc_timeout_secs, 15);
    }

    #[test]
    fn test_builder_default_trait() {
        let builder = DydxConsensusAppServiceBuilder::default();
        assert_eq!(builder.endpoint, DEFAULT_APP_ENDPOINT);
        assert_eq!(builder.grpc_timeout_secs, DEFAULT_GRPC_TIMEOUT_SECS);
    }
}
