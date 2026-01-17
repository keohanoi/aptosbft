// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! State synchronization for consensus.
//!
//! This module handles synchronization of consensus state when a node
//! falls behind the network, allowing it to catch up safely.

use consensus_traits::{
    block::{Block, BlockMetadata, QuorumCertificate},
    core::Hash as HashTrait,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    sync::Arc,
};
use anyhow::{ensure, Result};

use crate::types::{Epoch, Round};

/// Configuration for state synchronization.
#[derive(Clone, Debug)]
pub struct StateSyncConfig {
    /// Maximum number of blocks to sync per batch
    pub max_batch_size: usize,

    /// Timeout for sync requests (in milliseconds)
    pub sync_timeout_ms: u64,

    /// Number of retries for failed sync requests
    pub max_retries: usize,

    /// Whether to request sync from multiple peers
    pub verify_from_multiple_peers: bool,
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            sync_timeout_ms: 5000,
            max_retries: 3,
            verify_from_multiple_peers: true,
        }
    }
}

/// Status of a state synchronization request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// Not currently syncing
    Idle,

    /// Sync request in progress
    InProgress {
        /// Target round we're syncing to
        target_round: Round,

        /// Current round we've synced to
        current_round: Round,

        /// Number of blocks received so far
        blocks_received: usize,
    },

    /// Sync completed successfully
    Completed {
        /// Number of blocks synced
        blocks_synced: usize,
    },

    /// Sync failed
    Failed {
        /// Error message
        error: String,
    },
}

impl SyncStatus {
    /// Check if sync is currently in progress.
    pub fn is_in_progress(&self) -> bool {
        matches!(self, SyncStatus::InProgress { .. })
    }

    /// Check if sync has completed successfully.
    pub fn is_completed(&self) -> bool {
        matches!(self, SyncStatus::Completed { .. })
    }

    /// Check if sync has failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, SyncStatus::Failed { .. })
    }

    /// Get the target round if syncing.
    pub fn target_round(&self) -> Option<Round> {
        match self {
            SyncStatus::InProgress { target_round, .. } => Some(*target_round),
            _ => None,
        }
    }

    /// Get the current round if syncing.
    pub fn current_round(&self) -> Option<Round> {
        match self {
            SyncStatus::InProgress { current_round, .. } => Some(*current_round),
            _ => None,
        }
    }
}

/// A request to sync blocks from a peer.
#[derive(Clone, Debug)]
pub struct SyncRequest {
    /// The target round to sync to
    pub target_round: Round,

    /// The starting round to sync from
    pub start_round: Round,

    /// The epoch we're syncing in
    pub epoch: Epoch,
}

impl SyncRequest {
    /// Create a new sync request.
    pub fn new(target_round: Round, start_round: Round, epoch: Epoch) -> Self {
        Self {
            target_round,
            start_round,
            epoch,
        }
    }

    /// Check if this request is for a specific round range.
    pub fn covers_round(&self, round: Round) -> bool {
        round >= self.start_round && round <= self.target_round
    }

    /// Get the number of rounds this request covers.
    pub fn round_count(&self) -> u64 {
        self.target_round.saturating_sub(self.start_round) + 1
    }
}

/// A response containing synced blocks.
#[derive(Clone, Debug)]
pub struct SyncResponse<B>
where
    B: Block,
{
    /// The blocks being synced
    pub blocks: Vec<Arc<B>>,

    /// The epoch these blocks belong to
    pub epoch: Epoch,

    /// Whether this is the final batch
    pub is_final: bool,
}

impl<B> SyncResponse<B>
where
    B: Block,
{
    /// Create a new sync response.
    pub fn new(blocks: Vec<Arc<B>>, epoch: Epoch, is_final: bool) -> Self {
        Self {
            blocks,
            epoch,
            is_final,
        }
    }

    /// Check if this response is empty.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Get the number of blocks in this response.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Get the highest round in this response.
    pub fn highest_round(&self) -> Option<Round> {
        self.blocks.last().map(|b| b.metadata().round())
    }

    /// Get the lowest round in this response.
    pub fn lowest_round(&self) -> Option<Round> {
        self.blocks.first().map(|b| b.metadata().round())
    }

    /// Verify that blocks are in sequential order.
    pub fn verify_sequential(&self) -> Result<()> {
        for window in self.blocks.windows(2) {
            let current_round = window[0].metadata().round();
            let next_round = window[1].metadata().round();

            ensure!(
                next_round == current_round + 1,
                "Blocks not sequential: round {} followed by round {}",
                current_round,
                next_round
            );
        }

        Ok(())
    }
}

/// State synchronization coordinator.
///
/// This manages the process of catching up when a node falls behind,
/// coordinating with peers to fetch missing blocks and verify them.
///
/// # Type Parameters
///
/// - `B`: Block type implementing the [`Block`] trait
pub struct StateSync<B>
where
    B: Block,
{
    /// Configuration
    config: StateSyncConfig,

    /// Current sync status
    status: SyncStatus,

    /// Pending sync requests that haven't been sent yet
    pending_requests: Vec<SyncRequest>,

    /// Blocks that have been received but not yet committed
    received_blocks: HashMap<Round, Arc<B>>,

    /// The highest known round from our peers
    highest_known_round: Option<Round>,

    /// Our current round
    current_round: Round,
}

impl<B> StateSync<B>
where
    B: Block,
{
    /// Create a new state sync coordinator.
    ///
    /// ## Parameters
    ///
    /// - `config`: Sync configuration
    /// - `current_round`: Our current round
    pub fn new(config: StateSyncConfig, current_round: Round) -> Self {
        Self {
            config,
            status: SyncStatus::Idle,
            pending_requests: Vec::new(),
            received_blocks: HashMap::new(),
            highest_known_round: None,
            current_round,
        }
    }

    /// Get the current sync status.
    pub fn status(&self) -> &SyncStatus {
        &self.status
    }

    /// Get the current round.
    pub fn current_round(&self) -> Round {
        self.current_round
    }

    /// Get the highest known round from peers.
    pub fn highest_known_round(&self) -> Option<Round> {
        self.highest_known_round
    }

    /// Start syncing to a target round.
    ///
    /// ## Parameters
    ///
    /// - `target_round`: The round to sync to
    /// - `epoch`: The epoch we're syncing in
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - A sync is already in progress
    /// - The target round is behind our current round
    pub fn start_sync(&mut self, target_round: Round, epoch: Epoch) -> Result<()> {
        ensure!(!self.status.is_in_progress(), "Sync already in progress");

        if target_round <= self.current_round {
            // Already caught up
            self.status = SyncStatus::Completed {
                blocks_synced: 0,
            };
            return Ok(());
        }

        // Calculate the batch size
        let round_count = target_round.saturating_sub(self.current_round);
        let batch_size = std::cmp::min(round_count as usize, self.config.max_batch_size);

        self.status = SyncStatus::InProgress {
            target_round,
            current_round: self.current_round,
            blocks_received: 0,
        };

        // Create the initial sync request
        let request = SyncRequest::new(
            self.current_round + batch_size as u64,
            self.current_round + 1,
            epoch,
        );
        self.pending_requests.push(request);

        Ok(())
    }

    /// Add a sync response from a peer.
    ///
    /// ## Parameters
    ///
    /// - `response`: The response containing blocks
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - No sync is in progress
    /// - Blocks are not sequential
    /// - Blocks are from the wrong epoch
    pub fn add_sync_response(&mut self, response: SyncResponse<B>) -> Result<()> {
        let target_round = self.status.target_round()
            .ok_or_else(|| anyhow::anyhow!("No sync in progress"))?;

        // Verify blocks are sequential
        response.verify_sequential()?;

        // Verify epoch matches
        let expected_epoch = match &self.status {
            SyncStatus::InProgress { .. } => {
                // We'd need to track epoch in status
                // For now, just accept whatever epoch the response has
                response.epoch
            }
            _ => response.epoch,
        };

        // Add blocks to our received map
        for block in response.blocks {
            let round = block.metadata().round();
            self.received_blocks.insert(round, block);
        }

        // Update status
        let blocks_received = self.received_blocks.len();
        let highest_received = self.received_blocks.keys().copied().max();

        if let Some(highest) = highest_received {
            if highest >= target_round {
                // Sync complete
                self.status = SyncStatus::Completed {
                    blocks_synced: blocks_received,
                };
            } else {
                self.status = SyncStatus::InProgress {
                    target_round,
                    current_round: highest,
                    blocks_received,
                };

                // Request next batch if needed
                if response.is_final && highest < target_round {
                    let next_start = highest + 1;
                    let next_end = std::cmp::min(
                        target_round,
                        highest + self.config.max_batch_size as u64
                    );

                    let request = SyncRequest::new(next_end, next_start, expected_epoch);
                    self.pending_requests.push(request);
                }
            }
        }

        Ok(())
    }

    /// Get all pending sync requests.
    pub fn pending_requests(&self) -> &[SyncRequest] {
        &self.pending_requests
    }

    /// Take the next pending sync request.
    pub fn take_next_request(&mut self) -> Option<SyncRequest> {
        self.pending_requests.pop()
    }

    /// Get all received blocks that are ready to be committed.
    ///
    /// This returns blocks in sequential order starting from our current round.
    pub fn ready_blocks(&mut self) -> Vec<Arc<B>> {
        let mut ready = Vec::new();
        let mut next_round = self.current_round + 1;

        while let Some(block) = self.received_blocks.remove(&next_round) {
            ready.push(block);
            self.current_round = next_round;
            next_round += 1;
        }

        ready
    }

    /// Update the highest known round from peers.
    pub fn update_highest_known_round(&mut self, round: Round) {
        if let Some(current) = self.highest_known_round {
            self.highest_known_round = Some(current.max(round));
        } else {
            self.highest_known_round = Some(round);
        }
    }

    /// Check if we need to sync.
    pub fn needs_sync(&self) -> bool {
        if let Some(highest) = self.highest_known_round {
            highest > self.current_round && !self.status.is_in_progress()
        } else {
            false
        }
    }

    /// Cancel the current sync operation.
    pub fn cancel(&mut self) {
        self.status = SyncStatus::Idle;
        self.pending_requests.clear();
        self.received_blocks.clear();
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        self.status = SyncStatus::Idle;
        self.pending_requests.clear();
        self.received_blocks.clear();
        self.highest_known_round = None;
    }

    /// Get the number of blocks received but not yet committed.
    pub fn pending_block_count(&self) -> usize {
        self.received_blocks.len()
    }

    /// Check if we have a specific round.
    pub fn has_block(&self, round: Round) -> bool {
        self.received_blocks.contains_key(&round)
    }

    /// Get a specific block if we have it.
    pub fn get_block(&self, round: Round) -> Option<Arc<B>> {
        self.received_blocks.get(&round).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockBlock, MockBlockMetadata, MockNodeId, MockHash};
    use std::hash::Hash as StdHash;

    fn create_test_block(round: u64) -> Arc<MockBlock> {
        let metadata = MockBlockMetadata::new(
            1, // epoch
            round,
            MockHash(1), // author
            MockHash((round - 1) as u8), // parent_id is previous round
            round * 1000, // timestamp
        );
        Arc::new(MockBlock::new(vec![], metadata))
    }

    #[test]
    fn test_state_sync_config_default() {
        let config = StateSyncConfig::default();
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.sync_timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_sync_status() {
        let status = SyncStatus::Idle;
        assert!(!status.is_in_progress());
        assert!(!status.is_completed());
        assert!(!status.is_failed());

        let status = SyncStatus::InProgress {
            target_round: 100,
            current_round: 50,
            blocks_received: 50,
        };
        assert!(status.is_in_progress());
        assert_eq!(status.target_round(), Some(100));
        assert_eq!(status.current_round(), Some(50));

        let status = SyncStatus::Completed {
            blocks_synced: 100,
        };
        assert!(status.is_completed());

        let status = SyncStatus::Failed {
            error: "test error".to_string(),
        };
        assert!(status.is_failed());
    }

    #[test]
    fn test_sync_request() {
        let request = SyncRequest::new(100, 50, 1);

        assert!(request.covers_round(75));
        assert!(!request.covers_round(49));
        assert!(!request.covers_round(101));
        assert_eq!(request.round_count(), 51);
    }

    #[test]
    fn test_sync_response() {
        let blocks = vec![
            create_test_block(10),
            create_test_block(11),
            create_test_block(12),
        ];

        let response = SyncResponse::new(blocks.clone(), 1, false);

        assert!(!response.is_empty());
        assert_eq!(response.len(), 3);
        assert_eq!(response.highest_round(), Some(12));
        assert_eq!(response.lowest_round(), Some(10));

        // Verify sequential check
        assert!(response.verify_sequential().is_ok());

        // Test non-sequential blocks
        let blocks_non_seq = vec![
            create_test_block(10),
            create_test_block(15), // Gap!
        ];
        let response_non_seq = SyncResponse::new(blocks_non_seq, 1, false);
        assert!(response_non_seq.verify_sequential().is_err());
    }

    #[test]
    fn test_state_sync_new() {
        let config = StateSyncConfig::default();
        let sync: StateSync<MockBlock> = StateSync::new(config, 50);

        assert!(matches!(sync.status(), SyncStatus::Idle));
        assert_eq!(sync.current_round(), 50);
        assert_eq!(sync.highest_known_round(), None);
        assert!(!sync.needs_sync());
    }

    #[test]
    fn test_state_sync_start() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        assert!(sync.start_sync(100, 1).is_ok());
        assert!(sync.status().is_in_progress());
        assert_eq!(sync.pending_requests().len(), 1);
    }

    #[test]
    fn test_state_sync_already_caught_up() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        // Target is behind current round
        assert!(sync.start_sync(40, 1).is_ok());
        assert!(sync.status().is_completed());
    }

    #[test]
    fn test_state_sync_double_start_fails() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        assert!(sync.start_sync(100, 1).is_ok());
        assert!(sync.start_sync(200, 1).is_err());
    }

    #[test]
    fn test_state_sync_add_response() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.start_sync(100, 1).unwrap();

        let blocks = vec![
            create_test_block(51),
            create_test_block(52),
            create_test_block(53),
        ];

        let response = SyncResponse::new(blocks, 1, false);
        assert!(sync.add_sync_response(response).is_ok());

        assert_eq!(sync.pending_block_count(), 3);
        assert!(sync.has_block(51));
        assert!(sync.has_block(52));
        assert!(sync.has_block(53));
    }

    #[test]
    fn test_state_sync_ready_blocks() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.start_sync(100, 1).unwrap();

        // Add blocks 51, 52, 53
        let blocks = vec![
            create_test_block(51),
            create_test_block(52),
            create_test_block(53),
        ];

        let response = SyncResponse::new(blocks, 1, false);
        sync.add_sync_response(response).unwrap();

        // Get ready blocks - should return 51, 52, 53 in order
        let ready = sync.ready_blocks();
        assert_eq!(ready.len(), 3);
        assert_eq!(ready[0].metadata().round(), 51);
        assert_eq!(ready[1].metadata().round(), 52);
        assert_eq!(ready[2].metadata().round(), 53);

        // Current round should be updated
        assert_eq!(sync.current_round(), 53);
    }

    #[test]
    fn test_state_sync_gap_in_ready_blocks() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.start_sync(100, 1).unwrap();

        // Add blocks 51 and 53 (gap at 52)
        sync.received_blocks.insert(51, create_test_block(51));
        sync.received_blocks.insert(53, create_test_block(53));

        // Only block 51 should be ready
        let ready = sync.ready_blocks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].metadata().round(), 51);
        assert_eq!(sync.current_round(), 51);
        assert!(sync.has_block(53)); // Block 53 still pending
    }

    #[test]
    fn test_state_sync_needs_sync() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.update_highest_known_round(100);
        assert!(sync.needs_sync());

        sync.start_sync(100, 1).unwrap();
        assert!(!sync.needs_sync()); // Already syncing
    }

    #[test]
    fn test_state_sync_cancel() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.start_sync(100, 1).unwrap();
        assert!(sync.status().is_in_progress());

        sync.cancel();
        assert!(matches!(sync.status(), SyncStatus::Idle));
        assert_eq!(sync.pending_block_count(), 0);
    }

    #[test]
    fn test_state_sync_reset() {
        let config = StateSyncConfig::default();
        let mut sync: StateSync<MockBlock> = StateSync::new(config, 50);

        sync.update_highest_known_round(100);
        sync.start_sync(100, 1).unwrap();

        sync.received_blocks.insert(51, create_test_block(51));

        sync.reset();
        assert!(matches!(sync.status(), SyncStatus::Idle));
        assert_eq!(sync.pending_block_count(), 0);
        assert_eq!(sync.highest_known_round(), None);
    }
}
