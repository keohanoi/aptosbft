// gRPC server implementation for the ConsensusApp service
//
// This module implements the tonic-generated ConsensusApp trait, routing
// gRPC requests to the application-specific ConsensusAppService implementation.

use std::pin::Pin;
use std::sync::Arc;
use futures::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use consensus_grpc_protos::consensus_app::{
    consensus_app_server::ConsensusApp as ConsensusAppTrait,
    consensus_app_server::ConsensusAppServer as TonicConsensusAppServer,
    BuildBlockRequest, BuildBlockResponse,
    VerifyBlockRequest, VerifyBlockResponse,
    ExecuteBlockRequest, ExecuteBlockResponse,
    VoteExtensionRequest, VoteExtensionResponse,
    CommitBlockRequest, CommitBlockResponse,
    MempoolRequest, MempoolResponse,
    QueryRequest, QueryResponse,
};
use crate::service::ConsensusAppService;

/// The gRPC server that handles ConsensusApp requests.
///
/// This server implements the tonic-generated ConsensusApp trait and routes
/// requests to the provided ConsensusAppService implementation.
#[derive(Clone)]
pub struct ConsensusServer {
    /// The application service that handles actual request processing
    service: Arc<dyn ConsensusAppService>,
}

impl ConsensusServer {
    /// Create a new ConsensusServer with the given application service.
    pub fn new(service: Arc<dyn ConsensusAppService>) -> Self {
        Self { service }
    }

    /// Convert this server into a tonic Server for use with tonic::Server.
    pub fn into_server(self) -> TonicConsensusAppServer<ConsensusServer> {
        TonicConsensusAppServer::new(self)
    }

    /// Create a server that serves on the given address.
    ///
    /// This is a convenience method for starting a server. For more control
    /// over server configuration, use `into_server()` with tonic::Server::builder().
    pub async fn serve(self, addr: String) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let addr = addr.parse()?;
        let server = self.into_server();

        log::info!("ConsensusApp gRPC server listening on {}", addr);

        tonic::transport::Server::builder()
            .add_service(server)
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl ConsensusAppTrait for ConsensusServer {
    type MempoolSyncStream = Pin<Box<dyn Stream<Item = std::result::Result<MempoolResponse, Status>> + Send>>;

    async fn build_block(
        &self,
        request: Request<BuildBlockRequest>,
    ) -> std::result::Result<Response<BuildBlockResponse>, Status> {
        let req = request.into_inner();
        log::debug!("BuildBlock request: height={}, round={}", req.height, req.round);

        self.service
            .build_block(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }

    async fn verify_block(
        &self,
        request: Request<VerifyBlockRequest>,
    ) -> std::result::Result<Response<VerifyBlockResponse>, Status> {
        let req = request.into_inner();
        log::debug!("VerifyBlock request: height={}, round={}", req.height, req.round);

        self.service
            .verify_block(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }

    async fn execute_block(
        &self,
        request: Request<ExecuteBlockRequest>,
    ) -> std::result::Result<Response<ExecuteBlockResponse>, Status> {
        let req = request.into_inner();
        log::debug!(
            "ExecuteBlock request: height={}, round={}, txs={}",
            req.height,
            req.round,
            req.transactions.len()
        );

        self.service
            .execute_block(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }

    async fn generate_vote_extension(
        &self,
        request: Request<VoteExtensionRequest>,
    ) -> std::result::Result<Response<VoteExtensionResponse>, Status> {
        let req = request.into_inner();
        log::debug!(
            "GenerateVoteExtension request: height={}, round={}",
            req.height,
            req.round
        );

        self.service
            .generate_vote_extension(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }

    async fn commit_block(
        &self,
        request: Request<CommitBlockRequest>,
    ) -> std::result::Result<Response<CommitBlockResponse>, Status> {
        let req = request.into_inner();
        log::debug!("CommitBlock request: height={}", req.height);

        self.service
            .commit_block(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }

    async fn mempool_sync(
        &self,
        request: Request<tonic::Streaming<MempoolRequest>>,
    ) -> std::result::Result<Response<Self::MempoolSyncStream>, Status> {
        log::debug!("MempoolSync stream initiated");

        let stream = self.service
            .mempool_sync(request)
            .map_err(|e| e.to_status())?;

        // Convert our Result stream to a Status stream
        let stream = stream.map(|item| item.map_err(|e| e.to_status()));

        Ok(Response::new(Box::pin(stream)))
    }

    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> std::result::Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        log::debug!(
            "Query request: height={}, path={:?}",
            req.height,
            String::from_utf8_lossy(&req.path)
        );

        self.service
            .query(req)
            .await
            .map(Response::new)
            .map_err(|e| e.to_status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use std::sync::Arc;

    // A mock service for testing
    struct MockService;

    #[async_trait::async_trait]
    impl ConsensusAppService for MockService {
        async fn build_block(&self, _req: BuildBlockRequest) -> Result<BuildBlockResponse> {
            Ok(BuildBlockResponse {
                transactions: vec![vec![1, 2, 3]],
                extensions: vec![],
                state_update_hint: None,
            })
        }

        async fn verify_block(&self, _req: VerifyBlockRequest) -> Result<VerifyBlockResponse> {
            Ok(VerifyBlockResponse {
                accepted: true,
                reject_reason: String::new(),
                gas_used: 1000,
            })
        }

        async fn execute_block(&self, _req: ExecuteBlockRequest) -> Result<ExecuteBlockResponse> {
            Ok(ExecuteBlockResponse {
                new_state_root: vec![1, 2, 3],
                tx_results: vec![],
                events: vec![],
                gas_used: 1000,
                validator_updates: vec![],
            })
        }

        async fn generate_vote_extension(
            &self,
            _req: VoteExtensionRequest,
        ) -> Result<VoteExtensionResponse> {
            Ok(VoteExtensionResponse {
                extension: vec![4, 5, 6],
                signature: vec![7, 8, 9],
            })
        }

        async fn commit_block(&self, _req: CommitBlockRequest) -> Result<CommitBlockResponse> {
            Ok(CommitBlockResponse {
                hash: vec![10, 11, 12],
                prune_height: 0,
            })
        }

        fn mempool_sync(
            &self,
            _req: Request<tonic::Streaming<MempoolRequest>>,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<MempoolResponse>> + Send>>> {
            // Create a simple stream that yields one item then ends
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx.send(Ok(MempoolResponse {
                    response: None,
                })).await;
            });
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            Ok(Box::pin(stream))
        }

        async fn query(&self, _req: QueryRequest) -> Result<QueryResponse> {
            Ok(QueryResponse {
                value: vec![13, 14, 15],
                proof: vec![],
                height: 1,
                log: vec![],
            })
        }
    }

    #[tokio::test]
    async fn test_build_block() {
        let server = ConsensusServer::new(Arc::new(MockService));
        let request = Request::new(BuildBlockRequest {
            height: 1,
            round: 1,
            ..Default::default()
        });

        let response = server.build_block(request).await.unwrap();
        let response = response.into_inner();
        assert_eq!(response.transactions.len(), 1);
        assert_eq!(response.transactions[0], vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_verify_block() {
        let server = ConsensusServer::new(Arc::new(MockService));
        let request = Request::new(VerifyBlockRequest {
            height: 1,
            ..Default::default()
        });

        let response = server.verify_block(request).await.unwrap();
        let response = response.into_inner();
        assert!(response.accepted);
    }

    #[tokio::test]
    async fn test_generate_vote_extension() {
        let server = ConsensusServer::new(Arc::new(MockService));
        let request = Request::new(VoteExtensionRequest {
            height: 1,
            ..Default::default()
        });

        let response = server.generate_vote_extension(request).await.unwrap();
        let response = response.into_inner();
        assert_eq!(response.extension, vec![4, 5, 6]);
        assert_eq!(response.signature, vec![7, 8, 9]);
    }
}
