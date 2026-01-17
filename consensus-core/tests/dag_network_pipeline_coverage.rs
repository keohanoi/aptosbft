// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Coverage tests for DAG, Network, and Pipeline modules.
//!
//! This test module targets uncovered code paths in DAG ordering,
//! network message handling, and pipeline processing.

use consensus_core::{
    dag::{
        ordering::{OrderingConfig, RoundRobinAnchorElection, CommitEvent, AnchorElection},
        types::DagNodeId,
    },
    network::{NetworkMessage, ProcessResult, NetworkConfig, SimpleMessageHandler, MessageRouter, MessageHandlerStats, MessageHandler},
    pipeline::{Stage, StageResult, PipelineResult, Pipeline, PipelineConfig, PipelineBuilder, PipelineStats},
    testing::{MockBlock, MockVote, MockHash},
};
use consensus_traits::{
    core::Hash as HashTrait,
};
use std::{
    fmt::{self, Debug},
    sync::Arc,
    hash::Hash as StdHash,
};

// ============================================================================
// Test Types for DAG
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
struct TestRound(u64);

impl fmt::Display for TestRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HashTrait for TestRound {
    fn zero() -> Self {
        TestRound(0)
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, anyhow::Error> {
        Ok(TestRound(0))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
struct TestAuthor(u8);

impl fmt::Display for TestAuthor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HashTrait for TestAuthor {
    fn zero() -> Self {
        TestAuthor(0)
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, anyhow::Error> {
        Ok(TestAuthor(0))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

type TestNodeId = DagNodeId<u64, TestRound, TestAuthor>;

// ============================================================================
// DAG Ordering Coverage Tests (simplified)
// ============================================================================

#[test]
fn test_ordering_config_all_fields() {
    let config = OrderingConfig {
        window_size: 200,
        anchor_interval: 5,
    };
    assert_eq!(config.window_size, 200);
    assert_eq!(config.anchor_interval, 5);
}

#[test]
fn test_ordering_config_default() {
    let config = OrderingConfig::default();
    assert_eq!(config.window_size, 100);
    assert_eq!(config.anchor_interval, 2);
}

#[test]
fn test_commit_event_new_and_fields() {
    let id = TestNodeId::new(1, TestRound(10), TestAuthor(5));
    let parents = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
    let failed = vec![TestAuthor(4)];

    let event = CommitEvent::new(id.clone(), parents.clone(), failed.clone());

    assert_eq!(event.anchor_id, id);
    assert_eq!(event.parents, parents);
    assert_eq!(event.failed_authors, failed);
    assert_eq!(event.parents.len(), 3);
    assert_eq!(event.failed_authors.len(), 1);
}

#[test]
fn test_round_robin_anchor_methods() {
    let authors = vec![TestAuthor(1), TestAuthor(2), TestAuthor(3)];
    let election = RoundRobinAnchorElection::<TestRound, TestAuthor>::new(authors);

    // Test get_anchor (using the trait method)
    let anchor = election.get_anchor(&TestRound(0));
    assert_eq!(anchor, TestAuthor(1));

    // Test get_author_index
    assert_eq!(election.get_author_index(&TestAuthor(1)), Some(0));
    assert_eq!(election.get_author_index(&TestAuthor(2)), Some(1));
    assert_eq!(election.get_author_index(&TestAuthor(99)), None);

    // Test update_reputation (using the trait method)
    let event = CommitEvent::new(TestRound(5), vec![], vec![]);
    election.update_reputation(&event);
}

// ============================================================================
// Network Module Coverage Tests
// ============================================================================

#[test]
fn test_network_message_all_variants() {
    let block = Arc::new(MockBlock::genesis());
    let vote = MockVote::new(MockHash(1), MockHash(0), 0);

    let proposal_msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };
    let vote_msg = NetworkMessage::<MockBlock, MockVote>::Vote { vote };
    let timeout_msg = NetworkMessage::<MockBlock, MockVote>::Timeout { round: 10 };
    let sync_req_msg = NetworkMessage::<MockBlock, MockVote>::SyncRequest { round: 5 };
    let sync_resp_msg = NetworkMessage::<MockBlock, MockVote>::SyncResponse { blocks: vec![] };
    let epoch_msg = NetworkMessage::<MockBlock, MockVote>::EpochChange { epoch: 2 };

    // Just verify they all exist
    let _ = (proposal_msg, vote_msg, timeout_msg, sync_req_msg, sync_resp_msg, epoch_msg);
}

#[test]
fn test_network_message_display_all() {
    let block = Arc::new(MockBlock::genesis());
    let vote = MockVote::new(MockHash(1), MockHash(0), 0);

    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::Proposal { block }), "Proposal");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::Vote { vote }), "Vote");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::Timeout { round: 10 }), "Timeout(round=10)");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::SyncRequest { round: 5 }), "SyncRequest(round=5)");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::SyncResponse { blocks: vec![] }), "SyncResponse(count=0)");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::SyncResponse { blocks: vec![] }), "SyncResponse(count=0)");
    assert_eq!(format!("{}", NetworkMessage::<MockBlock, MockVote>::EpochChange { epoch: 2 }), "EpochChange(epoch=2)");
}

#[test]
fn test_process_result_all_constructors() {
    let success = ProcessResult::success();
    assert!(success.is_success());

    let validation_failed = ProcessResult::validation_failed("bad signature");
    assert!(!validation_failed.is_success());

    let ignored = ProcessResult::ignored("duplicate message");
    assert!(!ignored.is_success());

    let needs_action = ProcessResult::needs_action("process timeout");
    assert!(!needs_action.is_success());
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
fn test_message_handler_stats_new() {
    let stats = MessageHandlerStats::new();
    assert_eq!(stats.total_processed, 0);
    assert_eq!(stats.successful, 0);
    assert_eq!(stats.validation_failures, 0);
    assert_eq!(stats.ignored, 0);
    assert_eq!(stats.needs_action, 0);
}

#[test]
fn test_message_handler_stats_record_all() {
    let mut stats = MessageHandlerStats::new();

    stats.record_success();
    assert_eq!(stats.total_processed, 1);
    assert_eq!(stats.successful, 1);

    stats.record_validation_failure();
    assert_eq!(stats.total_processed, 2);
    assert_eq!(stats.validation_failures, 1);

    stats.record_ignored();
    assert_eq!(stats.total_processed, 3);
    assert_eq!(stats.ignored, 1);

    stats.record_needs_action();
    assert_eq!(stats.total_processed, 4);
    assert_eq!(stats.needs_action, 1);
}

#[test]
fn test_simple_message_handler_all_message_types() {
    let config = NetworkConfig::default();
    let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

    let block = Arc::new(MockBlock::genesis());
    let vote = MockVote::new(MockHash(1), MockHash(0), 0);

    // Test all message types
    let proposal_msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };
    let vote_msg = NetworkMessage::<MockBlock, MockVote>::Vote { vote };
    let timeout_msg = NetworkMessage::<MockBlock, MockVote>::Timeout { round: 10 };
    let sync_req_msg = NetworkMessage::<MockBlock, MockVote>::SyncRequest { round: 5 };
    let sync_resp_msg = NetworkMessage::<MockBlock, MockVote>::SyncResponse { blocks: vec![] };
    let epoch_msg = NetworkMessage::<MockBlock, MockVote>::EpochChange { epoch: 2 };

    assert!(handler.process_message(proposal_msg).unwrap().is_success());
    assert!(handler.process_message(vote_msg).unwrap().is_success());
    assert!(!handler.process_message(timeout_msg).unwrap().is_success()); // needs_action
    assert!(!handler.process_message(sync_req_msg).unwrap().is_success()); // needs_action
    assert!(handler.process_message(sync_resp_msg).unwrap().is_success());
    assert!(!handler.process_message(epoch_msg).unwrap().is_success()); // needs_action
}

#[test]
fn test_simple_message_handler_validation() {
    let mut config = NetworkConfig::default();
    config.validate_immediately = false;

    let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

    let block = Arc::new(MockBlock::genesis());
    let msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };

    // Should succeed even with validation disabled
    let result = handler.process_message(msg).unwrap();
    assert!(result.is_success());
}

#[test]
fn test_message_handler_validate_message() {
    let config = NetworkConfig::default();
    let handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

    let block = Arc::new(MockBlock::genesis());
    let msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };

    // Default implementation accepts all messages
    assert!(handler.validate_message(&msg));
}

#[test]
fn test_simple_message_handler_config() {
    let config = NetworkConfig::default();
    let handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

    assert_eq!(handler.config().max_message_size, 10 * 1024 * 1024);
}

#[test]
fn test_message_router_new() {
    let config = NetworkConfig::default();
    let router = MessageRouter::<MockBlock, MockVote>::new(config);

    // New router should have no handlers and no default
    let block = Arc::new(MockBlock::genesis());
    let msg = NetworkMessage::<MockBlock, MockVote>::Proposal { block };

    let result = router.route_message(msg).unwrap();
    assert!(result.is_none());
}

// ============================================================================
// Pipeline Module Coverage Tests
// ============================================================================

#[test]
fn test_stage_next_all() {
    assert_eq!(Stage::Receiving.next(), Some(Stage::Validation));
    assert_eq!(Stage::Validation.next(), Some(Stage::Processing));
    assert_eq!(Stage::Processing.next(), Some(Stage::Commit));
    assert_eq!(Stage::Commit.next(), None);
}

#[test]
fn test_stage_name_all() {
    assert_eq!(Stage::Receiving.name(), "receiving");
    assert_eq!(Stage::Validation.name(), "validation");
    assert_eq!(Stage::Processing.name(), "processing");
    assert_eq!(Stage::Commit.name(), "commit");
}

#[test]
fn test_stage_display_all() {
    assert_eq!(format!("{}", Stage::Receiving), "receiving");
    assert_eq!(format!("{}", Stage::Validation), "validation");
    assert_eq!(format!("{}", Stage::Processing), "processing");
    assert_eq!(format!("{}", Stage::Commit), "commit");
}

#[test]
fn test_stage_result_continue() {
    let result = StageResult::continue_to_next();
    assert!(result.should_continue());
}

#[test]
fn test_stage_result_stop() {
    let result = StageResult::stop("validation failed");
    assert!(!result.should_continue());
}

#[test]
fn test_stage_result_skip() {
    let result = StageResult::skip("duplicate message");
    assert!(!result.should_continue());
}

#[test]
fn test_pipeline_config_default() {
    let config = PipelineConfig::default();
    assert_eq!(config.stage_timeout_ms, 5000);
    assert_eq!(config.max_concurrent, 10);
    assert!(config.validate_immediately);
    assert!(!config.parallel_processing);
}

#[test]
fn test_pipeline_config_stage_timeout() {
    let config = PipelineConfig::default();
    assert_eq!(config.stage_timeout(), std::time::Duration::from_millis(5000));
}

#[test]
fn test_pipeline_stats_default() {
    let stats = PipelineStats::default();
    assert_eq!(stats.total_processed, 0);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.skipped, 0);
    assert_eq!(stats.stage_counts.len(), 4);
}

#[test]
fn test_pipeline_result_completed() {
    let result = PipelineResult::Completed;
    assert!(result.is_completed());
    assert_eq!(result.final_stage(), None);
}

#[test]
fn test_pipeline_result_stopped() {
    let result = PipelineResult::Stopped {
        stage: Stage::Validation,
        reason: "bad signature".to_string(),
    };
    assert!(!result.is_completed());
    assert_eq!(result.final_stage(), Some(Stage::Validation));
}

#[test]
fn test_pipeline_result_skipped() {
    let result = PipelineResult::Skipped {
        stage: Stage::Processing,
        reason: "duplicate".to_string(),
    };
    assert!(!result.is_completed());
    assert_eq!(result.final_stage(), Some(Stage::Processing));
}

#[test]
fn test_pipeline_new() {
    let config = PipelineConfig::default();
    let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    assert_eq!(pipeline.current_stage(), Stage::Receiving);
    assert_eq!(pipeline.stats().total_processed, 0);
}

#[test]
fn test_pipeline_process_block_completed() {
    let config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    let block = Arc::new(MockBlock::genesis());
    let result = pipeline.process_block(&block).unwrap();

    assert!(result.is_completed());
    assert_eq!(pipeline.stats().total_processed, 4); // 4 stages
}

#[test]
fn test_pipeline_process_vote() {
    let config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    let vote = MockVote::new(MockHash(1), MockHash(0), 0);
    let result = pipeline.process_vote(&vote).unwrap();

    // Simplified implementation returns Completed
    assert!(result.is_completed());
}

#[test]
fn test_pipeline_process_qc() {
    let config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    let result = pipeline.process_qc().unwrap();
    assert!(result.is_completed());
}

#[test]
fn test_pipeline_reset() {
    let config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    // Reset should bring us back to Receiving stage
    pipeline.reset();

    assert_eq!(pipeline.current_stage(), Stage::Receiving);
}

#[test]
fn test_pipeline_config() {
    let config = PipelineConfig::default();
    let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    assert_eq!(pipeline.config().stage_timeout_ms, 5000);
}

#[test]
fn test_pipeline_stats() {
    let config = PipelineConfig::default();
    let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    let stats = pipeline.stats();
    assert_eq!(stats.total_processed, 0);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.skipped, 0);
}

#[test]
fn test_pipeline_builder_all_methods() {
    let pipeline = PipelineBuilder::new()
        .stage_timeout_ms(1000)
        .max_concurrent(5)
        .validate_immediately(false)
        .parallel_processing(true)
        .build::<MockBlock, MockVote>();

    assert_eq!(pipeline.config().stage_timeout_ms, 1000);
    assert_eq!(pipeline.config().max_concurrent, 5);
    assert!(!pipeline.config().validate_immediately);
    assert!(pipeline.config().parallel_processing);
}

#[test]
fn test_pipeline_builder_default() {
    let pipeline = PipelineBuilder::new().build::<MockBlock, MockVote>();

    assert_eq!(pipeline.current_stage(), Stage::Receiving);
}

// ============================================================================
// Integration Tests for Coverage
// ============================================================================

#[test]
fn test_network_message_creation() {
    let block = Arc::new(MockBlock::genesis());

    // Just verify messages can be created
    let _msg1 = NetworkMessage::<MockBlock, MockVote>::Proposal { block: block.clone() };
    let _msg2 = NetworkMessage::<MockBlock, MockVote>::Proposal { block };
    let _msg3 = NetworkMessage::<MockBlock, MockVote>::Timeout { round: 5 };
}

#[test]
fn test_stage_ord() {
    assert!(Stage::Receiving < Stage::Validation);
    assert!(Stage::Validation < Stage::Processing);
    assert!(Stage::Processing < Stage::Commit);
}

#[test]
fn test_stage_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(Stage::Receiving);
    set.insert(Stage::Validation);
    set.insert(Stage::Processing);
    set.insert(Stage::Commit);

    assert_eq!(set.len(), 4);
}
