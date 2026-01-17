// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Reconfiguration handler for consensus.
//!
//! This module handles epoch transitions and validator set changes,
//! ensuring safe reconfiguration of the consensus protocol.

use consensus_traits::{
    block::{Block, BlockMetadata},
    core::Hash as HashTrait,
    proposer::ProposerElection,
};
use std::{
    collections::HashMap,
    fmt::Debug,
};
use anyhow::{ensure, Result};

use crate::{
    epoch_manager::EpochManager,
    types::{Epoch, Round},
};

/// Configuration for reconfiguration handling.
#[derive(Clone, Debug)]
pub struct ReconfigConfig {
    /// Number of rounds before epoch end to prepare for reconfiguration
    pub preparation_window: u64,

    /// Whether to verify validator signatures during reconfiguration
    pub verify_signatures: bool,

    /// Maximum time to wait for reconfiguration to complete (in milliseconds)
    pub reconfig_timeout_ms: u64,
}

impl Default for ReconfigConfig {
    fn default() -> Self {
        Self {
            preparation_window: 5,
            verify_signatures: true,
            reconfig_timeout_ms: 30000,
        }
    }
}

/// Status of a reconfiguration operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconfigStatus {
    /// No reconfiguration in progress
    Idle,

    /// Preparing for reconfiguration
    Preparing {
        /// Current epoch
        current_epoch: Epoch,

        /// Next epoch
        next_epoch: Epoch,
    },

    /// Reconfiguration in progress
    InProgress {
        /// New epoch being transitioned to
        new_epoch: Epoch,

        /// Number of validators processed
        validators_processed: usize,

        /// Total validators to process
        total_validators: usize,
    },

    /// Reconfiguration completed successfully
    Completed {
        /// The new epoch
        epoch: Epoch,

        /// Number of validators in the new set
        validator_count: usize,
    },

    /// Reconfiguration failed
    Failed {
        /// Error message
        error: String,
    },
}

impl ReconfigStatus {
    /// Check if reconfiguration is in progress.
    pub fn is_in_progress(&self) -> bool {
        matches!(self, ReconfigStatus::Preparing { .. } | ReconfigStatus::InProgress { .. })
    }

    /// Check if reconfiguration has completed.
    pub fn is_completed(&self) -> bool {
        matches!(self, ReconfigStatus::Completed { .. })
    }

    /// Check if reconfiguration has failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, ReconfigStatus::Failed { .. })
    }

    /// Get the current epoch if available.
    pub fn current_epoch(&self) -> Option<Epoch> {
        match self {
            ReconfigStatus::Preparing { current_epoch, .. } => Some(*current_epoch),
            ReconfigStatus::InProgress { new_epoch, .. } => Some(*new_epoch - 1),
            ReconfigStatus::Completed { epoch, .. } => Some(*epoch),
            _ => None,
        }
    }

    /// Get the new epoch if available.
    pub fn new_epoch(&self) -> Option<Epoch> {
        match self {
            ReconfigStatus::Preparing { next_epoch, .. } => Some(*next_epoch),
            ReconfigStatus::InProgress { new_epoch, .. } => Some(*new_epoch),
            ReconfigStatus::Completed { epoch, .. } => Some(*epoch),
            _ => None,
        }
    }
}

/// Validator set information.
#[derive(Clone, Debug)]
pub struct ValidatorSet<A> {
    /// The epoch this validator set is for
    pub epoch: Epoch,

    /// Map from validator ID to voting power
    pub validators: HashMap<A, u64>,

    /// Total voting power
    pub total_voting_power: u128,
}

impl<A> ValidatorSet<A>
where
    A: Clone + Eq + HashTrait + std::hash::Hash,
{
    /// Create a new validator set.
    ///
    /// ## Parameters
    ///
    /// - `epoch`: The epoch for this validator set
    /// - `validators`: Map of validator IDs to their voting power
    pub fn new(epoch: Epoch, validators: HashMap<A, u64>) -> Self {
        let total_voting_power = validators.values().map(|&v| v as u128).sum();

        Self {
            epoch,
            validators,
            total_voting_power,
        }
    }

    /// Get the number of validators.
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if the validator set is empty.
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Get the voting power for a specific validator.
    pub fn get_voting_power(&self, validator: &A) -> Option<u64> {
        self.validators.get(validator).copied()
    }

    /// Check if a validator is in the set.
    pub fn contains(&self, validator: &A) -> bool {
        self.validators.contains_key(validator)
    }

    /// Get all validator IDs.
    pub fn validator_ids(&self) -> impl Iterator<Item = &A> {
        self.validators.keys()
    }

    /// Calculate the quorum threshold (2f + 1 voting power).
    pub fn quorum_threshold(&self) -> u128 {
        // In BFT, we need > 2/3 of voting power for quorum
        (self.total_voting_power * 2 / 3) + 1
    }

    /// Check if a set of votes reaches quorum.
    pub fn has_quorum<'a>(&self, voters: impl Iterator<Item = &'a A>) -> bool {
        let voting_power: u128 = voters
            .filter_map(|v| self.get_voting_power(v))
            .map(|p| p as u128)
            .sum();

        voting_power >= self.quorum_threshold()
    }
}

/// Reconfiguration event.
#[derive(Clone, Debug)]
pub enum ReconfigEvent<A> {
    /// New epoch starting
    NewEpoch {
        /// The new epoch number
        epoch: Epoch,

        /// Starting round of the new epoch
        start_round: Round,
    },

    /// Validator set changed
    ValidatorSetChanged {
        /// The new validator set
        validator_set: ValidatorSet<A>,
    },

    /// Proposal generation policy changed
    PolicyChanged {
        /// Description of the policy change
        description: String,
    },
}

/// Reconfiguration handler for epoch transitions and validator set changes.
///
/// This handler coordinates the safe transition between epochs,
/// ensuring all validators agree on the new configuration.
///
/// # Type Parameters
///
/// - `A`: Author/validator type
/// - `PE`: ProposerElection type
pub struct ReconfigHandler<A, PE>
where
    A: Clone + Eq + HashTrait + std::hash::Hash + Debug + Send + Sync + 'static,
    PE: ProposerElection + Clone + 'static,
{
    /// Configuration
    config: ReconfigConfig,

    /// Current reconfiguration status
    status: ReconfigStatus,

    /// Epoch manager for tracking epoch state
    epoch_manager: EpochManager,

    /// Current validator set for each epoch
    validator_sets: HashMap<Epoch, ValidatorSet<A>>,

    /// Pending reconfiguration events to process
    pending_events: Vec<ReconfigEvent<A>>,

    /// The proposer election strategy
    proposer_election: Option<PE>,
}

impl<A, PE> ReconfigHandler<A, PE>
where
    A: Clone + Eq + HashTrait + std::hash::Hash + Debug + Send + Sync + 'static,
    PE: ProposerElection + Clone + 'static,
{
    /// Create a new reconfiguration handler.
    ///
    /// ## Parameters
    ///
    /// - `config`: Handler configuration
    /// - `epoch_manager`: Epoch manager instance
    pub fn new(config: ReconfigConfig, epoch_manager: EpochManager) -> Self {
        Self {
            config,
            status: ReconfigStatus::Idle,
            epoch_manager,
            validator_sets: HashMap::new(),
            pending_events: Vec::new(),
            proposer_election: None,
        }
    }

    /// Get the current reconfiguration status.
    pub fn status(&self) -> &ReconfigStatus {
        &self.status
    }

    /// Get the current epoch.
    pub fn current_epoch(&self) -> Epoch {
        self.epoch_manager.epoch()
    }

    /// Get the validator set for an epoch.
    pub fn validator_set(&self, epoch: Epoch) -> Option<&ValidatorSet<A>> {
        self.validator_sets.get(&epoch)
    }

    /// Get the current validator set.
    pub fn current_validator_set(&self) -> Option<&ValidatorSet<A>> {
        self.validator_set(self.current_epoch())
    }

    /// Set the proposer election strategy.
    pub fn set_proposer_election(&mut self, proposer_election: PE) {
        self.proposer_election = Some(proposer_election);
    }

    /// Start preparing for an epoch transition.
    ///
    /// ## Parameters
    ///
    /// - `new_epoch`: The epoch to transition to
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - A reconfiguration is already in progress
    /// - The new epoch is not the next epoch
    pub fn prepare_reconfig(&mut self, new_epoch: Epoch) -> Result<()> {
        ensure!(
            !self.status.is_in_progress(),
            "Reconfiguration already in progress"
        );

        let current_epoch = self.current_epoch();
        ensure!(
            new_epoch == current_epoch + 1,
            "Can only transition to the next epoch"
        );

        self.status = ReconfigStatus::Preparing {
            current_epoch,
            next_epoch: new_epoch,
        };

        Ok(())
    }

    /// Set the validator set for an epoch.
    ///
    /// ## Parameters
    ///
    /// - `epoch`: The epoch for this validator set
    /// - `validator_set`: The validator set
    pub fn set_validator_set(&mut self, epoch: Epoch, validator_set: ValidatorSet<A>) {
        self.validator_sets.insert(epoch, validator_set);
    }

    /// Begin the reconfiguration process.
    ///
    /// ## Parameters
    ///
    /// - `validator_set`: The new validator set
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - Not in preparing state
    /// - Validator set is empty
    pub fn begin_reconfig(&mut self, validator_set: ValidatorSet<A>) -> Result<()> {
        let next_epoch = match &self.status {
            ReconfigStatus::Preparing { next_epoch, .. } => *next_epoch,
            _ => anyhow::bail!("Not in preparing state"),
        };

        ensure!(!validator_set.is_empty(), "Validator set cannot be empty");

        let total_validators = validator_set.len();

        self.status = ReconfigStatus::InProgress {
            new_epoch: next_epoch,
            validators_processed: 0,
            total_validators,
        };

        // Store the validator set for the new epoch
        self.validator_sets.insert(next_epoch, validator_set);

        Ok(())
    }

    /// Complete the reconfiguration.
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - Not in progress state
    /// - Not all validators have been processed
    pub fn complete_reconfig(&mut self) -> Result<()> {
        let (new_epoch, validator_count) = match &self.status {
            ReconfigStatus::InProgress {
                new_epoch,
                validators_processed,
                total_validators,
            } => {
                ensure!(
                    validators_processed == total_validators,
                    "Not all validators processed: {}/{}",
                    validators_processed,
                    total_validators
                );
                (*new_epoch, *total_validators)
            }
            _ => anyhow::bail!("Not in progress state"),
        };

        // Update the epoch manager
        self.epoch_manager.start_new_epoch(new_epoch)?;

        self.status = ReconfigStatus::Completed {
            epoch: new_epoch,
            validator_count,
        };

        // Reset to idle after completion
        self.status = ReconfigStatus::Idle;

        Ok(())
    }

    /// Add a pending reconfiguration event.
    pub fn add_event(&mut self, event: ReconfigEvent<A>) {
        self.pending_events.push(event);
    }

    /// Get all pending events.
    pub fn pending_events(&self) -> &[ReconfigEvent<A>] {
        &self.pending_events
    }

    /// Take all pending events.
    pub fn take_pending_events(&mut self) -> Vec<ReconfigEvent<A>> {
        std::mem::take(&mut self.pending_events)
    }

    /// Cancel the current reconfiguration.
    pub fn cancel(&mut self) {
        self.status = ReconfigStatus::Idle;
        self.pending_events.clear();
    }

    /// Check if we need to reconfigure based on round and epoch.
    ///
    /// ## Parameters
    ///
    /// - `round`: Current round
    /// - `epoch_end_round`: The round at which the current epoch ends
    pub fn should_reconfigure(&self, round: Round, epoch_end_round: Round) -> bool {
        round >= epoch_end_round.saturating_sub(self.config.preparation_window)
    }

    /// Process a block for reconfiguration signals.
    ///
    /// ## Parameters
    ///
    /// - `block`: The block to process
    ///
    /// ## Returns
    ///
    /// An optional reconfiguration event if one is detected.
    pub fn process_block<B>(&mut self, block: &B) -> Option<ReconfigEvent<A>>
    where
        B: Block,
    {
        let metadata = block.metadata();
        let epoch = metadata.epoch();
        let current_epoch = self.current_epoch();

        // Check for epoch transition
        if epoch > current_epoch {
            // This block is from a future epoch
            return Some(ReconfigEvent::NewEpoch {
                epoch,
                start_round: metadata.round(),
            });
        }

        // Check for validator set changes embedded in the block
        // This would require parsing block data for reconfiguration info
        // For now, this is a placeholder
        None
    }

    /// Get the quorum certificate for the current epoch.
    ///
    /// This would typically be stored in the epoch manager.
    pub fn epoch_quorum_cert(&self) -> Option<Epoch> {
        // Placeholder - in production, this would return the QC that
        // proves the current epoch's configuration
        Some(self.current_epoch())
    }

    /// Update reconfiguration progress.
    ///
    /// ## Parameters
    ///
    /// - `validators_processed`: Number of validators processed
    pub fn update_progress(&mut self, validators_processed: usize) {
        if let ReconfigStatus::InProgress {
            new_epoch,
            total_validators,
            ..
        } = self.status
        {
            self.status = ReconfigStatus::InProgress {
                new_epoch,
                validators_processed,
                total_validators,
            };
        }
    }

    /// Get the number of pending events.
    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epoch_manager::EpochManagerConfig;
    use std::hash::Hash as StdHash;

    // Test author type
    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    struct TestAuthor(u8);

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

    type TestReconfigHandler = ReconfigHandler<TestAuthor, TestProposerElection>;

    // Simple test proposer election
    #[derive(Clone)]
    struct TestProposerElection;

    impl consensus_traits::proposer::ProposerElection for TestProposerElection {
        type Round = u64;
        type Author = TestAuthor;

        fn get_valid_proposer(&self, _round: &Self::Round) -> Option<Self::Author> {
            Some(TestAuthor(1))
        }
    }

    // SAFETY: TestProposerElection is a unit struct with no internal state
    unsafe impl Send for TestProposerElection {}
    unsafe impl Sync for TestProposerElection {}

    fn create_test_validator_set(epoch: u64, count: u8) -> ValidatorSet<TestAuthor> {
        let mut validators = HashMap::new();
        for i in 0..count {
            validators.insert(TestAuthor(i), 100); // Equal voting power
        }
        ValidatorSet::new(epoch, validators)
    }

    #[test]
    fn test_reconfig_config_default() {
        let config = ReconfigConfig::default();
        assert_eq!(config.preparation_window, 5);
        assert!(config.verify_signatures);
        assert_eq!(config.reconfig_timeout_ms, 30000);
    }

    #[test]
    fn test_reconfig_status() {
        let status = ReconfigStatus::Idle;
        assert!(!status.is_in_progress());
        assert!(!status.is_completed());
        assert!(!status.is_failed());

        let status = ReconfigStatus::Preparing {
            current_epoch: 1,
            next_epoch: 2,
        };
        assert!(status.is_in_progress());
        assert_eq!(status.current_epoch(), Some(1));
        assert_eq!(status.new_epoch(), Some(2));

        let status = ReconfigStatus::Completed {
            epoch: 2,
            validator_count: 100,
        };
        assert!(status.is_completed());
        assert_eq!(status.current_epoch(), Some(2));
        assert_eq!(status.new_epoch(), Some(2));
    }

    #[test]
    fn test_validator_set() {
        let mut validators = HashMap::new();
        validators.insert(TestAuthor(1), 100);
        validators.insert(TestAuthor(2), 200);
        validators.insert(TestAuthor(3), 150);

        let validator_set = ValidatorSet::new(1, validators);

        assert_eq!(validator_set.len(), 3);
        assert!(!validator_set.is_empty());
        assert_eq!(validator_set.get_voting_power(&TestAuthor(1)), Some(100));
        assert_eq!(validator_set.total_voting_power, 450);
        assert!(validator_set.contains(&TestAuthor(2)));
        assert!(!validator_set.contains(&TestAuthor(99)));

        // Test quorum
        let voters = vec![&TestAuthor(1), &TestAuthor(2)];
        // 200 < 301, so no quorum
        assert!(!validator_set.has_quorum(voters.into_iter()));

        // All 3 validators should reach quorum
        let all_voters = vec![&TestAuthor(1), &TestAuthor(2), &TestAuthor(3)];
        assert!(validator_set.has_quorum(all_voters.into_iter())); // 300 >= 301
    }

    #[test]
    fn test_reconfig_handler_new() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let handler = TestReconfigHandler::new(config, epoch_manager);

        assert!(matches!(handler.status(), ReconfigStatus::Idle));
        assert_eq!(handler.current_epoch(), 1);
        assert!(handler.current_validator_set().is_none());
        assert_eq!(handler.pending_event_count(), 0);
    }

    #[test]
    fn test_prepare_reconfig() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        // Prepare for next epoch
        assert!(handler.prepare_reconfig(2).is_ok());
        assert!(handler.status().is_in_progress());

        // Can't prepare again while in progress
        assert!(handler.prepare_reconfig(3).is_err());

        // Can't skip an epoch
        handler.cancel();
        assert!(handler.prepare_reconfig(3).is_err());
    }

    #[test]
    fn test_set_validator_set() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        let validator_set = create_test_validator_set(2, 4);
        handler.set_validator_set(2, validator_set.clone());

        assert_eq!(handler.validator_set(2).unwrap().len(), 4);
    }

    #[test]
    fn test_begin_reconfig() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        // Must prepare first
        let validator_set = create_test_validator_set(2, 4);
        assert!(handler.begin_reconfig(validator_set).is_err());

        // Prepare and then begin
        handler.prepare_reconfig(2).unwrap();
        let validator_set = create_test_validator_set(2, 4);
        assert!(handler.begin_reconfig(validator_set).is_ok());

        // Empty validator set should fail
        handler.cancel();
        handler.prepare_reconfig(2).unwrap();
        let empty_set = ValidatorSet::new(2, HashMap::new());
        assert!(handler.begin_reconfig(empty_set).is_err());
    }

    #[test]
    fn test_complete_reconfig() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        handler.prepare_reconfig(2).unwrap();
        let validator_set = create_test_validator_set(2, 4);
        handler.begin_reconfig(validator_set).unwrap();

        // Update progress to all validators processed
        handler.update_progress(4);

        // Complete successfully
        assert!(handler.complete_reconfig().is_ok());
        assert_eq!(handler.current_epoch(), 2);
        assert!(matches!(handler.status(), ReconfigStatus::Idle));
    }

    #[test]
    fn test_should_reconfigure() {
        let config = ReconfigConfig {
            preparation_window: 5,
            ..Default::default()
        };
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let handler = TestReconfigHandler::new(config, epoch_manager);

        // Epoch ends at round 100
        let epoch_end_round = 100;

        // Should not reconfigure too early
        assert!(!handler.should_reconfigure(90, epoch_end_round));

        // Should reconfigure within preparation window
        assert!(handler.should_reconfigure(96, epoch_end_round));
        assert!(handler.should_reconfigure(100, epoch_end_round));
    }

    #[test]
    fn test_pending_events() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        assert_eq!(handler.pending_event_count(), 0);

        handler.add_event(ReconfigEvent::NewEpoch {
            epoch: 2,
            start_round: 100,
        });

        assert_eq!(handler.pending_event_count(), 1);

        let events = handler.take_pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(handler.pending_event_count(), 0);
    }

    #[test]
    fn test_cancel() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        handler.prepare_reconfig(2).unwrap();
        assert!(handler.status().is_in_progress());

        handler.cancel();
        assert!(matches!(handler.status(), ReconfigStatus::Idle));
        assert_eq!(handler.pending_event_count(), 0);
    }

    #[test]
    fn test_update_progress() {
        let config = ReconfigConfig::default();
        let epoch_manager = EpochManager::new(EpochManagerConfig {
            initial_epoch: 1,
            auto_transition: false,
        });
        let mut handler = TestReconfigHandler::new(config, epoch_manager);

        handler.prepare_reconfig(2).unwrap();
        let validator_set = create_test_validator_set(2, 10);
        handler.begin_reconfig(validator_set).unwrap();

        handler.update_progress(5);

        if let ReconfigStatus::InProgress {
            validators_processed,
            ..
        } = handler.status()
        {
            assert_eq!(*validators_processed, 5);
        } else {
            panic!("Expected InProgress status");
        }
    }
}
