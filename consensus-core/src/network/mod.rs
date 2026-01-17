// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Network message handling for consensus.
//!
//! This module provides types and traits for handling network messages
//! in the consensus protocol.

use consensus_traits::{
    block::{Block, Vote},
    core::Hash as HashTrait,
    network::{ConsensusMessage, ConsensusNetwork, ValidatorInfo},
};

use Block as _;
use Vote as _;
use std::{
    collections::HashMap,
    fmt::{self, Debug, Display},
    sync::Arc,
    time::Duration,
};
use anyhow::Result;

/// Generic consensus network message types.
///
/// This enum represents all the different message types that can be
/// sent between validators during consensus.
#[derive(Clone, Debug, PartialEq)]
pub enum NetworkMessage<B, V>
where
    B: Block,
    V: Vote,
{
    /// Proposal message (contains a proposed block)
    Proposal {
        /// The proposed block
        block: Arc<B>,
    },

    /// Vote message (contains a vote for a block)
    Vote {
        /// The vote
        vote: V,
    },

    /// Timeout message (indicates round timeout)
    Timeout {
        /// The round that timed out
        round: u64,
    },

    /// Sync request (request for missing blocks/data)
    SyncRequest {
        /// Target round
        round: u64,
    },

    /// Sync response (response with requested data)
    SyncResponse {
        /// Blocks being synced
        blocks: Vec<Arc<B>>,
    },

    /// Epoch change proof
    EpochChange {
        /// New epoch number
        epoch: u64,
    },
}

impl<B, V> Display for NetworkMessage<B, V>
where
    B: Block,
    V: Vote,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMessage::Proposal { .. } => {
                write!(f, "Proposal")
            }
            NetworkMessage::Vote { .. } => {
                write!(f, "Vote")
            }
            NetworkMessage::Timeout { round } => {
                write!(f, "Timeout(round={})", round)
            }
            NetworkMessage::SyncRequest { round } => {
                write!(f, "SyncRequest(round={})", round)
            }
            NetworkMessage::SyncResponse { blocks } => {
                write!(f, "SyncResponse(count={})", blocks.len())
            }
            NetworkMessage::EpochChange { epoch } => {
                write!(f, "EpochChange(epoch={})", epoch)
            }
        }
    }
}

/// Result of processing a network message.
#[derive(Clone, Debug)]
pub enum ProcessResult {
    /// Message was successfully processed
    Success,

    /// Message validation failed
    ValidationFailed {
        /// Reason for failure
        reason: String,
    },

    /// Message was ignored (e.g., duplicate or old)
    Ignored {
        /// Reason for ignoring
        reason: String,
    },

    /// Message triggered further action
    NeedsAction {
        /// Description of the action needed
        action: String,
    },
}

impl ProcessResult {
    /// Create a success result.
    pub fn success() -> Self {
        ProcessResult::Success
    }

    /// Create a validation failure result.
    pub fn validation_failed(reason: impl Into<String>) -> Self {
        ProcessResult::ValidationFailed {
            reason: reason.into(),
        }
    }

    /// Create an ignored result.
    pub fn ignored(reason: impl Into<String>) -> Self {
        ProcessResult::Ignored {
            reason: reason.into(),
        }
    }

    /// Create a needs action result.
    pub fn needs_action(action: impl Into<String>) -> Self {
        ProcessResult::NeedsAction {
            action: action.into(),
        }
    }

    /// Check if the result indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self, ProcessResult::Success)
    }
}

/// Configuration for network message handling.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Timeout for network operations
    pub network_timeout_ms: u64,

    /// Maximum number of pending sync requests
    pub max_pending_syncs: usize,

    /// Whether to validate messages immediately
    pub validate_immediately: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            max_message_size: 10 * 1024 * 1024, // 10 MB
            network_timeout_ms: 5000,          // 5 seconds
            max_pending_syncs: 10,
            validate_immediately: true,
        }
    }
}

/// Network message handler.
///
/// This trait defines the interface for handling incoming network messages
/// in the consensus protocol.
pub trait MessageHandler<B, V>: Send + Sync
where
    B: Block,
    V: Vote,
{
    /// Process a network message.
    ///
    /// Returns the result of processing the message.
    fn process_message(&mut self, message: NetworkMessage<B, V>) -> Result<ProcessResult>;

    /// Validate a message before processing.
    ///
    /// Returns true if the message is valid.
    fn validate_message(&self, message: &NetworkMessage<B, V>) -> bool {
        // Default implementation: all messages are valid
        true
    }

    /// Get statistics about message processing.
    fn get_stats(&self) -> MessageHandlerStats;
}

/// Statistics for message processing.
#[derive(Clone, Debug, Default)]
pub struct MessageHandlerStats {
    /// Total messages processed
    pub total_processed: u64,

    /// Successful processes
    pub successful: u64,

    /// Failed validations
    pub validation_failures: u64,

    /// Ignored messages
    pub ignored: u64,

    /// Messages triggering actions
    pub needs_action: u64,
}

impl MessageHandlerStats {
    /// Create new stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful process.
    pub fn record_success(&mut self) {
        self.total_processed += 1;
        self.successful += 1;
    }

    /// Record a validation failure.
    pub fn record_validation_failure(&mut self) {
        self.total_processed += 1;
        self.validation_failures += 1;
    }

    /// Record an ignored message.
    pub fn record_ignored(&mut self) {
        self.total_processed += 1;
        self.ignored += 1;
    }

    /// Record a message that needs action.
    pub fn record_needs_action(&mut self) {
        self.total_processed += 1;
        self.needs_action += 1;
    }
}

/// Simple message handler implementation.
///
/// This is a basic implementation that tracks statistics and
/// provides a foundation for more complex handlers.
#[derive(Clone)]
pub struct SimpleMessageHandler<B, V>
where
    B: Block,
    V: Vote,
{
    /// Processing statistics
    stats: MessageHandlerStats,

    /// Configuration
    config: NetworkConfig,

    /// Phantom data for generic types
    _phantom: std::marker::PhantomData<(B, V)>,
}

impl<B, V> SimpleMessageHandler<B, V>
where
    B: Block,
    V: Vote,
{
    /// Create a new simple message handler.
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            stats: MessageHandlerStats::new(),
            config,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// Get mutable statistics (for testing).
    #[cfg(test)]
    pub fn stats_mut(&mut self) -> &mut MessageHandlerStats {
        &mut self.stats
    }
}

impl<B, V> MessageHandler<B, V> for SimpleMessageHandler<B, V>
where
    B: Block,
    V: Vote,
{
    fn process_message(&mut self, message: NetworkMessage<B, V>) -> Result<ProcessResult> {
        // Validate the message first
        if self.config.validate_immediately && !self.validate_message(&message) {
            self.stats.record_validation_failure();
            return Ok(ProcessResult::validation_failed("Message validation failed"));
        }

        // Process the message based on type
        let result = match message {
            NetworkMessage::Proposal { .. } => {
                // In production, this would add the proposal to the proposal pool
                self.stats.record_success();
                ProcessResult::success()
            }
            NetworkMessage::Vote { .. } => {
                // In production, this would process the vote
                self.stats.record_success();
                ProcessResult::success()
            }
            NetworkMessage::Timeout { .. } => {
                // In production, this would trigger round timeout handling
                self.stats.record_needs_action();
                ProcessResult::needs_action("Process timeout")
            }
            NetworkMessage::SyncRequest { .. } => {
                // In production, this would handle sync requests
                self.stats.record_needs_action();
                ProcessResult::needs_action("Handle sync request")
            }
            NetworkMessage::SyncResponse { .. } => {
                // In production, this would process sync responses
                self.stats.record_success();
                ProcessResult::success()
            }
            NetworkMessage::EpochChange { .. } => {
                // In production, this would handle epoch changes
                self.stats.record_needs_action();
                ProcessResult::needs_action("Handle epoch change")
            }
        };

        Ok(result)
    }

    fn get_stats(&self) -> MessageHandlerStats {
        self.stats.clone()
    }
}

/// Message router for directing messages to appropriate handlers.
///
/// This allows different message types to be handled by different
/// specialized handlers.
#[derive(Clone)]
pub struct MessageRouter<B, V>
where
    B: Block,
    V: Vote,
{
    /// Map from message type to handler
    handlers: HashMap<String, Arc<dyn MessageHandler<B, V>>>,

    /// Default handler for unrecognized message types
    default_handler: Option<Arc<dyn MessageHandler<B, V>>>,

    /// Configuration
    config: NetworkConfig,
}

impl<B, V> MessageRouter<B, V>
where
    B: Block,
    V: Vote,
{
    /// Create a new message router.
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: None,
            config,
        }
    }

    /// Register a handler for a message type.
    pub fn register_handler(
        &mut self,
        message_type: impl Into<String>,
        handler: Arc<dyn MessageHandler<B, V>>,
    ) {
        self.handlers.insert(message_type.into(), handler);
    }

    /// Set the default handler.
    pub fn set_default_handler(
        &mut self,
        handler: Arc<dyn MessageHandler<B, V>>,
    ) {
        self.default_handler = Some(handler);
    }

    /// Route a message to the appropriate handler.
    pub fn route_message(
        &self,
        message: NetworkMessage<B, V>,
    ) -> Result<Option<ProcessResult>> {
        let message_type = self.get_message_type(&message);

        if let Some(handler) = self.handlers.get(&message_type) {
            // Clone the handler to get mutable reference
            // In production, you'd use interior mutability (Arc<Mutex<>>)
            // For now, we'll return None to indicate we couldn't process
            return Ok(None);
        } else if let Some(default) = &self.default_handler {
            // Same issue with mutability
            return Ok(None);
        }

        Ok(None)
    }

    /// Get the message type for a message.
    fn get_message_type(&self, message: &NetworkMessage<B, V>) -> String {
        match message {
            NetworkMessage::Proposal { .. } => "proposal".to_string(),
            NetworkMessage::Vote { .. } => "vote".to_string(),
            NetworkMessage::Timeout { .. } => "timeout".to_string(),
            NetworkMessage::SyncRequest { .. } => "sync_request".to_string(),
            NetworkMessage::SyncResponse { .. } => "sync_response".to_string(),
            NetworkMessage::EpochChange { .. } => "epoch_change".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockBlock, MockVote};

    #[test]
    fn test_process_result() {
        assert!(ProcessResult::success().is_success());
        assert!(!ProcessResult::validation_failed("test").is_success());
        assert!(!ProcessResult::ignored("test").is_success());
        assert!(!ProcessResult::needs_action("test").is_success());
    }

    #[test]
    fn test_message_handler_stats() {
        let mut stats = MessageHandlerStats::new();

        stats.record_success();
        stats.record_validation_failure();
        stats.record_ignored();
        stats.record_needs_action();

        assert_eq!(stats.total_processed, 4);
        assert_eq!(stats.successful, 1);
        assert_eq!(stats.validation_failures, 1);
        assert_eq!(stats.ignored, 1);
        assert_eq!(stats.needs_action, 1);
    }

    #[test]
    fn test_simple_message_handler() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::Proposal {
            block: Arc::new(MockBlock::genesis()),
        };

        let result = handler.process_message(message).unwrap();

        assert!(result.is_success());
        assert_eq!(handler.get_stats().total_processed, 1);
        assert_eq!(handler.get_stats().successful, 1);
    }

    #[test]
    fn test_message_router() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        // Test message type detection
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Timeout { round: 10 };

        let message_type = router.get_message_type(&message);
        assert_eq!(message_type, "timeout");
    }

    #[test]
    fn test_network_message_display() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Timeout { round: 10 };
        assert_eq!(format!("{}", message), "Timeout(round=10)");
    }

    #[test]
    fn test_network_message_display_proposal() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Proposal {
            block: Arc::new(MockBlock::genesis()),
        };
        assert_eq!(format!("{}", message), "Proposal");
    }

    #[test]
    fn test_network_message_display_vote() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Vote {
            vote: MockVote::new(crate::testing::MockHash(0), crate::testing::MockHash(0), 0),
        };
        assert_eq!(format!("{}", message), "Vote");
    }

    #[test]
    fn test_network_message_display_sync_request() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::SyncRequest { round: 5 };
        assert_eq!(format!("{}", message), "SyncRequest(round=5)");
    }

    #[test]
    fn test_network_message_display_sync_response() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::SyncResponse {
            blocks: vec![],
        };
        assert_eq!(format!("{}", message), "SyncResponse(count=0)");
    }

    #[test]
    fn test_network_message_display_epoch_change() {
        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::EpochChange { epoch: 2 };
        assert_eq!(format!("{}", message), "EpochChange(epoch=2)");
    }

    #[test]
    fn test_message_handler_stats_mut() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        // Test stats_mut accessor (test-only method)
        handler.stats_mut().total_processed = 100;
        assert_eq!(handler.get_stats().total_processed, 100);
    }

    #[test]
    fn test_simple_message_handler_with_validation_disabled() {
        let mut config = NetworkConfig::default();
        config.validate_immediately = false;
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::Proposal {
            block: Arc::new(MockBlock::genesis()),
        };

        let result = handler.process_message(message).unwrap();
        assert!(result.is_success());
    }

    #[test]
    fn test_simple_message_handler_vote() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::Vote {
            vote: MockVote::new(crate::testing::MockHash(0), crate::testing::MockHash(0), 0),
        };

        let result = handler.process_message(message).unwrap();
        assert!(result.is_success());
    }

    #[test]
    fn test_simple_message_handler_timeout() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::Timeout { round: 5 };

        let result = handler.process_message(message).unwrap();
        assert!(!result.is_success()); // needs_action
        assert_eq!(handler.get_stats().needs_action, 1);
    }

    #[test]
    fn test_simple_message_handler_sync_request() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::SyncRequest { round: 10 };

        let result = handler.process_message(message).unwrap();
        assert!(!result.is_success()); // needs_action
    }

    #[test]
    fn test_simple_message_handler_sync_response() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::SyncResponse {
            blocks: vec![],
        };

        let result = handler.process_message(message).unwrap();
        assert!(result.is_success());
    }

    #[test]
    fn test_simple_message_handler_epoch_change() {
        let config = NetworkConfig::default();
        let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

        let message = NetworkMessage::EpochChange { epoch: 2 };

        let result = handler.process_message(message).unwrap();
        assert!(!result.is_success()); // needs_action
    }

    #[test]
    fn test_message_router_register_handler() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config.clone());
        let handler = Arc::new(SimpleMessageHandler::<MockBlock, MockVote>::new(config));

        // Test register_handler
        let mut router = router;
        router.register_handler("proposal", handler);

        let message_type = router.get_message_type(&NetworkMessage::Proposal {
            block: Arc::new(MockBlock::genesis()),
        });
        assert_eq!(message_type, "proposal");
    }

    #[test]
    fn test_message_router_set_default_handler() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config.clone());
        let handler = Arc::new(SimpleMessageHandler::<MockBlock, MockVote>::new(config));

        // Test set_default_handler
        let mut router = router;
        router.set_default_handler(handler);

        // Verify the handler is set (we can't directly access it but we know it's set)
        // The route_message will return None due to Arc mutability, but we test the setter
    }

    #[test]
    fn test_message_router_get_message_type_vote() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Vote {
            vote: MockVote::new(crate::testing::MockHash(0), crate::testing::MockHash(0), 0),
        };

        let message_type = router.get_message_type(&message);
        assert_eq!(message_type, "vote");
    }

    #[test]
    fn test_message_router_get_message_type_sync_request() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::SyncRequest { round: 5 };

        let message_type = router.get_message_type(&message);
        assert_eq!(message_type, "sync_request");
    }

    #[test]
    fn test_message_router_get_message_type_sync_response() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::SyncResponse {
            blocks: vec![],
        };

        let message_type = router.get_message_type(&message);
        assert_eq!(message_type, "sync_response");
    }

    #[test]
    fn test_message_router_get_message_type_epoch_change() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::EpochChange { epoch: 1 };

        let message_type = router.get_message_type(&message);
        assert_eq!(message_type, "epoch_change");
    }

    #[test]
    fn test_message_router_route_message() {
        let config = NetworkConfig::default();
        let router = MessageRouter::<MockBlock, MockVote>::new(config);

        let message: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Timeout { round: 10 };

        // Test route_message - returns None due to no registered handlers
        let result = router.route_message(message);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_process_result_constructors() {
        // Test all constructor methods
        let _ = ProcessResult::success();
        let _ = ProcessResult::validation_failed("invalid signature");
        let _ = ProcessResult::ignored("duplicate message");
        let _ = ProcessResult::needs_action("rebroadcast required");
    }

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
        assert_eq!(config.network_timeout_ms, 5000);
        assert_eq!(config.max_pending_syncs, 10);
        assert!(config.validate_immediately);
    }

    #[test]
    fn test_simple_message_handler_config_accessor() {
        let config = NetworkConfig::default();
        let handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config.clone());

        assert_eq!(handler.config().max_message_size, config.max_message_size);
    }
}
