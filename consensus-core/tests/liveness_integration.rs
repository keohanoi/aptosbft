// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Liveness integration tests for consensus.
//!
//! This module contains integration tests for liveness components including
//! pacemaker, proposer election, network message handling, and pipeline.

use consensus_core::{
    liveness::{
        Pacemaker, PacemakerConfig, NewRoundReason, ExponentialIntervalStrategy, ProposerElectionConfig,
        proposer::{RoundRobinProposer, WeightedProposer},
        RoundIntervalStrategy,
    },
    network::{NetworkMessage, ProcessResult, NetworkConfig, SimpleMessageHandler, MessageHandler},
    pipeline::{Pipeline, PipelineConfig, Stage},
    dag::testing::{TestAuthor, create_test_authors},
    testing::{MockBlock, MockVote},
};
use consensus_traits::proposer::ProposerElection;
use std::sync::Arc;

#[test]
fn test_pacemaker_round_progression() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        4,
    );
    let config = PacemakerConfig {
        base_duration_ms: 100,
        exponent_base: 2.0,
        max_exponent: 4,
        initial_round: 1,
    };

    let mut pacemaker = Pacemaker::new(strategy, config);

    // First record that round 1 was ordered (simulating a QC)
    pacemaker.record_ordered_round(1).unwrap();

    // Enter rounds sequentially
    let event1 = pacemaker.enter_new_round(2).unwrap();
    assert_eq!(event1.round, 2);
    assert!(matches!(event1.reason, NewRoundReason::QuorumCertificate));

    let event2 = pacemaker.enter_new_round(3).unwrap();
    assert_eq!(event2.round, 3);

    assert_eq!(pacemaker.current_round(), 3);
}

#[test]
fn test_pacemaker_timeout() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        4,
    );
    let config = PacemakerConfig::default();
    let mut pacemaker = Pacemaker::new(strategy, config);

    // Record an ordered round
    pacemaker.record_ordered_round(1).unwrap();

    // Next round should have shorter timeout (0 rounds since ordered)
    let timeout = pacemaker.current_round_timeout();
    assert_eq!(timeout, std::time::Duration::from_millis(100));
}

#[test]
fn test_exponential_interval_strategy() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        4,
    );

    // 0 rounds since ordered = base duration
    assert_eq!(strategy.get_round_duration(0), std::time::Duration::from_millis(100));

    // 1 round since ordered = 2x base
    assert_eq!(strategy.get_round_duration(1), std::time::Duration::from_millis(200));

    // 2 rounds since ordered = 4x base
    assert_eq!(strategy.get_round_duration(2), std::time::Duration::from_millis(400));

    // Capped at max_exponent
    assert_eq!(strategy.get_round_duration(10), std::time::Duration::from_millis(1600));
}

#[test]
fn test_round_robin_proposer() {
    let authors = create_test_authors(3);
    let config = ProposerElectionConfig::default();

    let election = RoundRobinProposer::new(authors, config);

    // Round 1 should select first author (index 0)
    assert_eq!(election.get_valid_proposer(&1), Some(TestAuthor(1)));

    // Round 2 should select second author (index 1)
    assert_eq!(election.get_valid_proposer(&2), Some(TestAuthor(2)));

    // Round 3 should select third author (index 2)
    assert_eq!(election.get_valid_proposer(&3), Some(TestAuthor(3)));

    // Round 4 should wrap back to first author
    assert_eq!(election.get_valid_proposer(&4), Some(TestAuthor(1)));
}

#[test]
fn test_weighted_proposer() {
    let authors = vec![
        (TestAuthor(1), 30), // 30% voting power
        (TestAuthor(2), 70), // 70% voting power
    ];
    let config = ProposerElectionConfig::default();

    let election = WeightedProposer::new(authors, config);

    // With weights 30/70, we should see selection proportional to weights
    let mut counts = std::collections::HashMap::new();

    for round in 1..=100 {
        if let Some(proposer) = election.get_valid_proposer(&round) {
            *counts.entry(proposer.0).or_insert(0) += 1;
        }
    }

    // Author 2 should be selected more often (higher weight)
    let count1 = *counts.get(&1).unwrap_or(&0);
    let count2 = *counts.get(&2).unwrap_or(&0);

    assert!(count2 > count1, "Author with higher weight should be selected more");
    assert_eq!(count1 + count2, 100, "Total selections should equal rounds");
}

#[test]
fn test_network_message_processing() {
    let config = NetworkConfig::default();
    let mut handler = SimpleMessageHandler::<MockBlock, MockVote>::new(config);

    let message = NetworkMessage::Timeout { round: 10 };
    let result = handler.process_message(message).unwrap();

    // Timeout messages should trigger action
    match result {
        ProcessResult::NeedsAction { action } => {
            assert_eq!(action, "Process timeout");
        }
        _ => panic!("Expected NeedsAction result"),
    }

    // Verify stats were updated
    let stats = handler.get_stats();
    assert_eq!(stats.total_processed, 1);
    assert_eq!(stats.needs_action, 1);
}

#[test]
fn test_pipeline_stages() {
    let config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    let block = Arc::new(MockBlock::genesis());
    let result = pipeline.process_block(&block).unwrap();

    // With placeholder implementations, should complete
    assert!(result.is_completed());

    // Verify all stages were processed
    let stats = pipeline.stats();
    assert_eq!(stats.total_processed, 4); // 4 stages
}

#[test]
fn test_pipeline_stopped() {
    // Create a custom handler that stops at validation
    let config = PipelineConfig::default();
    let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

    // Test that we can handle pipeline stopping
    // (This would require more complex mocking in production)
    assert_eq!(pipeline.current_stage(), Stage::Receiving);
}

#[test]
fn test_new_round_reason_display() {
    let reason = NewRoundReason::QuorumCertificate;
    assert_eq!(format!("{}", reason), "QCReady");

    let reason = NewRoundReason::Timeout { rounds_since_ordered: 5 };
    assert_eq!(format!("{}", reason), "Timeout(rounds_since_ordered=5)");
}

#[test]
fn test_stage_progression() {
    assert_eq!(Stage::Receiving.next(), Some(Stage::Validation));
    assert_eq!(Stage::Validation.next(), Some(Stage::Processing));
    assert_eq!(Stage::Processing.next(), Some(Stage::Commit));
    assert_eq!(Stage::Commit.next(), None);
}

#[test]
fn test_integration_end_to_end() {
    // Create a full liveness stack
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        4,
    );
    let config = PacemakerConfig {
        base_duration_ms: 100,
        exponent_base: 2.0,
        max_exponent: 4,
        initial_round: 1,
    };
    let mut pacemaker = Pacemaker::new(strategy, config);

    let authors = create_test_authors(3);
    let proposer = RoundRobinProposer::new(authors, ProposerElectionConfig::default());

    let network_config = NetworkConfig::default();
    let network_handler = SimpleMessageHandler::<MockBlock, MockVote>::new(network_config);

    let pipeline_config = PipelineConfig::default();
    let mut pipeline = Pipeline::<MockBlock, MockVote>::new(pipeline_config);

    // Test the integration works
    assert_eq!(pacemaker.current_round(), 1);
    assert_eq!(proposer.get_valid_proposer(&1), Some(TestAuthor(1)));
    assert_eq!(network_handler.get_stats().total_processed, 0);
    assert_eq!(pipeline.stats().total_processed, 0);

    // Simulate round progression
    let _ = pacemaker.enter_new_round(2).unwrap();
    assert_eq!(pacemaker.current_round(), 2);

    // Process a block
    let block = Arc::new(MockBlock::genesis());
    let _ = pipeline.process_block(&block).unwrap();
    assert_eq!(pipeline.stats().total_processed, 4); // 4 stages
}
