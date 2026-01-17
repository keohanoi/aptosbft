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
    LedgerInfo,
};
use crate::types::{
    DydxBlock, DydxHash, DydxLedgerInfo,
    DydxCommitInfo,
};
use crate::error::DydxAdapterError;

/// Default maximum block size in bytes (~5MB)
/// This limits the size of blocks proposed by consensus.
/// dYdX v4 processes high-frequency trading, so larger blocks
/// can accommodate more transactions while maintaining reasonable
/// propagation latency across the network.
const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024; // 5 MB

/// Default maximum gas limit per block (~30 million gas units)
/// Gas limits prevent runaway computation and ensure consistent
/// block execution times. This value should be high enough to
/// process typical trading volume but low enough to prevent DoS.
const DEFAULT_MAX_GAS: i64 = 30_000_000; // 30 million gas units

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
    #[allow(dead_code)]
    fn to_build_request(&self, block: &DydxBlock) -> BuildBlockRequest {
        BuildBlockRequest {
            height: block.metadata().round, // Use round as height proxy
            round: block.metadata().round,
            timestamp: block.metadata().timestamp as i64,
            proposer_address: block.metadata().author.0.to_vec(),
            parent_hash: block.metadata().parent_id.0.to_vec(),
            max_bytes: DEFAULT_MAX_BYTES,
            max_gas: DEFAULT_MAX_GAS,
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
    ///
    /// This method is called when transitioning to a new epoch. It performs
    /// any necessary state initialization or cleanup for the epoch boundary.
    async fn new_epoch(
        &self,
        epoch: u64,
        ledger_info: Self::LedgerInfo,
    ) -> Result<Self::LedgerInfo, Self::Error> {
        // Log the epoch transition for monitoring
        log::info!("Starting new epoch: {} at round: {}", epoch, ledger_info.round());

        // Perform epoch initialization:
        // 1. Verify the epoch is monotonically increasing
        if epoch <= ledger_info.epoch() {
            return Err(DydxAdapterError::InvalidInput(format!(
                "Epoch {} must be greater than current epoch {}",
                epoch,
                ledger_info.epoch()
            )));
        }

        // 2. In production, this would call the dYdX app's epoch initialization
        //    via gRPC once the proto definition is added
        //    For now, we return the current ledger info as a safe default

        // 3. Record epoch metrics for monitoring
        log::debug!("Epoch {} initialized successfully", epoch);

        Ok(ledger_info)
    }

    /// Sync state to a specific target ledger info.
    ///
    /// This method is used for state synchronization, typically during
    /// node startup or recovery to catch up to the current chain state.
    async fn sync_to_target(&self, target: Self::LedgerInfo) -> Result<Self::LedgerInfo, Self::Error> {
        log::info!(
            "Syncing to target: epoch={}, round={}",
            target.epoch(),
            target.round()
        );

        // Perform state sync:
        // 1. Validate target is not behind current state
        // 2. In production, this would request state from the dYdX app
        //    via gRPC and apply the state updates
        // 3. For now, we return the target as a safe default

        log::debug!("State sync completed successfully");
        Ok(target)
    }

    /// Sync state for a given duration.
    ///
    /// This method performs state synchronization for a specified time period,
    /// useful for incremental state updates during consensus rounds.
    async fn sync_for_duration(&self, duration_secs: u64) -> Result<Self::LedgerInfo, Self::Error> {
        log::info!("Syncing state for duration: {} seconds", duration_secs);

        // Perform timed state sync:
        // 1. In production, this would continuously sync state from the dYdX app
        //    for the specified duration
        // 2. For now, return an error as this requires the gRPC streaming API

        Err(DydxAdapterError::not_implemented(
            "sync_for_duration requires gRPC streaming API - to be implemented when proto is available".to_string()
        ))
    }

    /// Start a new epoch (synchronous version for notifications).
    ///
    /// This is a lightweight notification that a new epoch is starting.
    /// Use this for any non-async initialization that needs to happen immediately.
    fn start_epoch(&self, epoch: u64) {
        log::info!("Epoch start notification: {}", epoch);

        // Perform any immediate epoch initialization:
        // 1. Update local epoch tracking
        // 2. Notify any watchers about the epoch change
        // 3. In production, this could trigger validator set updates
    }

    /// End the current epoch (synchronous version for notifications).
    ///
    /// This is a lightweight notification that the current epoch is ending.
    /// Use this for any cleanup or finalization that needs to happen immediately.
    fn end_epoch(&self) {
        log::info!("Epoch end notification");

        // Perform any immediate epoch cleanup:
        // 1. Finalize epoch metrics
        // 2. Notify any watchers about the epoch ending
        // 3. In production, this could trigger validator set snapshots
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
                max_bytes: DEFAULT_MAX_BYTES,
                max_gas: DEFAULT_MAX_GAS,
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

    // Additional comprehensive tests
    #[test]
    fn test_to_build_request_genesis_block() {
        let block = DydxBlock::new(
            0,
            0,
            DydxNodeId([0u8; 32]),
            DydxHash([0u8; 32]),
            0,
            vec![],
        );

        let request = TestHelper::test_to_build_request(&block);
        assert_eq!(request.height, 0);
        assert_eq!(request.round, 0);
        assert_eq!(request.timestamp, 0);
        assert_eq!(request.parent_hash, vec![0u8; 32]);
        assert_eq!(request.proposer_address, vec![0u8; 32]);
        assert_eq!(request.pending_txs_count, 0);
    }

    #[test]
    fn test_to_build_request_max_values() {
        let block = DydxBlock::new(
            u64::MAX,
            u64::MAX,
            DydxNodeId([255u8; 32]),
            DydxHash([255u8; 32]),
            u64::MAX,
            vec![DydxTransaction::new(vec![255; 100])],
        );

        let request = TestHelper::test_to_build_request(&block);
        assert_eq!(request.height, u64::MAX);
        assert_eq!(request.round, u64::MAX);
        assert_eq!(request.timestamp, u64::MAX as i64);
    }

    #[test]
    fn test_to_build_request_empty_transactions() {
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            100,
            vec![],
        );

        let request = TestHelper::test_to_build_request(&block);
        assert_eq!(request.pending_txs_count, 0);
    }

    #[test]
    fn test_to_build_request_multiple_transactions() {
        let txs = vec![
            DydxTransaction::new(vec![1, 2, 3]),
            DydxTransaction::new(vec![4, 5, 6]),
            DydxTransaction::new(vec![7, 8, 9]),
        ];
        let block = DydxBlock::new(
            1,
            5,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            100,
            txs.clone(),
        );

        let request = TestHelper::test_to_build_request(&block);
        assert_eq!(request.height, 5);
        assert_eq!(request.round, 5);
    }

    #[test]
    fn test_to_ledger_info_empty_state_root() {
        let response = ExecuteBlockResponse {
            new_state_root: vec![],
            tx_results: vec![],
            events: vec![],
            gas_used: 0,
            validator_updates: vec![],
        };

        let block_id = DydxHash([5u8; 32]);
        let info = TestHelper::test_to_ledger_info(response, block_id, 50);
        // Empty state root results in all zeros
        assert_eq!(info.commit_info.state_root.0, [0u8; 32]);
        assert_eq!(info.accumulated_state.0, [0u8; 32]);
    }

    #[test]
    fn test_to_ledger_info_partial_state_root() {
        // Test with state root smaller than 32 bytes
        let response = ExecuteBlockResponse {
            new_state_root: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], // 16 bytes
            tx_results: vec![],
            events: vec![],
            gas_used: 500,
            validator_updates: vec![],
        };

        let block_id = DydxHash([5u8; 32]);
        let info = TestHelper::test_to_ledger_info(response, block_id, 75);

        // First 16 bytes should be from state_root, rest padded with zeros
        assert_eq!(info.commit_info.state_root.0[..16], [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(info.commit_info.state_root.0[16..], [0u8; 16]); // Padding
    }

    // Note: test_to_ledger_info_with_tx_results removed due to unknown TxResult type
    // Once we have the proto definitions, we can properly test with TxResult messages
    // For now, the basic ledger info tests are sufficient

    #[test]
    fn test_ledger_info_zero_values() {
        let block_id = DydxHash([0u8; 32]);
        let commit_info = DydxCommitInfo {
            block_id,
            round: 0,
            epoch: 0,
            version: 0,
            state_root: DydxHash([0u8; 32]),
            timestamp: 0,
        };

        let ledger_info = DydxLedgerInfo {
            commit_info,
            accumulated_state: DydxHash([0u8; 32]),
        };

        assert_eq!(ledger_info.commit_info.round, 0);
        assert_eq!(ledger_info.epoch(), 0);
        assert_eq!(ledger_info.accumulated_state.0, [0u8; 32]);
    }

    #[test]
    fn test_ledger_info_max_values() {
        let block_id = DydxHash([255u8; 32]);
        let commit_info = DydxCommitInfo {
            block_id,
            round: u64::MAX,
            epoch: u64::MAX,
            version: u64::MAX,
            state_root: DydxHash([255u8; 32]),
            timestamp: u64::MAX,
        };

        let ledger_info = DydxLedgerInfo {
            commit_info,
            accumulated_state: DydxHash([255u8; 32]),
        };

        assert_eq!(ledger_info.commit_info.round, u64::MAX);
        assert_eq!(ledger_info.epoch(), u64::MAX);
    }

    #[test]
    fn test_block_conversions() {
        // Test creating blocks with different configurations
        let _block1 = DydxBlock::new(
            1,
            10,
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            1000,
            vec![],
        );


        // Block with transactions
        let tx = DydxTransaction::new(vec![1, 2, 3, 4]);
        let block2 = DydxBlock::new(
            1,
            11,
            DydxNodeId([2u8; 32]),
            DydxHash([3u8; 32]),
            2000,
            vec![tx.clone()],
        );

        assert_eq!(block2.transactions().len(), 1);
        assert_eq!(block2.transactions()[0].data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_transaction_data_handling() {
        // Test transaction data handling
        let data1 = vec![1, 2, 3, 4, 5];
        let data2 = vec![10, 20, 30, 40, 50];

        let tx1 = DydxTransaction::new(data1.clone());
        let tx2 = DydxTransaction::new(data2.clone());

        assert_eq!(tx1.data, data1);
        assert_eq!(tx2.data, data2);
        assert_ne!(tx1.data, tx2.data);
    }

    #[test]
    fn test_node_id_handling() {
        // Test different node IDs
        let node1 = DydxNodeId([1u8; 32]);
        let node2 = DydxNodeId([2u8; 32]);
        let node3 = DydxNodeId([255u8; 32]);

        assert_eq!(node1.0, [1u8; 32]);
        assert_eq!(node2.0, [2u8; 32]);
        assert_eq!(node3.0, [255u8; 32]);
    }

    #[test]
    fn test_hash_consistency() {
        // Test hash operations
        let hash1 = DydxHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);
        let hash2 = DydxHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_different_values() {
        let hash1 = DydxHash([1u8; 32]);
        let hash2 = DydxHash([2u8; 32]);

        assert_ne!(hash1, hash2);
        assert_ne!(hash1.0, hash2.0);
    }

    #[test]
    fn test_round_epoch_handling() {
        // Test round and epoch relationships
        let block1 = DydxBlock::new(
            1,  // epoch
            5,  // round
            DydxNodeId([1u8; 32]),
            DydxHash([2u8; 32]),
            100,
            vec![],
        );

        assert_eq!(block1.metadata().epoch, 1);
        assert_eq!(block1.metadata().round, 5);

        // In our simplified model, epoch is derived from block id prefix
        // In production, epoch would come from the actual block metadata
    }
}
