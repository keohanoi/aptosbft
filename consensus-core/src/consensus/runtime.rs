// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Consensus runtime for async execution.
//!
//! This module provides a runtime for executing consensus operations asynchronously,
//! managing background tasks and coordinating with the network layer.

use consensus_traits::{
    block::{Block, Vote, BlockMetadata, QuorumCertificate, VoteData},
    proposer::ProposerElection,
};
use std::{
    fmt::{self, Debug, Display},
    sync::Arc,
    time::Duration,
};
use anyhow::Result;

use super::{Consensus, ConsensusState};
use crate::types::{Epoch, Round};

/// Configuration for the consensus runtime.
///
/// This configuration controls the execution environment for consensus.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Interval for processing pending items (in milliseconds)
    pub process_interval_ms: u64,

    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,

    /// Timeout for network operations (in milliseconds)
    pub network_timeout_ms: u64,

    /// Buffer size for network messages
    pub network_buffer_size: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            process_interval_ms: 100,
            max_concurrent_tasks: 10,
            network_timeout_ms: 5000,
            network_buffer_size: 1024 * 1024, // 1MB
        }
    }
}

impl RuntimeConfig {
    /// Create a new runtime configuration.
    pub fn new(
        process_interval_ms: u64,
        max_concurrent_tasks: usize,
        network_timeout_ms: u64,
        network_buffer_size: usize,
    ) -> Self {
        Self {
            process_interval_ms,
            max_concurrent_tasks,
            network_timeout_ms,
            network_buffer_size,
        }
    }

    /// Get the process interval as a Duration.
    pub fn process_interval(&self) -> Duration {
        Duration::from_millis(self.process_interval_ms)
    }

    /// Get the network timeout as a Duration.
    pub fn network_timeout(&self) -> Duration {
        Duration::from_millis(self.network_timeout_ms)
    }
}

/// Result of a consensus operation.
///
/// This type represents the outcome of executing a consensus-related task.
#[derive(Clone, Debug)]
pub enum ExecutionResult {
    /// Operation completed successfully
    Success,

    /// Operation failed with an error message
    Failed {
        /// Error message describing the failure
        message: String,
    },

    /// Operation timed out
    Timeout,

    /// Operation was skipped
    Skipped {
        /// Reason for skipping
        reason: String,
    },
}

impl ExecutionResult {
    /// Check if the result indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success)
    }

    /// Create a success result.
    pub fn success() -> Self {
        ExecutionResult::Success
    }

    /// Create a failed result.
    pub fn failed(message: impl Into<String>) -> Self {
        ExecutionResult::Failed {
            message: message.into(),
        }
    }

    /// Create a timeout result.
    pub fn timeout() -> Self {
        ExecutionResult::Timeout
    }

    /// Create a skipped result.
    pub fn skipped(reason: impl Into<String>) -> Self {
        ExecutionResult::Skipped {
            reason: reason.into(),
        }
    }
}

/// Network message types for consensus.
///
/// These are the messages that can be sent and received through the network.
#[derive(Clone, Debug)]
pub enum NetworkMessage<B, V>
where
    B: Block,
    V: Vote<Block = B>,
    <B::Metadata as BlockMetadata>::QuorumCert: Clone + std::fmt::Debug,
{
    /// Block proposal from the current proposer
    Proposal {
        /// The proposed block
        block: Arc<B>,
    },

    /// Vote for a proposed block
    Vote {
        /// The vote
        vote: Arc<V>,
    },

    /// Quorum certificate proving block commitment
    QuorumCertificate {
        /// The quorum certificate
        qc: Arc<<B::Metadata as BlockMetadata>::QuorumCert>,
    },

    /// Round timeout signal
    Timeout {
        /// The round that timed out
        round: Round,
    },

    /// State synchronization request
    SyncRequest {
        /// The target round
        target_round: Round,
    },

    /// State synchronization response
    SyncResponse {
        /// Blocks being synced
        blocks: Vec<Arc<B>>,
    },
}

impl<B, V> fmt::Display for NetworkMessage<B, V>
where
    B: Block,
    V: Vote<Block = B>,
    <B::Metadata as BlockMetadata>::QuorumCert: Clone + std::fmt::Debug,
    <V as Vote>::VoteData: VoteData,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMessage::Proposal { block } => {
                write!(f, "Proposal(round={})", block.metadata().round())
            }
            NetworkMessage::Vote { vote } => {
                write!(f, "Vote(round={})", vote.vote_data().proposed_block().round())
            }
            NetworkMessage::QuorumCertificate { qc } => {
                write!(f, "QC(round={})", qc.round())
            }
            NetworkMessage::Timeout { round } => {
                write!(f, "Timeout(round={})", round)
            }
            NetworkMessage::SyncRequest { target_round } => {
                write!(f, "SyncRequest(round={})", target_round)
            }
            NetworkMessage::SyncResponse { blocks } => {
                write!(f, "SyncResponse(count={})", blocks.len())
            }
        }
    }
}

/// Consensus runtime for async execution.
///
/// This runtime manages the execution context for consensus operations,
/// providing async task management and network coordination.
///
/// # Type Parameters
///
/// - `B`: Block type - must implement the [`Block`] trait
/// - `V`: Vote type - must implement the [`Vote`] trait
/// - `PE`: ProposerElection type - must implement the [`ProposerElection`] trait
/// - `N`: Network type (optional - can be any type, will be used where ConsensusNetwork is needed)
///
/// # Architecture
///
/// The runtime provides:
/// - Async task spawning and coordination
/// - Network message handling
/// - Background job processing
/// - Graceful shutdown coordination
pub struct ConsensusRuntime<B, V, PE, N = ()>
where
    B: Block + 'static,
    V: Vote<Block = B> + 'static,
    PE: ProposerElection<Round = u64> + Send + Sync + Clone + 'static,
    PE::Author: Clone + Debug + Display + Send + Sync + 'static,
    <B::Metadata as BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// The underlying consensus engine
    consensus: Consensus<B, V, PE>,

    /// Runtime configuration
    config: RuntimeConfig,

    /// P2P network interface (optional - if None, network is disabled)
    network: Option<Arc<N>>,

    /// Pending network messages to process
    pending_messages: VecDeque<NetworkMessage<B, V>>,

    /// Background tasks that are running
    active_tasks: Vec<u64>,

    /// Next task ID
    next_task_id: u64,

    /// Whether the runtime is running
    is_running: bool,
}

use std::collections::VecDeque;

impl<B, V, PE, N> ConsensusRuntime<B, V, PE, N>
where
    B: Block + 'static,
    V: Vote<Block = B> + 'static,
    PE: ProposerElection<Round = u64> + Send + Sync + Clone + 'static,
    PE::Author: Clone + Debug + Display + Send + Sync + 'static,
    <B::Metadata as BlockMetadata>::QuorumCert: std::fmt::Debug,
    N: Send + Sync + 'static,
{
    /// Create a new consensus runtime.
    ///
    /// ## Parameters
    ///
    /// - `consensus`: The underlying consensus engine
    /// - `config`: Runtime configuration
    /// - `network`: Optional P2P network interface
    pub fn new(consensus: Consensus<B, V, PE>, config: RuntimeConfig, network: Option<Arc<N>>) -> Self {
        Self {
            consensus,
            config,
            network,
            pending_messages: VecDeque::new(),
            active_tasks: Vec::new(),
            next_task_id: 0,
            is_running: false,
        }
    }

    /// Get the underlying consensus engine.
    pub fn consensus(&self) -> &Consensus<B, V, PE> {
        &self.consensus
    }

    /// Get a mutable reference to the underlying consensus engine.
    pub fn consensus_mut(&mut self) -> &mut Consensus<B, V, PE> {
        &mut self.consensus
    }

    /// Get the runtime configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Check if the runtime is running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Add a network message to be processed.
    ///
    /// This queues the message for processing by the runtime.
    pub fn add_network_message(&mut self, message: NetworkMessage<B, V>) {
        self.pending_messages.push_back(message);
    }

    /// Get the number of pending messages.
    pub fn pending_message_count(&self) -> usize {
        self.pending_messages.len()
    }

    /// Process all pending network messages.
    ///
    /// This method processes each queued message through the consensus engine.
    ///
    /// ## Returns
    ///
    /// A vector of execution results for each processed message.
    pub fn process_pending_messages(&mut self) -> Vec<(NetworkMessage<B, V>, ExecutionResult)> {
        let mut results = Vec::new();

        while let Some(message) = self.pending_messages.pop_front() {
            let result = match &message {
                NetworkMessage::Proposal { block } => {
                    // Process proposal through consensus
                    // Note: We'd need a validator verifier here
                    // For now, just acknowledge receipt
                    println!("Received proposal: {}", message);
                    ExecutionResult::success()
                }

                NetworkMessage::Vote { vote } => {
                    // Process vote through consensus
                    println!("Received vote: {}", message);
                    // Note: We'd need a validator verifier here
                    ExecutionResult::success()
                }

                NetworkMessage::QuorumCertificate { qc } => {
                    // Process QC through consensus
                    match self.consensus.process_qc(qc.clone()) {
                        Ok(()) => ExecutionResult::success(),
                        Err(e) => ExecutionResult::failed(e.to_string()),
                    }
                }

                NetworkMessage::Timeout { round } => {
                    // Process timeout through consensus
                    match self.consensus.process_timeout(*round) {
                        Ok(()) => ExecutionResult::success(),
                        Err(e) => ExecutionResult::failed(e.to_string()),
                    }
                }

                NetworkMessage::SyncRequest { .. } => {
                    // TODO: Implement state sync
                    ExecutionResult::skipped("State sync not implemented")
                }

                NetworkMessage::SyncResponse { .. } => {
                    // TODO: Implement state sync
                    ExecutionResult::skipped("State sync not implemented")
                }
            };

            results.push((message, result));
        }

        results
    }

    /// Execute a block through the consensus state machine.
    ///
    /// This method processes a block through the consensus pipeline.
    ///
    /// ## Parameters
    ///
    /// - `block`: The block to execute
    ///
    /// ## Returns
    ///
    /// An execution result indicating success or failure.
    pub fn execute_block(&mut self, block: Arc<B>) -> ExecutionResult {
        // TODO: Implement block execution
        // This would involve:
        // 1. Validating the block
        // 2. Executing transactions
        // 3. Updating state

        println!("Executing block in round {}", block.metadata().round());
        ExecutionResult::success()
    }

    /// Broadcast a proposal to the network.
    ///
    /// ## Parameters
    ///
    /// - `proposal`: The proposal to broadcast
    ///
    /// ## Returns
    ///
    /// An execution result indicating success or failure.
    pub fn broadcast_proposal(&self, proposal: Arc<B>) -> ExecutionResult {
        // TODO: Implement network broadcast
        // This would involve:
        // 1. Serializing the proposal
        // 2. Sending to network peers
        // 3. Handling send errors

        println!("Broadcasting proposal for round {}", proposal.metadata().round());
        ExecutionResult::success()
    }

    /// Broadcast a vote to the network.
    ///
    /// ## Parameters
    ///
    /// - `vote`: The vote to broadcast
    ///
    /// ## Returns
    ///
    /// An execution result indicating success or failure.
    pub fn broadcast_vote(&self, vote: Arc<V>) -> ExecutionResult {
        // TODO: Implement network broadcast
        // This would involve:
        // 1. Serializing the vote
        // 2. Sending to network peers
        // 3. Handling send errors

        println!("Broadcasting vote for round {}", vote.round());
        ExecutionResult::success()
    }

    /// Start the runtime.
    ///
    /// This starts the runtime and begins processing messages.
    ///
    /// ## Returns
    ///
    /// An error if the runtime is already running.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            anyhow::bail!("Runtime is already running");
        }

        self.is_running = true;

        // Start the underlying consensus engine
        self.consensus.start()?;

        println!("Consensus runtime started");
        Ok(())
    }

    /// Shutdown the runtime.
    ///
    /// This gracefully shuts down the runtime.
    ///
    /// ## Returns
    ///
    /// An error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.is_running {
            anyhow::bail!("Runtime is not running");
        }

        self.is_running = false;

        // Stop the underlying consensus engine
        self.consensus.stop()?;

        // Clear pending messages
        self.pending_messages.clear();

        // Note: In a real async runtime, we would wait for active tasks
        self.active_tasks.clear();

        println!("Consensus runtime stopped");
        Ok(())
    }

    /// Process a single network message.
    ///
    /// This is a convenience method for processing one message at a time.
    ///
    /// ## Parameters
    ///
    /// - `message`: The network message to process
    ///
    /// ## Returns
    ///
    /// An execution result indicating the outcome.
    pub fn handle_network_message(&mut self, message: NetworkMessage<B, V>) -> ExecutionResult {
        match &message {
            NetworkMessage::Proposal { block } => {
                println!("Processing proposal: round={}", block.metadata().round());
                // TODO: Process through consensus with verifier
                ExecutionResult::success()
            }

            NetworkMessage::Vote { vote } => {
                println!("Processing vote: round={}", vote.round());
                // TODO: Process through consensus with verifier
                ExecutionResult::success()
            }

            NetworkMessage::QuorumCertificate { qc } => {
                println!("Processing QC: round={}", qc.round());
                match self.consensus.process_qc(qc.clone()) {
                    Ok(()) => ExecutionResult::success(),
                    Err(e) => ExecutionResult::failed(e.to_string()),
                }
            }

            NetworkMessage::Timeout { round } => {
                println!("Processing timeout: round={}", round);
                match self.consensus.process_timeout(*round) {
                    Ok(()) => ExecutionResult::success(),
                    Err(e) => ExecutionResult::failed(e.to_string()),
                }
            }

            NetworkMessage::SyncRequest { .. } => {
                ExecutionResult::skipped("State sync not implemented")
            }

            NetworkMessage::SyncResponse { .. } => {
                ExecutionResult::skipped("State sync not implemented")
            }
        }
    }

    /// Get the current epoch.
    pub fn epoch(&self) -> Epoch {
        self.consensus.epoch()
    }

    /// Get the current round.
    pub fn round(&self) -> Round {
        self.consensus.round()
    }

    /// Get the current consensus state.
    pub fn state(&self) -> &ConsensusState {
        self.consensus.state()
    }

    /// Get the number of active background tasks.
    pub fn active_task_count(&self) -> usize {
        self.active_tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ConsensusBuilder;
    use crate::testing::{MockBlock, MockVote};
    use consensus_traits::core::Hash as HashTrait;
    use std::hash::Hash as StdHash;

    // Test author type
    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    struct TestAuthor(u8);

    // SAFETY: TestAuthor is a simple wrapper around u8
    unsafe impl Send for TestAuthor {}
    unsafe impl Sync for TestAuthor {}

    impl std::fmt::Display for TestAuthor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestAuthor {
        fn zero() -> Self {
            TestAuthor(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestAuthor(bytes.get(0).copied().unwrap_or(0)))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    // Simple test proposer election
    #[derive(Clone)]
    struct TestProposerElection;

    // SAFETY: TestProposerElection is a unit struct with no internal state
    unsafe impl Send for TestProposerElection {}
    unsafe impl Sync for TestProposerElection {}

    impl consensus_traits::proposer::ProposerElection for TestProposerElection {
        type Round = u64;
        type Author = TestAuthor;

        fn get_valid_proposer(&self, _round: &Self::Round) -> Option<Self::Author> {
            Some(TestAuthor(1))
        }
    }

    type TestConsensus = Consensus<MockBlock, MockVote, TestProposerElection>;

    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.process_interval_ms, 100);
        assert_eq!(config.max_concurrent_tasks, 10);
    }

    #[test]
    fn test_runtime_config_new() {
        let config = RuntimeConfig::new(50, 5, 2000, 2048);
        assert_eq!(config.process_interval_ms, 50);
        assert_eq!(config.max_concurrent_tasks, 5);
        assert_eq!(config.network_timeout_ms, 2000);
        assert_eq!(config.network_buffer_size, 2048);
    }

    #[test]
    fn test_execution_result() {
        assert!(ExecutionResult::success().is_success());
        assert!(ExecutionResult::failed("test error").is_success() == false);
        assert!(ExecutionResult::timeout().is_success() == false);
        assert!(ExecutionResult::skipped("test skip").is_success() == false);
    }

    #[test]
    fn test_network_message_display() {
        let block = Arc::new(MockBlock::genesis());
        let msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };

        assert!(format!("{}", msg).contains("Proposal"));
    }

    #[test]
    fn test_runtime_new() {
        let proposer = TestProposerElection;
        let consensus: TestConsensus = ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

        let runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
            consensus,
            RuntimeConfig::default(),
            None,
        );
        assert!(!runtime.is_running());
        assert_eq!(runtime.pending_message_count(), 0);
    }

    #[test]
    fn test_runtime_add_message() {
        let proposer = TestProposerElection;
        let consensus: TestConsensus = ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

        let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
            consensus,
            RuntimeConfig::default(),
            None,
        );
        let block = Arc::new(MockBlock::genesis());
        runtime.add_network_message(NetworkMessage::Proposal { block });

        assert_eq!(runtime.pending_message_count(), 1);
    }

    #[test]
    fn test_runtime_start_stop() {
        let proposer = TestProposerElection;
        let consensus: TestConsensus = ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

        let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
            consensus,
            RuntimeConfig::default(),
            None,
        );

        assert!(runtime.start().is_ok());
        assert!(runtime.is_running());

        assert!(runtime.shutdown().is_ok());
        assert!(!runtime.is_running());
    }

    #[test]
    fn test_runtime_getters() {
        let proposer = TestProposerElection;
        let consensus: TestConsensus = ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

        let runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
            consensus,
            RuntimeConfig::default(),
            None,
        );

        assert_eq!(runtime.epoch(), 0);
        assert_eq!(runtime.round(), 0);
        assert_eq!(runtime.active_task_count(), 0);
    }
}
