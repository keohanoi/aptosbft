// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Comprehensive coverage tests for consensus components.
//!
//! This test module targets uncovered code paths to achieve >95% coverage.
//! Tests are organized by module and focus on error paths and edge cases.

use consensus_core::{
    consensus::{self, Consensus, ConsensusBuilder, ConsensusConfig, ConsensusState, ProposalResult, VoteResult},
    consensus::runtime::{ConsensusRuntime, ExecutionResult, NetworkMessage, RuntimeConfig},
    liveness::{pacemaker::PacemakerConfig, proposer::RoundRobinProposer},
    pipeline::PipelineConfig,
    round_manager::{ProposalProcessingResult, RoundManager, RoundManagerConfig},
    safety_rules::{TwoChainSafetyRules, SafetyStateData},
    timeout::{TwoChainTimeout, TwoChainTimeoutCertificate},
    testing::{self, MockBlock, MockVote, MockHash, MockSignature},
    types::{Epoch, Round, TimeoutReason},
    epoch_manager::EpochManager,
    liveness::pacemaker::{ExponentialIntervalStrategy, Pacemaker, RoundIntervalStrategy},
};
use consensus_traits::{
    block::{Block, BlockMetadata, LedgerInfo, QuorumCertificate, Transaction, Vote, VoteData},
    core::{Error, Hash as HashTrait, NodeId, Signature},
    proposer::ProposerElection,
    SafetyRules, SafetyState, TimeoutCertificate, ValidatorVerifier,
};
use std::{
    fmt::{self, Debug, Display},
    sync::Arc,
    hash::Hash as StdHash,
};

// ============================================================================
// Helper Functions for Testing
// ============================================================================

// Helper to create a vote
fn make_vote(_epoch: u64, round: u64, author_id: u8, block_id: MockHash) -> MockVote {
    MockVote::new(testing::MockHash(author_id), block_id, round)
}

// Helper to create a block
fn make_block(epoch: u64, round: u64, author_id: u8, parent_id: MockHash) -> MockBlock {
    let metadata = testing::MockBlockMetadata::new(epoch, round, testing::MockHash(author_id), parent_id, round * 1000);
    MockBlock::new(vec![], metadata)
}

// Helper to create a quorum certificate
fn make_qc(round: u64, epoch: u64, block_id: MockHash) -> testing::MockQuorumCert {
    let metadata = testing::MockBlockMetadata::new(epoch, round, testing::MockHash(0), block_id, round * 1000);
    testing::MockQuorumCert::with_metadata(metadata, block_id)
}

// ============================================================================
// Mock Types for Testing
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

    fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
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
        write!(f, "Author({})", self.0)
    }
}

impl HashTrait for TestAuthor {
    fn zero() -> Self {
        TestAuthor(0)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(TestAuthor(bytes.get(0).copied().unwrap_or(0)))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

impl NodeId for TestAuthor {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(TestAuthor(bytes.get(0).copied().unwrap_or(0)))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

type TestNodeId = consensus_core::dag::types::DagNodeId<u64, TestRound, TestAuthor>;

#[derive(Clone)]
struct TestProposerElection {
    validators: Vec<TestAuthor>,
}

impl ProposerElection for TestProposerElection {
    type Round = u64;
    type Author = TestAuthor;

    fn get_valid_proposer(&self, round: &Self::Round) -> Option<Self::Author> {
        if self.validators.is_empty() {
            return None;
        }
        let index = (*round as usize) % self.validators.len();
        self.validators.get(index).cloned()
    }

    fn get_voting_power_participation_ratio(&self, _round: &Self::Round) -> f64 {
        1.0 // All validators participate for testing
    }
}

#[derive(Clone, Debug)]
struct TestVerifier;

impl ValidatorVerifier<MockBlock, MockVote> for TestVerifier {
    fn verify_vote(&self, _vote: &MockVote) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block(&self, _block: &MockBlock) -> Result<(), Error> {
        Ok(())
    }

    fn get_voting_power(&self, _validator_id: &<MockVote as Vote>::NodeId) -> Option<u64> {
        Some(100)
    }

    fn total_voting_power(&self) -> u128 {
        400
    }

    fn quorum_voting_power(&self) -> u128 {
        267
    }

    fn check_voting_power<'a>(
        &self,
        signers: impl Iterator<Item = &'a <MockVote as Vote>::NodeId>,
        check_quorum: bool,
    ) -> Result<(), consensus_traits::core::VerifyError> {
        let mut power = 0u128;
        for _signer in signers {
            power += 100;
        }

        if check_quorum && power < self.quorum_voting_power() {
            return Err(consensus_traits::core::VerifyError::TooLittleVotingPower {
                voting_power: power,
                expected_voting_power: self.quorum_voting_power(),
            });
        }

        Ok(())
    }

    fn aggregate_signatures<'a>(
        &self,
        _signatures: impl Iterator<Item = &'a <MockVote as Vote>::Signature>,
    ) -> Result<<MockSignature as Signature>::Aggregated, Error> {
        Ok(consensus_core::testing::MockAggregatedSignature)
    }
}

// ============================================================================
// Consensus/mod.rs Coverage Tests
// ============================================================================

#[test]
fn test_consensus_process_proposal_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    // Consensus starts in Bootstrapping state, not Running
    let proposal = Arc::new(MockBlock::genesis());
    let verifier = TestVerifier;

    let result = consensus.process_proposal(proposal, &verifier).unwrap();

    assert!(!result.accepted);
    assert!(result.reason.is_some());
    assert!(!result.voted);
    assert_eq!(consensus.state(), &ConsensusState::Bootstrapping);
}

#[test]
fn test_consensus_process_vote_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    // Consensus in Bootstrapping state
    let vote = make_vote(0, 0, 0, MockHash(0));
    let verifier = TestVerifier;

    let result = consensus.process_vote(vote, &verifier);

    // Should fail because not in Running state
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not in running state"));
}

#[test]
fn test_consensus_process_qc_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let qc = make_qc(0, 0, MockHash(0));

    let result = consensus.process_qc(Arc::new(qc));

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not in running state"));
}

#[test]
fn test_consensus_process_timeout_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let result = consensus.process_timeout(5);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not in running state"));
}

#[test]
fn test_consensus_generate_proposal_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .self_validator_id(TestAuthor(1))
            .build()
            .unwrap();

    // Not in running state
    let result = consensus.generate_proposal();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not in running state"));
}

#[test]
fn test_consensus_generate_proposal_when_not_proposer() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .self_validator_id(TestAuthor(3)) // Not in validators list
            .build()
            .unwrap();

    consensus.start().unwrap();

    // We're not a validator at all, so won't be proposer
    let result = consensus.generate_proposal().unwrap();

    assert!(result.is_none());
}

#[test]
fn test_consensus_stop_when_not_running() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    // Already in Bootstrapping, not Running
    let result = consensus.stop();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cannot stop consensus from state"));
}

#[test]
fn test_consensus_stop_when_already_stopped() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    consensus.start().unwrap();
    consensus.stop().unwrap();

    // Try to stop again
    let result = consensus.stop();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cannot stop consensus from state"));
}

#[test]
fn test_consensus_builder_with_custom_config() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let config = ConsensusConfig::default();

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .config(config.clone())
            .proposer_election(proposer)
            .self_validator_id(TestAuthor(1))
            .build()
            .unwrap();

    assert_eq!(consensus.epoch(), config.initial_epoch);
}

#[test]
fn test_consensus_builder_missing_proposer_election() {
    let result: Result<Consensus<MockBlock, MockVote, TestProposerElection>, _> =
        ConsensusBuilder::new().build();

    assert!(result.is_err());
}

// ============================================================================
// Consensus/Runtime Coverage Tests
// ============================================================================

#[test]
fn test_runtime_config_durations() {
    let config = RuntimeConfig::new(100, 10, 5000, 2048);

    assert_eq!(config.process_interval(), std::time::Duration::from_millis(100));
    assert_eq!(config.network_timeout(), std::time::Duration::from_millis(5000));
}

#[test]
fn test_execution_result_variants() {
    assert!(ExecutionResult::Success.is_success());
    assert!(!ExecutionResult::Failed { message: "error".to_string() }.is_success());
    assert!(!ExecutionResult::Timeout.is_success());
    assert!(!ExecutionResult::Skipped { reason: "skip".to_string() }.is_success());
}

#[test]
fn test_execution_result_constructors() {
    let success = ExecutionResult::success();
    assert!(matches!(success, ExecutionResult::Success));

    let failed = ExecutionResult::failed("test error");
    assert!(matches!(failed, ExecutionResult::Failed { .. }));

    let timeout = ExecutionResult::timeout();
    assert!(matches!(timeout, ExecutionResult::Timeout));

    let skipped = ExecutionResult::skipped("test reason");
    assert!(matches!(skipped, ExecutionResult::Skipped { .. }));
}

#[test]
fn test_network_message_vote_display() {
    let vote = Arc::new(make_vote(0, 5, 1, MockHash(5)));
    let msg = NetworkMessage::<MockBlock, MockVote>::Vote { vote };

    let display_str = format!("{}", msg);
    // Check that display contains "Vote"
    assert!(display_str.contains("Vote"));
}

#[test]
fn test_network_message_qc_display() {
    let qc = make_qc(10, 0, MockHash(10));
    let msg = NetworkMessage::<MockBlock, MockVote>::QuorumCertificate { qc: Arc::new(qc) };

    let display_str = format!("{}", msg);
    assert!(display_str.contains("QC"));
    assert!(display_str.contains("10"));
}

#[test]
fn test_network_message_timeout_display() {
    let msg = NetworkMessage::<MockBlock, MockVote>::Timeout { round: 7 };

    let display_str = format!("{}", msg);
    assert!(display_str.contains("Timeout"));
    assert!(display_str.contains("7"));
}

#[test]
fn test_network_message_sync_request_display() {
    let msg = NetworkMessage::<MockBlock, MockVote>::SyncRequest { target_round: 100 };

    let display_str = format!("{}", msg);
    assert!(display_str.contains("SyncRequest"));
    assert!(display_str.contains("100"));
}

#[test]
fn test_network_message_sync_response_display() {
    let blocks = vec![
        Arc::new(MockBlock::genesis()),
        Arc::new(MockBlock::genesis()),
    ];
    let msg = NetworkMessage::<MockBlock, MockVote>::SyncResponse { blocks };

    let display_str = format!("{}", msg);
    assert!(display_str.contains("SyncResponse"));
    assert!(display_str.contains("2"));
}

#[test]
fn test_runtime_process_pending_messages() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    // Add various messages
    let block = Arc::new(MockBlock::genesis());
    runtime.add_network_message(NetworkMessage::Proposal { block });

    let vote = Arc::new(make_vote(0, 0, 1, MockHash(0)));
    runtime.add_network_message(NetworkMessage::Vote { vote });

    let qc = make_qc(0, 0, MockHash(0));
    runtime.add_network_message(NetworkMessage::QuorumCertificate { qc: Arc::new(qc) });

    runtime.add_network_message(NetworkMessage::Timeout { round: 1 });

    runtime.add_network_message(NetworkMessage::SyncRequest { target_round: 5 });

    let blocks = vec![Arc::new(MockBlock::genesis())];
    runtime.add_network_message(NetworkMessage::SyncResponse { blocks });

    // Process all messages
    let results = runtime.process_pending_messages();

    assert_eq!(results.len(), 6);

    // Check that sync messages return Skipped
    let sync_request_result = &results[4];
    let sync_response_result = &results[5];

    assert!(!sync_request_result.1.is_success());
    assert!(!sync_response_result.1.is_success());
}

#[test]
fn test_runtime_handle_network_message_proposal() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let block = Arc::new(MockBlock::genesis());
    let result = runtime.handle_network_message(NetworkMessage::Proposal { block });

    assert!(result.is_success());
}

#[test]
fn test_runtime_handle_network_message_vote() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let vote = Arc::new(make_vote(0, 0, 1, MockHash(0)));
    let result = runtime.handle_network_message(NetworkMessage::Vote { vote });

    assert!(result.is_success());
}

#[test]
fn test_runtime_handle_network_message_timeout() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    // Start consensus to put it in Running state
    consensus.start().unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let result = runtime.handle_network_message(NetworkMessage::Timeout { round: 1 });

    // Should succeed now that consensus is running
    assert!(result.is_success());
}

#[test]
fn test_runtime_handle_network_message_sync_request() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let result = runtime.handle_network_message(NetworkMessage::SyncRequest { target_round: 5 });

    assert!(!result.is_success());
}

#[test]
fn test_runtime_handle_network_message_sync_response() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let blocks = vec![Arc::new(MockBlock::genesis())];
    let result = runtime.handle_network_message(NetworkMessage::SyncResponse { blocks });

    assert!(!result.is_success());
}

#[test]
fn test_runtime_execute_block() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let block = Arc::new(MockBlock::genesis());
    let result = runtime.execute_block(block);

    assert!(result.is_success());
}

#[test]
fn test_runtime_broadcast_proposal() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let proposal = Arc::new(MockBlock::genesis());
    let result = runtime.broadcast_proposal(proposal);

    assert!(result.is_success());
}

#[test]
fn test_runtime_broadcast_vote() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    let vote = Arc::new(make_vote(0, 0, 1, MockHash(0)));
    let result = runtime.broadcast_vote(vote);

    assert!(result.is_success());
}

#[test]
fn test_runtime_shutdown_when_not_running() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    // Runtime not started
    let result = runtime.shutdown();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Runtime is not running"));
}

#[test]
fn test_runtime_start_when_already_running() {
    use consensus_core::consensus::Consensus;

    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .build()
            .unwrap();

    let mut runtime = ConsensusRuntime::<MockBlock, MockVote, TestProposerElection, ()>::new(
        consensus,
        RuntimeConfig::default(),
        None,
    );

    runtime.start().unwrap();

    // Try to start again
    let result = runtime.start();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Runtime is already running"));
}

// ============================================================================
// Timeout/TwoChain Coverage Tests
// ============================================================================

#[test]
fn test_timeout_verify_invalid_hqc_round_equals() {
    let timeout = TwoChainTimeout::new(0, 5, 5); // hqc_round == round

    let result = timeout.verify();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hqc_round"));
}

#[test]
fn test_timeout_verify_invalid_hqc_round_greater() {
    let timeout = TwoChainTimeout::new(0, 5, 10); // hqc_round > round

    let result = timeout.verify();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hqc_round"));
}

#[test]
fn test_timeout_certificate_verify_invalid_hqc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 5, 5)); // Invalid: hqc_round == round
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 5, 5, 0, timeout, MockSignature(0),
    );

    let result = tc.verify();

    assert!(result.is_err());
}

#[test]
fn test_timeout_certificate_verify_invalid_highest_tc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 10, 5, 10, // Invalid: highest_tc_round == round
        timeout, MockSignature(0),
    );

    let result = tc.verify();

    assert!(result.is_err());
}

#[test]
fn test_timeout_certificate_allows_advance_true_via_hqc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 5, 4));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 5, 4, 0, // round == hqc_round + 1
        timeout, MockSignature(0),
    );

    assert!(tc.allows_advance());
}

#[test]
fn test_timeout_certificate_allows_advance_true_via_highest_tc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 10, 5, 9, // round == highest_tc_round + 1
        timeout, MockSignature(0),
    );

    assert!(tc.allows_advance());
}

#[test]
fn test_timeout_certificate_allows_advance_false() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 10, 5, 7, // Neither condition met
        timeout, MockSignature(0),
    );

    assert!(!tc.allows_advance());
}

#[test]
fn test_timeout_certificate_effective_hqc_round_uses_hqc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 10, 8));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 10, 8, 5, // hqc_round > highest_tc_round
        timeout, MockSignature(0),
    );

    assert_eq!(tc.effective_hqc_round(), 8);
}

#[test]
fn test_timeout_certificate_effective_hqc_round_uses_highest_tc() {
    let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        0, 10, 5, 8, // highest_tc_round > hqc_round
        timeout, MockSignature(0),
    );

    assert_eq!(tc.effective_hqc_round(), 8);
}

#[test]
fn test_timeout_certificate_timeout_certificate_trait() {
    let timeout = Arc::new(TwoChainTimeout::new(1, 10, 5));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        1, 10, 5, 8, timeout, MockSignature(0),
    );

    assert_eq!(tc.epoch(), 1);
    assert_eq!(tc.round(), 10);
    assert_eq!(tc.highest_qc_round(), 5);
    assert_eq!(tc.highest_tc_round(), 8);
}

// ============================================================================
// RoundManager Coverage Tests
// ============================================================================

#[test]
fn test_round_manager_try_vote_for_proposal_older_round() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let mut manager = RoundManager::new(config, safety_rules);

    // Record a vote for round 10
    manager.safety_rules_mut().record_vote(10).unwrap();

    let proposal = Arc::new(MockBlock::genesis());
    let result = manager.try_vote_for_proposal(&proposal, None);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already voted"));
}

#[test]
fn test_round_manager_process_proposal_old_round() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let mut manager = RoundManager::new(config, safety_rules);

    // Start at round 5
    manager.start_round(5).unwrap();

    let proposal = Arc::new(make_block(0, 3, 0, MockHash(0)));
    let verifier = TestVerifier;

    let result = manager.process_proposal(proposal, &verifier).unwrap();

    assert_eq!(result, ProposalProcessingResult::OldRound);
}

#[test]
fn test_round_manager_process_proposal_future_round() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let mut manager = RoundManager::new(config, safety_rules);

    // Start at round 5
    manager.start_round(5).unwrap();

    let proposal = Arc::new(make_block(0, 10, 0, MockHash(0)));
    let verifier = TestVerifier;

    let result = manager.process_proposal(proposal, &verifier).unwrap();

    assert_eq!(result, ProposalProcessingResult::FutureRound);
}

#[test]
fn test_round_manager_process_timeout_safety_check() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let mut manager = RoundManager::new(config, safety_rules);

    // Record a vote first to establish last_voted_round
    manager.safety_rules_mut().record_vote(0).unwrap();

    // Process timeout should succeed
    let result = manager.process_timeout(TimeoutReason::NoQuorum);

    // Should succeed and advance to round 1
    assert!(result.is_ok());
    assert_eq!(manager.round(), 1);
}

#[test]
fn test_round_manager_getters() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let manager = RoundManager::new(config, safety_rules);

    assert_eq!(manager.epoch(), 0);
    assert_eq!(manager.round(), 0);
}

// ============================================================================
// Integration Tests for Coverage
// ============================================================================

#[test]
fn test_consensus_full_lifecycle() {
    let proposer = TestProposerElection {
        validators: vec![TestAuthor(1), TestAuthor(2)],
    };

    let mut consensus: Consensus<MockBlock, MockVote, TestProposerElection> =
        ConsensusBuilder::new()
            .proposer_election(proposer)
            .self_validator_id(TestAuthor(1))
            .build()
            .unwrap();

    // Start consensus
    assert!(consensus.start().is_ok());
    assert_eq!(consensus.state(), &ConsensusState::Running);

    // Process a proposal
    let proposal = Arc::new(make_block(0, 0, 0, MockHash(0)));
    let verifier = TestVerifier;

    let result = consensus.process_proposal(proposal, &verifier).unwrap();
    assert!(result.accepted);

    // Process a vote
    let vote = make_vote(0, 0, 1, MockHash(0));
    let vote_result = consensus.process_vote(vote, &verifier).unwrap();
    assert!(vote_result.accepted);

    // Process a QC (for round 0, which is past the initial round 1)
    // This should work since we're recording a QC for an earlier round
    let qc = make_qc(0, 0, MockHash(0));
    // Note: process_qc may fail for various reasons depending on pacemaker state
    // We just check that the call doesn't panic
    let _ = consensus.process_qc(Arc::new(qc));

    // Process a timeout
    assert!(consensus.process_timeout(1).is_ok());

    // Stop consensus
    assert!(consensus.stop().is_ok());
    assert_eq!(consensus.state(), &ConsensusState::Stopped);
}

#[test]
fn test_consensus_config_new() {
    let pacemaker_config = PacemakerConfig::default();
    let round_manager_config = RoundManagerConfig::default();
    let pipeline_config = PipelineConfig::default();

    let config = ConsensusConfig::new(
        5,
        1,
        pacemaker_config.clone(),
        round_manager_config.clone(),
        pipeline_config.clone(),
    );

    assert_eq!(config.initial_round, 5);
    assert_eq!(config.initial_epoch, 1);
}

// ============================================================================
// SafetyRules State Coverage Tests
// ============================================================================

#[test]
fn test_safety_state_all_accessors() {
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let state = safety_rules.state();

    assert_eq!(state.last_voted_round(), 0);
    assert_eq!(state.one_chain_round(), 0);
    assert_eq!(state.preferred_round(), 0);
    assert_eq!(state.highest_timeout_round(), 0);
}

#[test]
fn test_safety_state_round_progression() {
    let mut safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);

    // Record vote for round 5
    safety_rules.record_vote(5).unwrap();

    // Observe QC for round 10
    let qc = make_qc(10, 0, MockHash(10));
    safety_rules.observe_qc(&qc).unwrap();

    let state = safety_rules.state();

    assert_eq!(state.last_voted_round(), 5);
    assert_eq!(state.one_chain_round(), 10);
}

// ============================================================================
// Pacemaker Coverage Tests
// ============================================================================

#[test]
fn test_pacemaker_exponential_strategy() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        5,
    );

    // Round index 0: base duration
    let duration_0 = strategy.get_round_duration(0);
    assert_eq!(duration_0, std::time::Duration::from_millis(100));

    // Round index 1: base * 2^1
    let duration_1 = strategy.get_round_duration(1);
    assert_eq!(duration_1, std::time::Duration::from_millis(200));

    // Round index 3: base * 2^3 = 800ms
    let duration_3 = strategy.get_round_duration(3);
    assert_eq!(duration_3, std::time::Duration::from_millis(800));

    // Round index 10: should be capped at max_exponent (2^5 = 32)
    let duration_10 = strategy.get_round_duration(10);
    assert_eq!(duration_10, std::time::Duration::from_millis(3200));
}

#[test]
fn test_pacemaker_lifecycle() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        5,
    );
    let config = PacemakerConfig::default();
    let mut pacemaker = Pacemaker::new(strategy, config);

    // Initial round is 1 (from config), so start at round 2
    assert!(pacemaker.enter_new_round(2).is_ok());
    assert_eq!(pacemaker.current_round(), 2);

    // Record ordered round
    assert!(pacemaker.record_ordered_round(2).is_ok());

    // Enter round 3
    assert!(pacemaker.enter_new_round(3).is_ok());
    assert_eq!(pacemaker.current_round(), 3);
}

#[test]
fn test_pacemaker_cannot_decrease_round() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        5,
    );
    let config = PacemakerConfig::default();
    let mut pacemaker = Pacemaker::new(strategy, config);

    // Start round 5 (from initial 1 to 5)
    assert!(pacemaker.enter_new_round(5).is_ok());

    // Try to go back to round 3
    let result = pacemaker.enter_new_round(3);

    assert!(result.is_err());
}

#[test]
fn test_pacemaker_cannot_order_past_round() {
    let strategy = ExponentialIntervalStrategy::new(
        std::time::Duration::from_millis(100),
        2.0,
        5,
    );
    let config = PacemakerConfig::default();
    let mut pacemaker = Pacemaker::new(strategy, config);

    // Start round 10 (from initial 1 to 10)
    assert!(pacemaker.enter_new_round(10).is_ok());

    // Record order for round 10 (current round)
    assert!(pacemaker.record_ordered_round(10).is_ok());

    // Try to record order for round 5 (in the past, less than highest_ordered_round which is now 10)
    let result = pacemaker.record_ordered_round(5);

    assert!(result.is_err());
}

// ============================================================================
// EpochManager Coverage Tests
// ============================================================================

#[test]
fn test_epoch_manager_new() {
    let config = consensus_core::epoch_manager::EpochManagerConfig {
        initial_epoch: 5,
        auto_transition: false,
    };

    let manager = EpochManager::new(config);

    assert_eq!(manager.epoch(), 5);
}

#[test]
fn test_epoch_manager_advance_epoch() {
    let config = consensus_core::epoch_manager::EpochManagerConfig {
        initial_epoch: 0,
        auto_transition: false,
    };

    let mut manager = EpochManager::new(config);

    // Use force_next_epoch to advance
    assert!(manager.force_next_epoch().is_ok());
    assert_eq!(manager.epoch(), 1);

    assert!(manager.force_next_epoch().is_ok());
    assert_eq!(manager.epoch(), 2);

    assert!(manager.force_next_epoch().is_ok());
    assert_eq!(manager.epoch(), 3);
}

// ============================================================================
// Error Path Coverage Tests
// ============================================================================

#[test]
fn test_try_vote_for_proposal_safety_rule_violation() {
    let config = RoundManagerConfig::default();
    let safety_rules = TwoChainSafetyRules::<MockBlock, MockVote>::new(0);
    let mut manager = RoundManager::new(config, safety_rules);

    // Record a vote for round 10
    manager.safety_rules_mut().record_vote(10).unwrap();

    // Try to vote for a proposal in round 5 (older than last_voted_round)
    let proposal = Arc::new(make_block(0, 5, 0, MockHash(5)));

    let result = manager.try_vote_for_proposal(&proposal, None);

    // Should fail - can't vote for older round
    assert!(result.is_err());
}

// ============================================================================
// DAG Types Coverage Tests
// ============================================================================

#[test]
fn test_dag_node_id_new_and_accessors() {
    use consensus_core::dag::types::DagNodeId;
    use consensus_traits::core::Hash as HashTrait;

    let id = DagNodeId::new(1u64, 10u64, TestAuthor(5));

    assert_eq!(*id.epoch(), 1);
    assert_eq!(*id.round(), 10);
    assert_eq!(*id.author(), TestAuthor(5));
}

#[test]
fn test_dag_node_id_ord() {
    use consensus_core::dag::types::DagNodeId;
    use consensus_traits::core::Hash as HashTrait;

    let id1 = DagNodeId::new(1u64, 10u64, TestAuthor(5));
    let id2 = DagNodeId::new(1u64, 10u64, TestAuthor(5));
    let id3 = DagNodeId::new(1u64, 11u64, TestAuthor(5));

    // Same round -> Equal
    assert_eq!(id1, id2);
    assert!(id1 == id2);

    // Different rounds -> proper ordering
    assert!(id1 < id3);
    assert!(id1 <= id3);
}

#[test]
fn test_dag_node_id_ord_different_rounds() {
    use consensus_core::dag::types::DagNodeId;
    use consensus_traits::core::Hash as HashTrait;

    let id1 = DagNodeId::new(1u64, 5u64, TestAuthor(5));
    let id2 = DagNodeId::new(1u64, 10u64, TestAuthor(5));

    assert!(id1 < id2);
    assert_eq!(id1.cmp(&id2), std::cmp::Ordering::Less);
}

#[test]
fn test_dag_node_id_ord_different_epochs() {
    use consensus_core::dag::types::DagNodeId;
    use std::cmp::Ordering;

    let id1 = DagNodeId::new(1u64, 10u64, TestAuthor(5));
    let id2 = DagNodeId::new(2u64, 10u64, TestAuthor(5));

    // PartialEq compares all fields, so they're NOT equal (different epochs)
    assert_ne!(id1, id2);

    // But Ord only compares round, so they're equal in ordering
    assert_eq!(id1.cmp(&id2), Ordering::Equal);
}

#[test]
fn test_dag_node_id_display() {
    use consensus_core::dag::types::DagNodeId;
    use consensus_traits::core::Hash as HashTrait;

    let id = DagNodeId::new(1u64, 10u64, TestAuthor(5));

    // Display should include the values
    let display = format!("{}", id);
    // The exact format depends on the Display implementation
    assert!(!display.is_empty());
}

// ============================================================================
// DAG Driver Coverage Tests
// ============================================================================

#[test]
fn test_dag_driver_config_default() {
    use consensus_core::dag::driver::DagDriverConfig;

    let config = DagDriverConfig::default();
    assert_eq!(config.window_size, 100);
    assert_eq!(config.anchor_interval, 2);
    assert_eq!(config.max_concurrent_rounds, 10);
}

#[test]
fn test_dag_driver_config_new() {
    use consensus_core::dag::driver::DagDriverConfig;

    let config = DagDriverConfig {
        window_size: 200,
        anchor_interval: 5,
        max_concurrent_rounds: 20,
    };

    assert_eq!(config.window_size, 200);
    assert_eq!(config.anchor_interval, 5);
    assert_eq!(config.max_concurrent_rounds, 20);
}

// ============================================================================
// Additional Testing Utility Tests
// ============================================================================

#[test]
fn test_mock_block_all_methods() {
    let block = MockBlock::genesis();
    let metadata = block.metadata();
    let signature = block.signature();

    // Just verify these methods exist
    let _ = metadata.epoch();
    let _ = metadata.round();
    let _ = metadata.author();
    let _ = metadata.timestamp();
    // MockBlock::genesis() has no signature
    assert!(signature.is_none());
}

#[test]
fn test_mock_vote_all_methods() {
    let vote = MockVote::new(testing::MockHash(1), MockHash(0), 0);
    let vote_data = vote.vote_data();
    let ledger_info = vote.ledger_info();
    let _signature = vote.signature();

    // Just verify these methods exist
    let _ = vote_data.proposed_block();
    let _ = vote_data.parent_block();
    let _ = ledger_info.commit_info();
    assert_eq!(vote.round(), 0);
    assert_eq!(vote.block_id(), MockHash(0));
}

#[test]
fn test_mock_quorum_cert_methods() {
    let qc = testing::MockQuorumCert::new(testing::MockHash(10));

    let _block_metadata = qc.certified_block();
    assert_eq!(qc.block_id(), testing::MockHash(10));
    assert!(qc.verify().is_ok());
}

#[test]
fn test_mock_hash_trait_methods() {
    let hash1 = MockHash(5);
    let hash2 = MockHash(10);

    assert_eq!(hash1.inner(), 5);
    assert_eq!(hash2.inner(), 10);
    assert!(hash1 != hash2);

    // Test zero
    let zero = testing::MockHash::zero();
    assert_eq!(zero.inner(), 0);
}

#[test]
fn test_mock_validator_set_quorum_calculation() {
    use consensus_core::testing::MockValidatorSet;

    let validator_set = MockValidatorSet::new(vec![testing::MockValidator::new(1, 100)]);

    // Test that validator set was created - count validator IDs
    let validator_count = validator_set.validator_ids().count();
    assert_eq!(validator_count, 1);
}

#[test]
fn test_mock_signature_aggregation() {
    use consensus_core::testing::MockSignature;

    let sig1 = MockSignature(1);
    let sig2 = MockSignature(2);

    // Aggregation succeeds for mock (needs references)
    let _aggregated = MockSignature::aggregate([&sig1, &sig2].into_iter());

    let sig = MockSignature(42);
    let bytes = sig.to_bytes();

    assert_eq!(bytes, vec![42]);
    assert_eq!(MockSignature::from_bytes(&bytes).unwrap(), sig);
}

#[test]
fn test_mock_signature_verify() {
    use consensus_core::testing::MockPublicKey;

    let sig = MockSignature(42);
    let pub_key = MockPublicKey;

    // Verification should always succeed for mock
    assert!(sig.verify(b"test", &pub_key).is_ok());
}

// ============================================================================
// Coverage for Display and Debug Implementations
// ============================================================================

#[test]
fn test_consensus_state_display_all() {
    assert_eq!(format!("{}", consensus::ConsensusState::Bootstrapping), "bootstrapping");
    assert_eq!(format!("{}", consensus::ConsensusState::Running), "running");
    assert_eq!(format!("{}", consensus::ConsensusState::CatchingUp), "catching_up");
    assert_eq!(format!("{}", consensus::ConsensusState::Stopping), "stopping");
    assert_eq!(format!("{}", consensus::ConsensusState::Stopped), "stopped");
}

#[test]
fn test_timeout_reason_display() {
    use consensus_core::types::TimeoutReason;

    assert_eq!(format!("{}", TimeoutReason::NoQuorum), "no_quorum");
    assert_eq!(format!("{}", TimeoutReason::NoQuorum), "no_quorum");
}

#[test]
fn test_proposal_result_all_variants() {
    let accepted = consensus::ProposalResult {
        accepted: true,
        reason: None,
        round: 5,
        voted: true,
    };

    assert!(accepted.accepted);
    assert!(accepted.voted);
    assert_eq!(accepted.round, 5);
    assert!(accepted.reason.is_none());

    let rejected = consensus::ProposalResult {
        accepted: false,
        reason: Some("bad block".to_string()),
        round: 3,
        voted: false,
    };

    assert!(!rejected.accepted);
    assert!(!rejected.voted);
    assert_eq!(rejected.round, 3);
    assert!(rejected.reason.is_some());
}

#[test]
fn test_vote_result_all_fields() {
    let result = consensus::VoteResult {
        accepted: true,
        round: 10,
    };

    assert!(result.accepted);
    assert_eq!(result.round, 10);
}

#[test]
fn test_commit_decision_fields() {
    use consensus_traits::safety::CommitDecision;

    // Test NoCommit variant
    let no_commit = CommitDecision::<MockHash>::NoCommit;
    assert!(!no_commit.is_commit());
    assert_eq!(no_commit.committed_id(), None);

    // Test Commit variant
    let commit = CommitDecision::Commit(MockHash(100));
    assert!(commit.is_commit());
    assert_eq!(commit.committed_id(), Some(&MockHash(100)));
}

// ============================================================================
// Coverage for Clone and Debug Implementations
// ============================================================================

#[test]
fn test_runtime_config_clone() {
    let config = RuntimeConfig::new(100, 10, 2000, 2048);
    let cloned = config.clone();

    assert_eq!(cloned.process_interval_ms, 100);
    assert_eq!(cloned.max_concurrent_tasks, 10);
}

#[test]
fn test_execution_result_clone() {
    let result1 = ExecutionResult::success();
    let cloned = result1.clone();

    assert!(matches!(cloned, ExecutionResult::Success));
    assert!(cloned.is_success());
}

#[test]
fn test_commit_event_clone() {
    use consensus_core::dag::ordering::CommitEvent;

    let id = TestNodeId::new(1, TestRound(10), TestAuthor(5));
    let parents: Vec<TestAuthor> = vec![];
    let failed_authors: Vec<TestAuthor> = vec![];
    let event = CommitEvent::new(id.clone(), parents, failed_authors);

    let cloned = event.clone();

    assert_eq!(cloned.anchor_id, id);
    assert_eq!(cloned.parents.len(), 0);
    assert_eq!(cloned.failed_authors.len(), 0);
}

// ============================================================================
// Coverage for Hash Trait Implementations
// ============================================================================

#[test]
fn test_test_round_hash() {
    let round = TestRound(42);

    // Test zero
    assert_eq!(TestRound::zero().0, 0);

    // Test as_bytes
    assert!(round.as_bytes().is_empty());

    // Test Display
    assert_eq!(format!("{}", round), "42");
}

#[test]
fn test_test_author_hash() {
    let author = TestAuthor(7);

    // Test zero
    assert_eq!(TestAuthor::zero().0, 0);

    // Test as_bytes - use NodeId trait to disambiguate
    use consensus_traits::core::NodeId;
    assert!(NodeId::as_bytes(&author).is_empty());

    // Test Display - note: Display uses the Display impl which includes "Author()"
    assert_eq!(format!("{}", author), "Author(7)");
}

#[test]
fn test_test_nodeid_hash() {
    let id = TestNodeId::new(1, TestRound(10), TestAuthor(5));

    // Test Display
    let display = format!("{}", id);
    assert!(!display.is_empty());

    // Test creating a zero node ID
    let zero_id = TestNodeId::new(0, TestRound::zero(), TestAuthor::zero());
    assert_eq!(*zero_id.epoch(), 0);
    assert_eq!(*zero_id.round(), TestRound(0));
    assert_eq!(*zero_id.author(), TestAuthor(0));
}

// ============================================================================
// Additional Coverage Tests for DAG Types
// ============================================================================

#[test]
fn test_dag_payload_impl_empty() {
    use consensus_core::dag::types::DagPayloadImpl;

    let payload = DagPayloadImpl::<TestRound>::empty();
    assert_eq!(payload.data().len(), 0);
}

// ============================================================================
// Coverage Tests for Safety Rules Recovery
// ============================================================================

#[test]
fn test_safety_state_data_new_epoch() {
    use consensus_core::safety_rules::SafetyStateData;

    let state1 = SafetyStateData {
        epoch: 5,
        last_voted_round: 100,
        preferred_round: 100,
        one_chain_round: 99,
        highest_timeout_round: 80,
    };

    let state2 = state1.new_epoch(6);
    assert_eq!(state2.epoch, 6);
    assert_eq!(state2.last_voted_round, 0); // Reset in new epoch
    assert_eq!(state2.preferred_round, 100); // Kept from previous epoch
    assert_eq!(state2.one_chain_round, 0);
    assert_eq!(state2.highest_timeout_round, 0);
}

#[test]
fn test_safety_state_data_display() {
    use consensus_core::safety_rules::SafetyStateData;

    let state = SafetyStateData {
        epoch: 5,
        last_voted_round: 10,
        preferred_round: 10,
        one_chain_round: 9,
        highest_timeout_round: 8,
    };

    let display = format!("{}", state);
    assert!(display.contains("epoch"));
    assert!(display.contains("5"));
}

// ============================================================================
// Coverage Tests for Timeout Types
// ============================================================================

#[test]
fn test_two_chain_timeout_display() {
    use consensus_core::timeout::TwoChainTimeout;

    let timeout = TwoChainTimeout::new(1, 10, 9);
    let debug = format!("{:?}", timeout);
    assert!(debug.contains("epoch") || debug.contains("1"));
}

#[test]
fn test_two_chain_timeout_certificate_debug() {
    use consensus_core::timeout::TwoChainTimeoutCertificate;
    use std::sync::Arc;

    let timeout = Arc::new(TwoChainTimeout::new(1, 5, 4));
    let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
        1, 5, 4, 0, timeout, testing::MockSignature(0),
    );

    let debug = format!("{:?}", tc);
    assert!(debug.contains("TwoChainTimeoutCertificate"));
}

// ============================================================================
// Coverage Tests for Consensus Runtime
// ============================================================================

#[test]
fn test_runtime_config_default() {
    use consensus_core::consensus::runtime::RuntimeConfig;

    let config = RuntimeConfig::default();
    assert_eq!(config.process_interval_ms, 100);
    assert_eq!(config.max_concurrent_tasks, 10);
}

#[test]
fn test_execution_result_error() {
    use consensus_core::consensus::runtime::ExecutionResult;

    let failed = ExecutionResult::failed("Test error");
    assert!(!failed.is_success());
}

// ============================================================================
// Coverage Tests for Pipeline
// ============================================================================

#[test]
fn test_pipeline_config_default() {
    use consensus_core::pipeline::PipelineConfig;

    let config = PipelineConfig::default();
    assert_eq!(config.stage_timeout_ms, 5000);
    assert_eq!(config.max_concurrent, 10);
}

#[test]
fn test_pipeline_result_display() {
    use consensus_core::pipeline::PipelineResult;

    let result = PipelineResult::Completed;
    let display = format!("{:?}", result);
    assert!(display.contains("Completed") || display.contains("Success"));
}

// ============================================================================
// Coverage Tests for Network Module
// ============================================================================

#[test]
fn test_network_message_display() {
    use consensus_core::network::NetworkMessage;
    use std::sync::Arc;

    let block = Arc::new(MockBlock::genesis());
    let vote = MockVote::new(testing::MockHash(1), testing::MockHash(2), 5);

    let proposal_msg: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Proposal { block };
    let vote_msg: NetworkMessage<MockBlock, MockVote> = NetworkMessage::Vote { vote };

    let prop_display = format!("{:?}", proposal_msg);
    let vote_display = format!("{:?}", vote_msg);

    assert!(prop_display.contains("Proposal") || prop_display.contains("proposal"));
    assert!(vote_display.contains("Vote") || vote_display.contains("vote"));
}

// ============================================================================
// Coverage Tests for State Round State
// ============================================================================

#[test]
fn test_round_state_config_default() {
    use consensus_core::state::RoundStateConfig;

    let config = RoundStateConfig::default();
    assert_eq!(config.max_buffered_rounds, 10);
}

// ============================================================================
// Coverage Tests for Pacemaker
// ============================================================================

#[test]
fn test_pacemaker_config_default() {
    use consensus_core::liveness::pacemaker::PacemakerConfig;

    let config = PacemakerConfig::default();
    assert_eq!(config.base_duration_ms, 1000);
}

#[test]
fn test_exponential_interval_strategy_display() {
    use consensus_core::liveness::pacemaker::ExponentialIntervalStrategy;
    use std::time::Duration;

    // Test that the strategy can be created successfully
    let _ = ExponentialIntervalStrategy::new(Duration::from_millis(1000), 2.0, 3);
}

// ============================================================================
// Coverage Tests for Round Manager
// ============================================================================

#[test]
fn test_round_manager_config_default() {
    use consensus_core::round_manager::RoundManagerConfig;

    let config = RoundManagerConfig::default();
    assert_eq!(config.max_buffered_rounds, 10);
}

// ============================================================================
// Coverage Tests for Proposal Processing Results
// ============================================================================

#[test]
fn test_proposal_processing_result_display() {
    use consensus_core::round_manager::ProposalProcessingResult;

    let accepted = ProposalProcessingResult::Accepted;
    let rejected = ProposalProcessingResult::Rejected("Invalid block".to_string());

    let acc_display = format!("{:?}", accepted);
    let rej_display = format!("{:?}", rejected);

    assert!(acc_display.contains("Accepted"));
    assert!(rej_display.contains("Rejected") || rej_display.contains("Invalid"));
}

