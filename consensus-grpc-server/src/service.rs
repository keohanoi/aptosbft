// ConsensusApp service trait and implementation
//
// This module defines the interface that applications must implement to
// handle gRPC requests from AptosBFT consensus.

use std::pin::Pin;
use consensus_grpc_protos::{
    BuildBlockRequest, BuildBlockResponse,
    VerifyBlockRequest, VerifyBlockResponse,
    ExecuteBlockRequest, ExecuteBlockResponse,
    VoteExtensionRequest, VoteExtensionResponse,
    CommitBlockRequest, CommitBlockResponse,
    MempoolRequest, MempoolResponse,
    QueryRequest, QueryResponse,
};
use futures::Stream;
use tonic::Request;
use crate::error::{Error, Result};

/// Trait that applications must implement to handle ConsensusApp requests.
///
/// This trait abstracts the application logic that AptosBFT needs to execute.
/// Implementations typically forward requests to the actual application
/// (e.g., dYdX v4) via gRPC or shared memory.
#[async_trait::async_trait]
pub trait ConsensusAppService: Send + Sync + 'static {
    /// Build a block proposal.
    ///
    /// Called by AptosBFT's proposer to:
    /// 1. Fetch pending transactions from mempool
    /// 2. Execute PrepareProposal logic (order matching for dYdX)
    /// 3. Return assembled block data
    async fn build_block(&self, request: BuildBlockRequest) -> Result<BuildBlockResponse>;

    /// Verify a proposed block.
    ///
    /// Called by AptosBFT's non-proposing validators to:
    /// 1. Validate block structure and semantics
    /// 2. Re-execute matching engine deterministically
    /// 3. Verify the proposer acted honestly
    async fn verify_block(&self, request: VerifyBlockRequest) -> Result<VerifyBlockResponse>;

    /// Execute a block's transactions.
    ///
    /// Called after a QuorumCertificate is formed to:
    /// 1. Execute all transactions in the block
    /// 2. Update application state
    /// 3. Compute new state root (app hash)
    /// 4. Return execution results for commitment
    async fn execute_block(&self, request: ExecuteBlockRequest) -> Result<ExecuteBlockResponse>;

    /// Generate vote extension data.
    ///
    /// Called by AptosBFT before voting to:
    /// 1. Fetch application-specific data (e.g., oracle prices)
    /// 2. Sign and return data for inclusion in Precommit votes
    async fn generate_vote_extension(
        &self,
        request: VoteExtensionRequest,
    ) -> Result<VoteExtensionResponse>;

    /// Commit a block.
    ///
    /// Called after execute_block succeeds and the block is finalized:
    /// 1. Persist state changes
    /// 2. Update committed block height
    /// 3. Trigger any post-commit hooks
    async fn commit_block(&self, request: CommitBlockRequest) -> Result<CommitBlockResponse>;

    /// Handle mempool synchronization (bidirectional streaming).
    ///
    /// This method receives a stream of requests from the application and
    /// returns a stream of responses. It's used for:
    /// 1. Order streaming from Quorum Store to application
    /// 2. Batch fetching of transactions on demand
    /// 3. Mempool gossip coordination
    fn mempool_sync(
        &self,
        request: Request<tonic::Streaming<MempoolRequest>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<MempoolResponse>> + Send>>>;

    /// Query application state.
    ///
    /// Used for lightweight queries without full block execution:
    /// 1. Validator set queries
    /// 2. Account balance checks
    /// 3. State proof generation
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse>;
}

/// Default implementation that returns errors for all operations.
///
/// This is useful for testing or as a starting point for custom implementations.
pub struct DefaultConsensusAppService;

#[async_trait::async_trait]
impl ConsensusAppService for DefaultConsensusAppService {
    async fn build_block(&self, _request: BuildBlockRequest) -> Result<BuildBlockResponse> {
        Err(Error::Internal("Not implemented".to_string()))
    }

    async fn verify_block(&self, _request: VerifyBlockRequest) -> Result<VerifyBlockResponse> {
        Ok(VerifyBlockResponse {
            accepted: false,
            reject_reason: "Not implemented".to_string(),
            gas_used: 0,
        })
    }

    async fn execute_block(&self, _request: ExecuteBlockRequest) -> Result<ExecuteBlockResponse> {
        Err(Error::Internal("Not implemented".to_string()))
    }

    async fn generate_vote_extension(
        &self,
        _request: VoteExtensionRequest,
    ) -> Result<VoteExtensionResponse> {
        Ok(VoteExtensionResponse {
            extension: vec![],
            signature: vec![],
        })
    }

    async fn commit_block(&self, _request: CommitBlockRequest) -> Result<CommitBlockResponse> {
        Ok(CommitBlockResponse {
            hash: vec![],
            prune_height: 0,
        })
    }

    fn mempool_sync(
        &self,
        _request: Request<tonic::Streaming<MempoolRequest>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<MempoolResponse>> + Send>>> {
        Err(Error::Internal("Not implemented".to_string()))
    }

    async fn query(&self, _request: QueryRequest) -> Result<QueryResponse> {
        Err(Error::Internal("Not implemented".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_service_build_block_error() {
        let service = DefaultConsensusAppService;
        let result = service.build_block(BuildBlockRequest {
            height: 1,
            ..Default::default()
        }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_service_verify_block_rejects() {
        let service = DefaultConsensusAppService;
        let result = service.verify_block(VerifyBlockRequest {
            height: 1,
            ..Default::default()
        }).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().accepted);
    }

    #[tokio::test]
    async fn test_default_service_generate_vote_extension_empty() {
        let service = DefaultConsensusAppService;
        let result = service.generate_vote_extension(VoteExtensionRequest {
            height: 1,
            ..Default::default()
        }).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.extension.is_empty());
        assert!(response.signature.is_empty());
    }

    #[tokio::test]
    async fn test_default_service_commit_block_ok() {
        let service = DefaultConsensusAppService;
        let result = service.commit_block(CommitBlockRequest {
            height: 1,
            ..Default::default()
        }).await;
        assert!(result.is_ok());
    }
}
