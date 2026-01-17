// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Epoch management for consensus.
//!
//! This module provides epoch management functionality, handling epoch transitions
//! and validator set management.

use crate::types::Epoch;

/// Configuration for epoch management
#[derive(Clone, Debug)]
pub struct EpochManagerConfig {
    /// The initial epoch
    pub initial_epoch: Epoch,

    /// Whether to automatically transition epochs
    pub auto_transition: bool,
}

impl Default for EpochManagerConfig {
    fn default() -> Self {
        Self {
            initial_epoch: 0,
            auto_transition: false,
        }
    }
}

/// State information for an epoch
#[derive(Clone, Debug)]
pub struct EpochState {
    /// The current epoch number
    epoch: Epoch,

    /// The timestamp when this epoch started
    start_time: u64,
}

impl EpochState {
    /// Create a new epoch state
    pub fn new(epoch: Epoch, start_time: u64) -> Self {
        Self { epoch, start_time }
    }

    /// Get the current epoch
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Get the start time of this epoch
    pub fn start_time(&self) -> u64 {
        self.start_time
    }
}

/// Epoch manager for handling epoch transitions
///
/// This struct manages epochs in the consensus protocol, tracking the current
/// epoch and handling epoch transitions.
pub struct EpochManager {
    /// The current epoch state
    epoch_state: EpochState,

    /// Configuration
    config: EpochManagerConfig,
}

impl EpochManager {
    /// Create a new EpochManager
    pub fn new(config: EpochManagerConfig) -> Self {
        let epoch_state = EpochState::new(config.initial_epoch, 0);
        Self {
            epoch_state,
            config,
        }
    }

    /// Get the current epoch
    pub fn epoch(&self) -> Epoch {
        self.epoch_state.epoch()
    }

    /// Get the current epoch state
    pub fn epoch_state(&self) -> &EpochState {
        &self.epoch_state
    }

    /// Start a new epoch
    ///
    /// This method transitions to the specified epoch, updating the epoch state.
    pub fn start_new_epoch(&mut self, epoch: Epoch) -> Result<(), EpochTransitionError> {
        if epoch <= self.epoch_state.epoch() {
            return Err(EpochTransitionError::OldEpoch {
                current: self.epoch_state.epoch(),
                proposed: epoch,
            });
        }

        if epoch != self.epoch_state.epoch() + 1 {
            return Err(EpochTransitionError::SkippedEpoch {
                current: self.epoch_state.epoch(),
                proposed: epoch,
            });
        }

        // Update the epoch state
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.epoch_state = EpochState::new(epoch, start_time);

        Ok(())
    }

    /// Check if an epoch transition is needed
    ///
    /// In a full implementation, this would check if we've received enough
    /// epoch change votes or certificates to transition to the next epoch.
    pub fn check_epoch_transition(&self) -> bool {
        if !self.config.auto_transition {
            return false;
        }

        // In a full implementation, this would check:
        // 1. If we have a quorum of epoch change votes
        // 2. If the current epoch has ended
        // 3. If we've received a valid epoch certificate

        false
    }

    /// Force transition to the next epoch
    ///
    /// This method can be used to force an epoch transition, typically
    /// for testing or recovery purposes.
    pub fn force_next_epoch(&mut self) -> Result<(), EpochTransitionError> {
        let next_epoch = self.epoch_state.epoch() + 1;
        self.start_new_epoch(next_epoch)
    }
}

/// Errors that can occur during epoch transition
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EpochTransitionError {
    /// Attempted to transition to an old epoch
    OldEpoch {
        /// The current epoch
        current: Epoch,
        /// The proposed (older) epoch
        proposed: Epoch,
    },

    /// Attempted to skip epochs
    SkippedEpoch {
        /// The current epoch
        current: Epoch,
        /// The proposed epoch
        proposed: Epoch,
    },

    /// Invalid epoch transition
    InvalidTransition(String),
}

impl std::fmt::Display for EpochTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OldEpoch { current, proposed } => write!(
                f,
                "Cannot transition to epoch {}: already at epoch {}",
                proposed, current
            ),
            Self::SkippedEpoch { current, proposed } => write!(
                f,
                "Cannot skip epochs: current={}, proposed={}",
                current, proposed
            ),
            Self::InvalidTransition(msg) => write!(f, "Invalid epoch transition: {}", msg),
        }
    }
}

impl std::error::Error for EpochTransitionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_manager_new() {
        let config = EpochManagerConfig::default();
        let manager = EpochManager::new(config);

        assert_eq!(manager.epoch(), 0);
    }

    #[test]
    fn test_epoch_state_new() {
        let state = EpochState::new(5, 12345);

        assert_eq!(state.epoch(), 5);
        assert_eq!(state.start_time(), 12345);
    }

    #[test]
    fn test_start_new_epoch() {
        let config = EpochManagerConfig::default();
        let mut manager = EpochManager::new(config);

        manager.start_new_epoch(1).unwrap();
        assert_eq!(manager.epoch(), 1);

        manager.start_new_epoch(2).unwrap();
        assert_eq!(manager.epoch(), 2);
    }

    #[test]
    fn test_start_new_epoch_rejects_old() {
        let config = EpochManagerConfig::default();
        let mut manager = EpochManager::new(config);

        manager.start_new_epoch(1).unwrap();

        let result = manager.start_new_epoch(0);
        assert!(matches!(
            result,
            Err(EpochTransitionError::OldEpoch { current: 1, proposed: 0 })
        ));
    }

    #[test]
    fn test_start_new_epoch_rejects_skip() {
        let config = EpochManagerConfig::default();
        let mut manager = EpochManager::new(config);

        let result = manager.start_new_epoch(2);
        assert!(matches!(
            result,
            Err(EpochTransitionError::SkippedEpoch { current: 0, proposed: 2 })
        ));
    }

    #[test]
    fn test_force_next_epoch() {
        let config = EpochManagerConfig::default();
        let mut manager = EpochManager::new(config);

        manager.force_next_epoch().unwrap();
        assert_eq!(manager.epoch(), 1);

        manager.force_next_epoch().unwrap();
        assert_eq!(manager.epoch(), 2);
    }

    #[test]
    fn test_epoch_transition_error_display() {
        let err = EpochTransitionError::OldEpoch { current: 5, proposed: 3 };
        assert_eq!(
            format!("{}", err),
            "Cannot transition to epoch 3: already at epoch 5"
        );

        let err = EpochTransitionError::SkippedEpoch { current: 1, proposed: 3 };
        assert_eq!(
            format!("{}", err),
            "Cannot skip epochs: current=1, proposed=3"
        );
    }
}
