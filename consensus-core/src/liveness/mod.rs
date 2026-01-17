// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Liveness components for consensus.
//!
//! This module contains components that ensure the consensus system makes progress
//! even with network issues or failing validators.
//!
//! ## Structure
//!
//! - **pacemaker**: Round timeout and progression management
//! - **proposer**: Proposer election logic

pub mod pacemaker;
pub mod proposer;

// Re-export commonly used types
pub use pacemaker::{
    Pacemaker, PacemakerConfig, NewRoundReason, NewRoundEvent,
    RoundIntervalStrategy, ExponentialIntervalStrategy, PacemakerBuilder,
};
pub use proposer::{
    ProposerElectionImpl, RoundRobinProposer, WeightedProposer,
    ReputationTrackerImpl, ProposerElectionConfig,
};
