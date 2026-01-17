// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Storage and state traits for consensus.
//!
//! This module defines traits for persistent storage and state computation.

use crate::block::{Block, LedgerInfo, QuorumCertificate};
use crate::core::Hash;
use crate::safety::SafetyState;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

/// Persistent storage for consensus data.
///
/// Handles storing blocks, votes, and quorum certificates.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Asynchronous (using async/await)
/// - Durable (persists across restarts)
///
/// # Example
///
/// ```text
/// use consensus_traits::storage::{ConsensusStorage, QuorumCertificate};
/// use consensus_traits::block::Block;
/// use async_trait::async_trait;
///
/// struct MyStorage;
///
/// #[async_trait]
/// impl<B, QC> ConsensusStorage<B, QC> for MyStorage
/// where
///     B: Block + Send + Sync,
///     QC: QuorumCertificate + Send + Sync,
/// {
///     type Error = anyhow::Error;
///     type Hash = [u8; 32];
///
///     async fn save_block(&mut self, block: B) -> Result<(), Self::Error> {
///         // Save block to database
///         Ok(())
///     }
///
///     async fn get_block(&self, id: [u8; 32]) -> Result<Option<B>, Self::Error> {
///         // Retrieve block from database
///         Ok(None)
///     }
///
///     // ... other methods
/// }
/// ```text
#[async_trait]
pub trait ConsensusStorage<B, QC>: Send + Sync
where
    B: Block,
    QC: QuorumCertificate,
{
    /// Error type for storage operations.
    type Error: std::error::Error + Send + Sync;

    /// The hash type used for block IDs.
    type Hash: Hash;

    /// Save a block to storage.
    async fn save_block(&mut self, block: B) -> Result<(), Self::Error>;

    /// Retrieve a block by ID.
    async fn get_block(&self, id: Self::Hash) -> Result<Option<B>, Self::Error>;

    /// Save a quorum certificate.
    async fn save_quorum_cert(&mut self, qc: QC) -> Result<(), Self::Error>;

    /// Retrieve a quorum certificate by block ID.
    async fn get_quorum_cert(&self, block_id: Self::Hash) -> Result<Option<QC>, Self::Error>;

    /// Prune old blocks and certificates.
    ///
    /// Removes blocks older than the given depth from the current height.
    async fn prune(&mut self, retain_depth: u64) -> Result<(), Self::Error>;

    // Safety State Storage Methods
    // ========================================

    /// Save the safety state to persistent storage.
    ///
    /// This MUST be called before voting to ensure the safety state is persisted.
    /// If a validator restarts, it loads the safety state to prevent double-voting.
    ///
    /// # Safety
    ///
    /// The safety state MUST be persisted atomically before any vote is sent.
    /// This prevents the validator from voting for the same round twice after a restart.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be persisted.
    async fn save_safety_state<S>(&mut self, state: S) -> Result<(), Self::Error>
    where
        S: SafetyState + Serialize + Send + Sync;

    /// Load the safety state from persistent storage.
    ///
    /// Returns None if no safety state has been persisted yet (e.g., first startup).
    ///
    /// # Safety
    ///
    /// This MUST be called on startup before any consensus operations begin.
    /// The loaded state is used to initialize the SafetyRules.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be loaded or is corrupted.
    async fn get_safety_state<S>(&self) -> Result<Option<S>, Self::Error>
    where
        S: SafetyState + DeserializeOwned + Send + Sync;
}

/// State computer interface for executing blocks.
///
/// This is the bridge between consensus and execution layers.
/// The consensus layer uses this to execute blocks and compute state roots.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Asynchronous (using async/await)
///
/// # Example
///
/// ```text
/// use consensus_traits::storage::{StateComputer, Block};
/// use async_trait::async_trait;
///
/// struct MyStateComputer;
///
/// #[async_trait]
/// impl<B> StateComputer<B> for MyStateComputer
/// where
///     B: Block + Send + Sync,
/// {
///     type LedgerInfo = MyLedgerInfo;
///     type Error = anyhow::Error;
///
///     async fn execute_block(
///         &self,
///         block: B,
///         parent_ledger_info: MyLedgerInfo,
///     ) -> Result<MyLedgerInfo, Self::Error> {
///         // Execute transactions and compute new state root
///         Ok(MyLedgerInfo::default())
///     }
///
///     async fn new_epoch(
///         &self,
///         epoch: u64,
///         ledger_info: MyLedgerInfo,
///     ) -> Result<MyLedgerInfo, Self::Error> {
///         // Initialize new epoch
///         Ok(ledger_info)
///     }
///
///     async fn sync_to_target(
///         &self,
///         target: MyLedgerInfo,
///     ) -> Result<MyLedgerInfo, Self::Error> {
///         // Sync state to target
///         Ok(target)
///     }
///
///     async fn sync_for_duration(
///         &self,
///         duration_secs: u64,
///     ) -> Result<MyLedgerInfo, Self::Error> {
///         // Sync state for given duration
///         unimplemented!()
///     }
///
///     fn start_epoch(&self, epoch: u64) {
///         // Start epoch logic
///     }
///
///     fn end_epoch(&self) {
///         // End epoch logic
///     }
/// }
/// ```text
#[async_trait]
pub trait StateComputer<B>: Send + Sync
where
    B: Block,
{
    /// The ledger info type.
    type LedgerInfo: LedgerInfo;

    /// Error type for state computation.
    type Error: std::error::Error + Send + Sync;

    /// Execute a block and compute the new state.
    ///
    /// This should execute all transactions in the block and return
    /// the resulting ledger info with the new state root.
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails.
    async fn execute_block(
        &self,
        block: B,
        parent_ledger_info: Self::LedgerInfo,
    ) -> Result<Self::LedgerInfo, Self::Error>;

    /// Initialize a new epoch.
    ///
    /// # Errors
    ///
    /// Returns an error if epoch initialization fails.
    async fn new_epoch(
        &self,
        epoch: u64,
        ledger_info: Self::LedgerInfo,
    ) -> Result<Self::LedgerInfo, Self::Error>;

    /// Sync state to a specific target ledger info.
    ///
    /// # Errors
    ///
    /// Returns an error if sync fails.
    async fn sync_to_target(&self, target: Self::LedgerInfo) -> Result<Self::LedgerInfo, Self::Error>;

    /// Sync state for a given duration.
    ///
    /// # Errors
    ///
    /// Returns an error if sync fails.
    async fn sync_for_duration(&self, duration_secs: u64) -> Result<Self::LedgerInfo, Self::Error>;

    /// Start a new epoch (synchronous version for notifications).
    fn start_epoch(&self, epoch: u64);

    /// End the current epoch (synchronous version for cleanup).
    fn end_epoch(&self);
}

/// State view for reading blockchain state.
///
/// This provides read access to the current state during execution.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
pub trait StateView: Send + Sync {
    /// Error type for state access.
    type Error: std::error::Error + Send + Sync;

    /// Get a value from state by key.
    ///
    /// # Errors
    ///
    /// Returns an error if the key doesn't exist or access fails.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Get multiple values from state.
    ///
    /// # Errors
    ///
    /// Returns an error if any access fails.
    fn get_multi(&self, keys: &[Vec<u8>]) -> Result<Vec<Option<Vec<u8>>>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct MockError(String);

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for MockError {}

    struct MockStateView;

    impl StateView for MockStateView {
        type Error = MockError;

        fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }

        fn get_multi(&self, keys: &[Vec<u8>]) -> Result<Vec<Option<Vec<u8>>>, Self::Error> {
            Ok(vec![None; keys.len()])
        }
    }

    #[test]
    fn test_state_view() {
        let view = MockStateView;
        assert!(view.get(b"test").unwrap().is_none());
        assert_eq!(view.get_multi(&[vec![1], vec![2]]).unwrap().len(), 2);
    }
}
