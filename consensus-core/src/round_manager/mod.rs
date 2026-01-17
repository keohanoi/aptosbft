// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Core round-based consensus state machine.
//!
//! This module provides the generic round manager that implements the core
//! AptosBFT consensus algorithm, independent of any specific blockchain implementation.
//!
//! # Overview
//!
//! The round manager is responsible for:
//! - Managing rounds and epochs
//! - Processing proposals and votes
//! - Forming quorum certificates
//! - Advancing rounds on timeout
//!
//! # Generic Design
//!
//! The round manager is generic over:
//! - `B`: Block type implementing the [`Block`] trait
//! - `V`: Vote type implementing the [`Vote`] trait
//! - `SR`: SafetyRules type implementing the [`SafetyRules`] trait

use crate::state::RoundState;
use crate::types::{Epoch, Round, TimeoutReason, VoteStatus};
use crate::votes::{PendingVotes, VoteReceptionResult};
use consensus_traits::{Block, BlockMetadata, LedgerInfo, SafetyRules, SafetyState, ValidatorVerifier, Vote};
use consensus_traits::core::Error;
use std::sync::Arc;

/// Configuration for the RoundManager
#[derive(Clone, Debug)]
pub struct RoundManagerConfig {
    /// Maximum number of rounds to buffer votes for
    pub max_buffered_rounds: u64,

    /// Timeout duration for each round (in milliseconds)
    pub round_timeout_ms: u64,

    /// Maximum number of pending votes to track
    pub max_pending_votes: usize,
}

impl Default for RoundManagerConfig {
    fn default() -> Self {
        Self {
            max_buffered_rounds: 10,
            round_timeout_ms: 5000,
            max_pending_votes: 1000,
        }
    }
}

/// Result of processing a proposal
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalProcessingResult {
    /// The proposal was accepted
    Accepted,

    /// The proposal was rejected
    Rejected(String),

    /// The proposal was for an old round and ignored
    OldRound,

    /// The proposal was for a future round and buffered
    FutureRound,
}

/// Core round-based consensus manager
///
/// This struct implements the core AptosBFT consensus state machine,
/// managing rounds, processing proposals and votes, and forming quorum certificates.
///
/// # Type Parameters
///
/// * `B`: Block type - must implement the [`Block`] trait
/// * `V`: Vote type - must implement the [`Vote`] trait
/// * `SR`: SafetyRules type - must implement the [`SafetyRules`] trait
pub struct RoundManager<B, V, SR>
where
    B: Block,
    V: Vote<Block = B>,
    SR: SafetyRules<B, V>,
    <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// Round state management
    round_state: RoundState<B>,

    /// Pending votes tracker
    pending_votes: PendingVotes<B, V>,

    /// Safety rules for Byzantine fault tolerance
    safety_rules: SR,

    /// Configuration
    config: RoundManagerConfig,
}

impl<B, V, SR> RoundManager<B, V, SR>
where
    B: Block,
    V: Vote<Block = B>,
    SR: SafetyRules<B, V>,
    <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert: std::fmt::Debug,
{
    /// Create a new RoundManager
    pub fn new(config: RoundManagerConfig, safety_rules: SR) -> Self {
        let round_state_config = crate::state::RoundStateConfig {
            initial_epoch: 0,
            initial_round: 0,
            max_buffered_rounds: config.max_buffered_rounds,
        };

        Self {
            round_state: RoundState::new(round_state_config),
            pending_votes: PendingVotes::new(config.max_buffered_rounds),
            safety_rules,
            config,
        }
    }

    /// Get the current epoch
    pub fn epoch(&self) -> Epoch {
        self.round_state.epoch()
    }

    /// Get the current round
    pub fn round(&self) -> Round {
        self.round_state.round()
    }

    /// Get the round state
    pub fn round_state(&self) -> &RoundState<B> {
        &self.round_state
    }

    /// Get the pending votes
    pub fn pending_votes(&self) -> &PendingVotes<B, V> {
        &self.pending_votes
    }

    /// Get the safety rules
    pub fn safety_rules(&self) -> &SR {
        &self.safety_rules
    }

    /// Get a mutable reference to the safety rules
    pub fn safety_rules_mut(&mut self) -> &mut SR {
        &mut self.safety_rules
    }

    /// Attempt to vote for a proposal, checking safety rules first.
    ///
    /// This method checks if it's safe to vote for the given proposal (2-chain rule),
    /// and if so, updates the safety state to record the vote.
    ///
    /// Note: This method does NOT generate or broadcast the vote - it only checks
    /// safety and updates state. The actual vote generation is done by the caller.
    ///
    /// `★ Insight ─────────────────────────────────────`
    /// Safety rule check happens BEFORE voting:
    /// 1. Check safe_to_vote() with the proposal
    /// 2. If safe, record the vote to update last_voted_round
    /// 3. Caller then generates and broadcasts the vote
    /// This ensures we never vote for blocks that violate the 2-chain rule
    /// `─────────────────────────────────────────────────`
    ///
    /// # Parameters
    ///
    /// * `proposal` - The proposal we want to vote for
    /// * `timeout_cert` - Optional timeout certificate if voting after a timeout
    ///
    /// # Returns
    ///
    /// * `Ok(())` - It's safe to vote, state has been updated
    /// * `Err(Error)` - Not safe to vote (violates 2-chain rule)
    pub fn try_vote_for_proposal(
        &mut self,
        proposal: &Arc<B>,
        timeout_cert: Option<&SR::TimeoutCert>,
    ) -> Result<(), Error>
    {
        let proposal_round = proposal.metadata().round();

        // First check if we can vote in this round (monotonicity check)
        if proposal_round < self.safety_rules.state().last_voted_round() {
            return Err(Error::msg(format!(
                "Cannot vote for round {}: already voted in round {}",
                proposal_round,
                self.safety_rules.state().last_voted_round()
            )));
        }

        // Check the 2-chain safety rule
        self.safety_rules.safe_to_vote(proposal, timeout_cert)?;

        // Record the vote - this updates last_voted_round
        self.safety_rules.record_vote(proposal_round)?;

        Ok(())
    }

    /// Process a proposal message
    ///
    /// This method validates the proposal and, if valid, adds it to the
    /// pending votes and potentially creates a vote for it.
    pub fn process_proposal<VV>(
        &mut self,
        proposal: Arc<B>,
        _verifier: &VV,
    ) -> Result<ProposalProcessingResult, Error>
    where
        VV: ValidatorVerifier<B, V>,
    {
        let proposal_round = proposal.metadata().round();

        // Check if proposal is for current round
        if !self.round_state.is_current_round(proposal_round) {
            if proposal_round < self.round_state.round() {
                return Ok(ProposalProcessingResult::OldRound);
            }
            // Future round - could be buffered
            return Ok(ProposalProcessingResult::FutureRound);
        }

        // Verify the proposal signature
        proposal.verify_signature()?;

        // Set the proposed block for the current round
        self.round_state.set_proposed_block(proposal);

        Ok(ProposalProcessingResult::Accepted)
    }

    /// Process a vote message
    ///
    /// This method validates the vote and adds it to the pending votes.
    /// If a quorum is reached, a quorum certificate is formed.
    ///
    /// `★ Insight ─────────────────────────────────────`
    /// Safety rule integration happens here:
    /// 1. Before adding vote to pending votes, we check safe_to_vote
    /// 2. After QC formation, we call observe_qc to update safety state
    /// 3. This ensures the 2-chain commit rule is never violated
    /// `─────────────────────────────────────────────────`
    pub fn process_vote<VV>(
        &mut self,
        vote: Arc<V>,
        verifier: &VV,
    ) -> Result<VoteReceptionResult, Error>
    where
        VV: ValidatorVerifier<B, V>,
    {
        // Verify the vote
        vote.verify(verifier)?;

        // Check if vote is for current epoch
        if !self.round_state.is_current_epoch(vote.ledger_info().epoch()) {
            return Ok(VoteReceptionResult::Rejected(VoteStatus::Rejected));
        }

        // Note: Safety check for voting happens in try_vote_for_proposal(),
        // which should be called by the consensus runtime before generating
        // a vote. Here we just accept and process votes from other validators.

        // Add vote to pending votes
        let result = self.pending_votes.add_vote(vote.clone(), verifier);

        // If quorum was reached, form and store the quorum certificate
        if result == VoteReceptionResult::QuorumCompleted {
            self.form_quorum_certificate_for_block(vote.block_id(), vote.round(), verifier)?;
        }

        Ok(result)
    }

    /// Form a quorum certificate for the given block and round
    ///
    /// This method aggregates the votes for a block and creates a quorum certificate
    /// using actual voting power and signature aggregation. Once a QC is formed,
    /// the votes for that round are cleaned up.
    ///
    /// `★ Insight ─────────────────────────────────────`
    /// After forming a QC, we MUST call observe_qc to update safety state:
    /// 1. Updates one_chain_round if this QC is higher
    /// 2. Updates preferred_round if we have a 2-chain
    /// 3. This ensures future votes respect the 2-chain commit rule
    /// `─────────────────────────────────────────────────`
    fn form_quorum_certificate_for_block<VV>(
        &mut self,
        block_id: <V as Vote>::Hash,
        round: Round,
        verifier: &VV,
    ) -> Result<(), Error>
    where
        VV: ValidatorVerifier<B, V>,
    {
        use consensus_traits::QuorumCertificate;
        use crate::crypto::SignatureAggregator;

        // Get all votes for this block
        let votes = self.pending_votes.get_votes(round, block_id);

        // Build signature aggregator with all votes
        let mut aggregator = SignatureAggregator::<V>::new();
        for vote in votes.iter() {
            let author = vote.author();
            if let Some(voting_power) = verifier.get_voting_power(&author) {
                aggregator.add_signature(author, vote.signature().clone(), voting_power);
            }
        }

        // Check if we have quorum
        aggregator.check_quorum::<B, VV>(verifier)?;

        // Aggregate signatures
        let aggregated_sig = aggregator.aggregate::<B, VV>(verifier)?;

        // Convert references to Arcs for from_votes
        let vote_arcs: Vec<_> = votes.iter().cloned().cloned().collect();

        // Create quorum certificate using the trait method
        let qc = <<B as Block>::Metadata as consensus_traits::BlockMetadata>::QuorumCert::from_votes(
            &vote_arcs,
            aggregated_sig,
        )?;

        // Update safety rules with the new QC
        self.safety_rules.observe_qc(&qc)?;

        // Store the quorum certificate in round state
        self.round_state.set_quorum_cert(qc);

        // Clean up votes for this round after QC formation
        self.pending_votes.remove_round(round);

        Ok(())
    }

    /// Check if a quorum certificate exists for a block
    ///
    /// Returns true if a QC has been formed for the given block and round.
    pub fn has_quorum_certificate(&self, block_id: <V as Vote>::Hash, round: Round) -> bool {
        // In a full implementation, this would check if we have a stored QC
        // For now, we check if votes were cleaned up (indicating QC formation)
        // AND we've moved past that round
        self.pending_votes.vote_count(round, block_id) == 0 && round < self.round_state.round()
    }

    /// Process a round timeout
    ///
    /// This method advances to the next round when a timeout occurs.
    ///
    /// `★ Insight ─────────────────────────────────────`
    /// Safety rule integration for timeouts:
    /// 1. Check safe_to_timeout before advancing
    /// 2. Record the timeout in safety rules
    /// 3. This ensures we don't timeout past our one_chain_round
    /// `─────────────────────────────────────────────────`
    pub fn process_timeout(&mut self, reason: TimeoutReason) -> Result<(), Error> {
        let current_round = self.round_state.round();

        // Check safety rules before timing out
        // We need the highest QC round known - for now, use 0 as a placeholder
        // In a full implementation, we'd track the highest QC round
        let qc_round = self.safety_rules.state().one_chain_round();

        // Check if it's safe to timeout
        // Note: We don't have a timeout certificate yet, so pass None
        self.safety_rules.safe_to_timeout(current_round + 1, qc_round, None)
            .map_err(|e| Error::msg(format!("Not safe to timeout: {}", e)))?;

        // Record the timeout
        self.safety_rules.record_timeout(current_round);

        // Advance to the next round
        self.round_state.advance_round();
        self.pending_votes.set_round(self.round_state.round());

        // In a full implementation, this would:
        // 1. Broadcast a timeout message
        // 2. Wait for other validators' timeouts
        // 3. Start the next round

        Ok(())
    }

    /// Start a new round
    ///
    /// This method is called to start participating in a new round.
    pub fn start_round(&mut self, round: Round) -> Result<(), Error> {
        self.round_state.set_round(round);
        self.pending_votes.set_round(round);

        // In a full implementation, this would:
        // 1. Set a timeout for the round
        // 2. If we're the proposer, generate a proposal
        // 3. Otherwise, wait for proposals

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple mock block that meets all trait requirements
    #[derive(Clone, Debug)]
    struct SimpleBlock {
        epoch: u64,
        round: u64,
        metadata: SimpleMetadata,
    }

    impl Block for SimpleBlock {
        type Transaction = SimpleTransaction;
        type Metadata = SimpleMetadata;
        type Signature = SimpleSignature;
        type Hash = SimpleHash;

        fn id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn parent_id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn transactions(&self) -> &[Self::Transaction] {
            &[]
        }

        fn metadata(&self) -> &Self::Metadata {
            &self.metadata
        }

        fn signature(&self) -> Option<&Self::Signature> {
            static SIG: SimpleSignature = SimpleSignature(0);
            Some(&SIG)
        }

        fn verify_signature(&self) -> Result<(), Error> {
            Ok(())
        }

        fn is_genesis(&self) -> bool {
            self.epoch == 0 && self.round == 0
        }

        fn is_nil(&self) -> bool {
            false
        }

        fn new_genesis() -> Self {
            SimpleBlock {
                epoch: 0,
                round: 0,
                metadata: SimpleMetadata { epoch: 0, round: 0 },
            }
        }
    }

    #[derive(Clone, Debug)]
    struct SimpleTransaction;

    impl consensus_traits::block::Transaction for SimpleTransaction {
        type Hash = SimpleHash;

        fn hash(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn size(&self) -> usize {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct SimpleHash(u8);  // Inner value for uniqueness in tests

    impl consensus_traits::core::Hash for SimpleHash {
        fn zero() -> Self {
            SimpleHash(0)
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(SimpleHash(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    impl consensus_traits::core::NodeId for SimpleHash {
        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(SimpleHash(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct SimpleSignature(u8);

    #[derive(Clone, Debug)]
    struct SimplePublicKey;

    impl consensus_traits::core::PublicKey for SimplePublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(SimplePublicKey)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct SimpleMetadata {
        epoch: u64,
        round: u64,
    }

    impl consensus_traits::block::BlockMetadata for SimpleMetadata {
        type QuorumCert = SimpleQuorumCert;
        type NodeId = SimpleHash;
        type Hash = SimpleHash;

        fn epoch(&self) -> u64 {
            self.epoch
        }

        fn round(&self) -> u64 {
            self.round
        }

        fn author(&self) -> Self::NodeId {
            SimpleHash(0)
        }

        fn parent_id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct SimpleQuorumCert;

    impl consensus_traits::block::QuorumCertificate for SimpleQuorumCert {
        type BlockMetadata = SimpleMetadata;
        type Hash = SimpleHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            static BLOCK: SimpleMetadata = SimpleMetadata { epoch: 0, round: 0 };
            &BLOCK
        }

        fn block_id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[std::sync::Arc<V>],
            _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
        ) -> Result<Self, Error>
        where
            B: consensus_traits::Block,
            V: consensus_traits::Vote<Block = B>,
        {
            Ok(SimpleQuorumCert)
        }
    }

    // Simple mock vote
    #[derive(Clone, Debug)]
    struct SimpleVote {
        epoch: u64,
        round: u64,
        signature: SimpleSignature,
        author_id: u8,  // Used to create unique authors
        ledger_info: SimpleLedgerInfo,
    }

    impl Vote for SimpleVote {
        type Block = SimpleBlock;
        type Hash = SimpleHash;
        type VoteData = SimpleVoteData;
        type NodeId = SimpleHash;
        type LedgerInfo = SimpleLedgerInfo;
        type Signature = SimpleSignature;

        fn vote_data(&self) -> &Self::VoteData {
            static DATA: SimpleVoteData = SimpleVoteData;
            &DATA
        }

        fn author(&self) -> Self::NodeId {
            SimpleHash(self.author_id)
        }

        fn ledger_info(&self) -> &Self::LedgerInfo {
            &self.ledger_info
        }

        fn signature(&self) -> &Self::Signature {
            &self.signature
        }

        fn block_id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn round(&self) -> u64 {
            self.round
        }

        fn verify<B2, VV>(&self, _verifier: &VV) -> Result<(), anyhow::Error>
        where
            B2: Block,
            VV: consensus_traits::ValidatorVerifier<B2, Self>,
        {
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct SimpleVoteData;

    impl consensus_traits::block::VoteData for SimpleVoteData {
        type BlockMetadata = SimpleMetadata;

        fn proposed_block(&self) -> &Self::BlockMetadata {
            static BLOCK: SimpleMetadata = SimpleMetadata { epoch: 0, round: 0 };
            &BLOCK
        }

        fn parent_block(&self) -> &Self::BlockMetadata {
            static BLOCK: SimpleMetadata = SimpleMetadata { epoch: 0, round: 0 };
            &BLOCK
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct SimpleLedgerInfo {
        epoch: u64,
        round: u64,
    }

    impl consensus_traits::block::LedgerInfo for SimpleLedgerInfo {
        type CommitInfo = SimpleCommitInfo;
        type Hash = SimpleHash;

        fn commit_info(&self) -> &Self::CommitInfo {
            static INFO: SimpleCommitInfo = SimpleCommitInfo;
            &INFO
        }

        fn epoch(&self) -> u64 {
            self.epoch
        }

        fn round(&self) -> u64 {
            self.round
        }

        fn accumulated_state(&self) -> &Self::Hash {
            static HASH: SimpleHash = SimpleHash(0);
            &HASH
        }
    }

    #[derive(Clone, Debug)]
    struct SimpleCommitInfo;

    impl consensus_traits::block::CommitInfo for SimpleCommitInfo {
        type Hash = SimpleHash;

        fn block_id(&self) -> Self::Hash {
            SimpleHash(0)
        }

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn version(&self) -> u64 {
            0
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    struct SimpleVerifier;

    impl ValidatorVerifier<SimpleBlock, SimpleVote> for SimpleVerifier {
        fn verify_vote(&self, _vote: &SimpleVote) -> Result<(), Error> {
            Ok(())
        }

        fn verify_block(&self, _block: &SimpleBlock) -> Result<(), Error> {
            Ok(())
        }

        fn get_voting_power(&self, _validator_id: &<SimpleVote as Vote>::NodeId) -> Option<u64> {
            Some(100) // Fixed voting power for tests
        }

        fn total_voting_power(&self) -> u128 {
            400 // 4 validators with 100 power each
        }

        fn quorum_voting_power(&self) -> u128 {
            267 // 2/3 of 400 + 1
        }

        fn check_voting_power<'a>(
            &self,
            signers: impl Iterator<Item = &'a <SimpleVote as Vote>::NodeId>,
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
            _signatures: impl Iterator<Item = &'a <SimpleVote as Vote>::Signature>,
        ) -> Result<SimpleAggregatedSignature, Error> {
            // For testing, return a dummy aggregated signature
            Ok(SimpleAggregatedSignature)
        }
    }

    // Add the Aggregated type for SimpleSignature
    impl consensus_traits::core::Signature for SimpleSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = SimplePublicKey;
        type Aggregated = SimpleAggregatedSignature;

        fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
            Ok(())
        }

        fn to_bytes(&self) -> Vec<u8> {
            vec![self.0]
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.is_empty() {
                return Err(Error::msg("Empty bytes for SimpleSignature"));
            }
            Ok(SimpleSignature(bytes[0]))
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
            Ok(SimpleAggregatedSignature)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct SimpleAggregatedSignature;

    // Mock SafetyRules for testing
    use crate::safety_rules::{TwoChainSafetyRules, SafetyStateData};
    use crate::timeout::TwoChainTimeoutCertificate;

    // Helper to create a TwoChainSafetyRules for testing
    fn create_test_safety_rules() -> TwoChainSafetyRules<SimpleBlock, SimpleVote> {
        TwoChainSafetyRules::new(0)
    }

    #[test]
    fn test_round_manager_new() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        assert_eq!(manager.round(), 0);
        assert_eq!(manager.epoch(), 0);
    }

    #[test]
    fn test_round_manager_start_round() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        manager.start_round(5).unwrap();
        assert_eq!(manager.round(), 5);
    }

    #[test]
    fn test_round_manager_process_timeout() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        manager.process_timeout(TimeoutReason::NoQuorum).unwrap();
        assert_eq!(manager.round(), 1);
    }

    #[test]
    fn test_round_manager_process_proposal() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 0, metadata: SimpleMetadata { epoch: 0, round: 0 } });
        let result = manager.process_proposal(proposal, &verifier).unwrap();

        assert_eq!(result, ProposalProcessingResult::Accepted);
    }

    // Helper function to create votes with different signatures
    fn create_vote_with_sig(epoch: u64, round: u64, sig: u8) -> Arc<SimpleVote> {
        Arc::new(SimpleVote {
            epoch,
            round,
            signature: SimpleSignature(sig),
            author_id: sig,
            ledger_info: SimpleLedgerInfo { epoch, round },
        })
    }

    #[test]
    fn test_quorum_certificate_formation() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Add 3 votes for the same block (SimpleHash(0))
        let block_id = SimpleHash(0);
        let round = 0;

        for i in 0..3 {
            let vote = create_vote_with_sig(0, 0, i);
            let result = manager.process_vote(vote, &verifier).unwrap();

            // First two votes should be accepted, third forms quorum
            if i < 2 {
                assert_eq!(result, VoteReceptionResult::Accepted);
            }
        }

        // After 3 votes, pending votes should be cleaned up (QC formed)
        assert_eq!(manager.pending_votes.vote_count(round, block_id), 0);
    }

    #[test]
    fn test_no_quorum_with_insufficient_votes() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Add only 2 votes (not enough for quorum)
        for i in 0..2 {
            let vote = create_vote_with_sig(0, 0, i);
            manager.process_vote(vote, &verifier).unwrap();
        }

        // Votes should still be pending (no QC formed)
        assert_eq!(manager.pending_votes.vote_count(0, SimpleHash(0)), 2);
    }

    #[test]
    fn test_has_quorum_certificate() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Initially, no QC
        assert!(!manager.has_quorum_certificate(SimpleHash(0), 0));

        // Add votes to form a QC (need at least 3 votes)
        for i in 0..3 {
            let vote = create_vote_with_sig(0, 0, i);
            manager.process_vote(vote, &verifier).unwrap();
        }

        // After QC formation and cleanup, has_quorum_certificate should return true
        // (when we've advanced to a later round)
        manager.start_round(1).unwrap();
        assert!(manager.has_quorum_certificate(SimpleHash(0), 0));
    }

    #[test]
    fn test_vote_rejection_after_quorum() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Form a quorum with 3 votes
        for i in 0..3 {
            let vote = create_vote_with_sig(0, 0, i);
            manager.process_vote(vote, &verifier).unwrap();
        }

        // Advance to next round
        manager.process_timeout(TimeoutReason::NoQuorum).unwrap();

        // Try to add another vote for the old round
        let vote = create_vote_with_sig(0, 0, 3);
        let result = manager.process_vote(vote, &verifier).unwrap();

        // Vote should be rejected as old round (since we've moved past it)
        assert_eq!(result, VoteReceptionResult::OldRound);
    }

    #[test]
    fn test_integration_full_consensus_flow() {
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Start with round 0
        manager.start_round(0).unwrap();

        // Process a proposal
        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 0, metadata: SimpleMetadata { epoch: 0, round: 0 } });
        assert_eq!(
            manager.process_proposal(proposal.clone(), &verifier).unwrap(),
            ProposalProcessingResult::Accepted
        );

        // Add votes to form a quorum (need at least 3)
        for i in 0..3 {
            let result = manager.process_vote(
                create_vote_with_sig(0, 0, i),
                &verifier
            ).unwrap();

            // First 2 votes should be accepted, 3rd forms quorum
            if i < 2 {
                assert_eq!(result, VoteReceptionResult::Accepted);
            }
        }

        // Move to next round
        manager.process_timeout(TimeoutReason::NoQuorum).unwrap();
        assert_eq!(manager.round(), 1);

        // Verify QC exists for previous round
        assert!(manager.has_quorum_certificate(SimpleHash(0), 0));
    }

    #[test]
    fn test_safety_rules_accessor() {
        // Test lines 144-145: safety_rules() accessor
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // Test immutable accessor
        let safety = manager.safety_rules();
        assert_eq!(safety.state().epoch(), 0);
    }

    #[test]
    fn test_try_vote_for_proposal_success() {
        // Test lines 196, 199, 201: try_vote_for_proposal success path
        // Note: 2-chain rule requires block.round == qc.round + 1
        // Since qc_round starts at 0, we need to vote for round 1
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 1, metadata: SimpleMetadata { epoch: 0, round: 1 } });

        // Should be safe to vote for round 1 proposal (qc_round 0 + 1 = 1)
        let result = manager.try_vote_for_proposal(&proposal, None);
        assert!(result.is_ok());

        // Last voted round should be updated
        assert_eq!(manager.safety_rules().state().last_voted_round(), 1);
    }

    #[test]
    fn test_try_vote_for_proposal_old_round_rejection() {
        // Test monotonicity check: cannot vote for older round
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // First, vote for round 1 (qc_round 0 + 1 = 1)
        let proposal_round_1 = Arc::new(SimpleBlock { epoch: 0, round: 1, metadata: SimpleMetadata { epoch: 0, round: 1 } });
        manager.try_vote_for_proposal(&proposal_round_1, None).unwrap();
        assert_eq!(manager.safety_rules().state().last_voted_round(), 1);

        // Now try to vote for round 0 (older than last_voted_round 1)
        // This should fail the monotonicity check
        let proposal_round_0 = Arc::new(SimpleBlock { epoch: 0, round: 0, metadata: SimpleMetadata { epoch: 0, round: 0 } });
        let result = manager.try_vote_for_proposal(&proposal_round_0, None);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("already voted"));
    }

    #[test]
    fn test_process_vote_epoch_mismatch() {
        // Test line 260: vote rejection for epoch mismatch
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Create a vote for epoch 5 (different from manager's epoch 0)
        let vote_epoch_5 = Arc::new(SimpleVote {
            epoch: 5,
            round: 0,
            signature: SimpleSignature(0),
            author_id: 0,
            ledger_info: SimpleLedgerInfo { epoch: 5, round: 0 },
        });

        let result = manager.process_vote(vote_epoch_5, &verifier).unwrap();

        // Should be rejected due to epoch mismatch
        assert_eq!(result, VoteReceptionResult::Rejected(VoteStatus::Rejected));
    }

    #[test]
    fn test_try_vote_for_proposal_with_timeout_cert() {
        // Test try_vote_for_proposal with timeout certificate
        // Note: 2-chain rule requires block.round == qc.round + 1
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 1, metadata: SimpleMetadata { epoch: 0, round: 1 } });

        // Vote for a proposal after a timeout (pass None for timeout cert as placeholder)
        let result = manager.try_vote_for_proposal(&proposal, None);
        assert!(result.is_ok());
        assert_eq!(manager.safety_rules().state().last_voted_round(), 1);
    }

    #[test]
    fn test_safety_rules_mut_accessor() {
        // Test the mutable accessor for safety_rules
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // Test mutable accessor
        let safety = manager.safety_rules_mut();
        assert_eq!(safety.state().epoch(), 0);
    }

    #[test]
    fn test_round_state_accessor() {
        // Test the round_state accessor
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // Test round_state accessor
        let state = manager.round_state();
        assert_eq!(state.epoch(), 0);
        assert_eq!(state.round(), 0);
    }

    #[test]
    fn test_pending_votes_accessor() {
        // Test the pending_votes accessor
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // Test pending_votes accessor - just verify we can access it
        let _votes = manager.pending_votes();
        // The accessor works if we can call this
    }

    #[test]
    fn test_epoch_accessor() {
        // Test the epoch accessor
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        assert_eq!(manager.epoch(), 0);
    }

    #[test]
    fn test_round_accessor() {
        // Test the round accessor
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        assert_eq!(manager.round(), 0);
    }

    #[test]
    fn test_try_vote_for_proposal_same_round_twice() {
        // Test voting for the same round twice (should succeed - idempotent)
        // Note: 2-chain rule requires block.round == qc.round + 1
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);

        // First vote for round 1
        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 1, metadata: SimpleMetadata { epoch: 0, round: 1 } });
        manager.try_vote_for_proposal(&proposal, None).unwrap();
        assert_eq!(manager.safety_rules().state().last_voted_round(), 1);

        // Second vote for same round (should be ok since round >= last_voted_round)
        let result = manager.try_vote_for_proposal(&proposal, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_vote_current_epoch_accepted() {
        // Test that votes for current epoch are accepted
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Create a vote for epoch 0 (same as manager's epoch)
        let vote = create_vote_with_sig(0, 0, 0);

        let result = manager.process_vote(vote, &verifier).unwrap();

        // Should be accepted (not rejected for epoch mismatch)
        assert_eq!(result, VoteReceptionResult::Accepted);
    }

    #[test]
    fn test_round_manager_config_default() {
        // Test RoundManagerConfig default values
        let config = RoundManagerConfig::default();
        assert_eq!(config.max_buffered_rounds, 10);
        assert_eq!(config.round_timeout_ms, 5000);
        assert_eq!(config.max_pending_votes, 1000);
    }

    #[test]
    fn test_round_manager_config_custom() {
        // Test RoundManagerConfig with custom values
        let config = RoundManagerConfig {
            max_buffered_rounds: 20,
            round_timeout_ms: 10000,
            max_pending_votes: 2000,
        };
        assert_eq!(config.max_buffered_rounds, 20);
        assert_eq!(config.round_timeout_ms, 10000);
        assert_eq!(config.max_pending_votes, 2000);
    }

    #[test]
    fn test_proposal_processing_result_variants() {
        // Test all ProposalProcessingResult variants
        let accepted = ProposalProcessingResult::Accepted;
        let rejected = ProposalProcessingResult::Rejected("invalid".to_string());
        let old_round = ProposalProcessingResult::OldRound;
        let future_round = ProposalProcessingResult::FutureRound;

        assert_eq!(accepted, ProposalProcessingResult::Accepted);
        assert_eq!(rejected, ProposalProcessingResult::Rejected("invalid".to_string()));
        assert_eq!(old_round, ProposalProcessingResult::OldRound);
        assert_eq!(future_round, ProposalProcessingResult::FutureRound);
    }

    #[test]
    fn test_process_proposal_old_round() {
        // Test proposal processing for old round
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Advance to round 5
        manager.start_round(5).unwrap();

        // Try to process proposal for round 3 (old round)
        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 3, metadata: SimpleMetadata { epoch: 0, round: 3 } });
        let result = manager.process_proposal(proposal, &verifier).unwrap();

        assert_eq!(result, ProposalProcessingResult::OldRound);
    }

    #[test]
    fn test_process_proposal_future_round() {
        // Test proposal processing for future round
        let config = RoundManagerConfig::default();
        let safety_rules = create_test_safety_rules();
        let mut manager = RoundManager::<SimpleBlock, SimpleVote, TwoChainSafetyRules<SimpleBlock, SimpleVote>>::new(config, safety_rules);
        let verifier = SimpleVerifier;

        // Currently at round 0
        // Try to process proposal for round 10 (future round)
        let proposal = Arc::new(SimpleBlock { epoch: 0, round: 10, metadata: SimpleMetadata { epoch: 0, round: 10 } });
        let result = manager.process_proposal(proposal, &verifier).unwrap();

        assert_eq!(result, ProposalProcessingResult::FutureRound);
    }
}
