// StateComputer implementation for dYdX v4
//
// This module implements the StateComputer trait from AptosBFT, bridging
// consensus to the dYdX application via the gRPC ConsensusApp service.

use tonic::transport::Channel;
use async_trait::async_trait;
use consensus_grpc_protos::consensus_app::{
    consensus_app_client::ConsensusAppClient as GrpcClient,
    BuildBlockRequest,
    ExecuteBlockRequest,
    ExecuteBlockResponse,
};
use consensus_traits::{
    StateComputer,
    Block as BlockTrait,
};
use crate::types::{
    DydxBlock, DydxHash, DydxLedgerInfo,
    DydxCommitInfo,
};
use crate::error::DydxAdapterError;

/// dYdX StateComputer that communicates with the Go application via gRPC.
///
/// DydxStateComputer implements the AptosBFT StateComputer trait by forwarding
/// execution requests to the dYdX v4 Go application through the ConsensusApp
/// gRPC service. This allows AptosBFT consensus (Rust) to drive the dYdX
/// application state machine (Go).
pub struct DydxStateComputer {
    /// gRPC client connected to the dYdX application
    client: GrpcClient<Channel>,

    /// Cached application endpoint for reconnection
    endpoint: String,
}

impl DydxStateComputer {
    /// Create a new DydxStateComputer connected to the given endpoint.
    ///
    /// The endpoint should be in the format "host:port", e.g., "localhost:50051".
    pub async fn new(endpoint: String) -> Result<Self, DydxAdapterError> {
        let client = GrpcClient::connect(endpoint.clone())
            .await
            .map_err(|e| DydxAdapterError::Connection(format!("Failed to connect: {}", e)))?;

        Ok(Self {
            client,
            endpoint,
        })
    }

    /// Reconnect to the application endpoint.
    pub async fn reconnect(&mut self) -> Result<(), DydxAdapterError> {
        self.client = GrpcClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| DydxAdapterError::Connection(format!("Failed to reconnect: {}", e)))?;
        Ok(())
    }

    /// Convert a DydxBlock to a gRPC BuildBlockRequest.
    fn to_build_request(&self, block: &DydxBlock) -> BuildBlockRequest {
        BuildBlockRequest {
            height: block.metadata().round, // Use round as height proxy
            round: block.metadata().round,
            timestamp: block.metadata().timestamp as i64,
            proposer_address: block.metadata().author.0.to_vec(),
            parent_hash: block.metadata().parent_id.0.to_vec(),
            max_bytes: 0, // TODO: configure
            max_gas: 0,   // TODO: configure
            pending_txs_count: 0,
        }
    }

    /// Convert a DydxBlock to a gRPC ExecuteBlockRequest.
    fn to_execute_request(&self, block: &DydxBlock, parent_state_root: Vec<u8>) -> ExecuteBlockRequest {
        ExecuteBlockRequest {
            height: block.metadata().round,
            round: block.metadata().round,
            timestamp: block.metadata().timestamp as i64,
            proposer_address: block.metadata().author.0.to_vec(),
            parent_hash: block.metadata().parent_id.0.to_vec(),
            parent_state_root,
            transactions: block.transactions().iter().map(|tx| tx.data.clone()).collect(),
            aggregated_vote_extensions: vec![],
        }
    }

    /// Convert gRPC response to a DydxLedgerInfo.
    fn to_ledger_info(&self, response: ExecuteBlockResponse, block_id: DydxHash, round: u64) -> DydxLedgerInfo {
        let state_root_array = response.new_state_root;
        let mut state_root_arr = [0u8; 32];
        let len = state_root_array.len().min(32);
        state_root_arr[..len].copy_from_slice(&state_root_array[..len]);

        let commit_info = DydxCommitInfo {
            block_id,
            round,
            epoch: round, // Simplified: use round as epoch proxy
            version: response.tx_results.len() as u64,
            state_root: DydxHash(state_root_arr),
            timestamp: response.tx_results.len() as u64,
        };

        DydxLedgerInfo {
            commit_info,
            accumulated_state: DydxHash(state_root_arr),
        }
    }
}

#[async_trait]
impl<B> StateComputer<B> for DydxStateComputer
where
    B: BlockTrait + Into<DydxBlock> + Send + Sync,
{
    type LedgerInfo = DydxLedgerInfo;
    type Error = DydxAdapterError;

    /// Execute a block and compute the new state.
    async fn execute_block(
        &self,
        block: B,
        parent_ledger_info: Self::LedgerInfo,
    ) -> Result<Self::LedgerInfo, Self::Error> {
        // Convert the block to dYdX type
        let dydx_block: DydxBlock = block.into();
        let block_id = dydx_block.id();

        let request = self.to_execute_request(&dydx_block, parent_ledger_info.accumulated_state.0.to_vec());

        let mut client = self.client.clone();
        let response: ExecuteBlockResponse = client
            .execute_block(request)
            .await
            .map_err(|e| DydxAdapterError::grpc(format!("ExecuteBlock RPC failed: {}", e)))?
            .into_inner();

        // Check if execution was successful
        if response.new_state_root.is_empty() {
            return Err(DydxAdapterError::execution_failed(
                dydx_block.metadata().round,
                "Empty state root returned",
            ));
        }

        let round = dydx_block.metadata().round;

        Ok(self.to_ledger_info(response, block_id, round))
    }

    /// Initialize a new epoch.
    async fn new_epoch(
        &self,
        _epoch: u64,
        ledger_info: Self::LedgerInfo,
    ) -> Result<Self::LedgerInfo, Self::Error> {
        // TODO: Implement actual new_epoch logic when proto is added
        // For now, just return the same ledger info
        Ok(ledger_info)
    }

    /// Sync state to a specific target ledger info.
    async fn sync_to_target(&self, target: Self::LedgerInfo) -> Result<Self::LedgerInfo, Self::Error> {
        // TODO: Implement actual sync logic when proto is added
        Ok(target)
    }

    /// Sync state for a given duration.
    async fn sync_for_duration(&self, _duration_secs: u64) -> Result<Self::LedgerInfo, Self::Error> {
        // TODO: Implement actual sync logic when proto is added
        Err(DydxAdapterError::msg("sync_for_duration not implemented"))
    }

    /// Start a new epoch (synchronous version for notifications).
    fn start_epoch(&self, _epoch: u64) {
        // TODO: Implement epoch start logic
        log::info!("Starting epoch {}", _epoch);
    }

    /// End the current epoch (synchronous version for notifications).
    fn end_epoch(&self) {
        // TODO: Implement epoch end logic
        log::info!("Ending epoch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DydxBlock, DydxHash, DydxNodeId, DydxTransaction};
    use consensus_traits::LedgerInfo;

    /// Helper to test conversion methods without needing a real gRPC connection
    struct TestHelper;
    impl TestHelper {
        /// Test the to_build_request conversion
        pub fn test_to_build_request(block: &DydxBlock) -> BuildBlockRequest {
            BuildBlockRequest {
                height: block.metadata().round,
                round: block.metadata().round,
                timestamp: block.metadata().timestamp as i64,
                proposer_address: block.metadata().author.0.to_vec(),
                parent_hash: block.metadata().parent_id.0.to_vec(),
                max_bytes: 0,
                max_gas: 0,
                pending_txs_count: 0,
            }
        }

        /// Test the to_ledger_info conversion
        pub fn test_to_ledger_info(response: ExecuteBlockResponse, block_id: DydxHash, round: u64) -> DydxLedgerInfo {
            let state_root_array = response.new_state_root;
            let mut state_root_arr = [0u8; 32];
            let len = state_root_array.len().min(32);
            state_root_arr[..len].copy_from_slice(&state_root_array[..len]);

            let commit_info = DydxCommitInfo {
                block_id,
                round,
                epoch: round,
                version: response.tx_results.len() as u64,
                state_root: DydxHash(state_root_arr),
                timestamp: response.tx_results.len() as u64,
            };

            DydxLedgerInfo {
                commit_info,
                accumulated_state: DydxHash(state_root_arr),
            }
        }
    }

    #[test]
    fn test_to_build_request() {
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            1000,
            vec![DydxTransaction::new(vec![1, 2, 3])],
        );

        let request = TestHelper::test_to_build_request(&block);
        assert_eq!(request.height, 5);
        assert_eq!(request.round, 5);
        assert_eq!(request.parent_hash, vec![2u8; 32]);
        assert_eq!(request.proposer_address, vec![1u8; 32]);
    }

    #[test]
    fn test_to_ledger_info() {
        let response = ExecuteBlockResponse {
            new_state_root: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
            tx_results: vec![],
            events: vec![],
            gas_used: 1000,
            validator_updates: vec![],
        };

        let block_id = DydxHash([5u8; 32]);
        let info = TestHelper::test_to_ledger_info(response, block_id, 100);
        assert_eq!(info.commit_info.block_id.0[..4], [5, 5, 5, 5]);
        assert_eq!(info.commit_info.round, 100);
        assert_eq!(info.commit_info.state_root.0[..4], [1, 2, 3, 4]);
    }

    #[test]
    fn test_ledger_info_conversions() {
        use crate::types::DydxCommitInfo;

        let block_id = DydxHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let round = 100;

        let commit_info = DydxCommitInfo {
            block_id,
            round,
            epoch: round,
            version: 5,
            state_root: block_id,
            timestamp: 1000,
        };

        let ledger_info = DydxLedgerInfo {
            commit_info,
            accumulated_state: block_id,
        };

        assert_eq!(ledger_info.commit_info.round, 100);
        assert_eq!(ledger_info.epoch(), 100);
        assert_eq!(ledger_info.accumulated_state, block_id);
    }
}
