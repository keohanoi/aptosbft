// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! # AptosBFT Consensus Core Library
//!
//! This library provides the core AptosBFT consensus algorithm, generic over any blockchain implementation.
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//!
//! - [`crypto`] - Signature aggregation and voting power utilities
//! - [`state`] - Round state management and epoch transitions
//! - [`votes`] - Vote aggregation and pending vote tracking
//! - [`types`] - Common consensus types (Round, Epoch, etc.)
//!
//! ## Usage
//!
//! The library is designed to be used through trait implementations. Users must:
//!
//! 1. Implement the traits from `consensus-traits` for their blockchain types
//! 2. Use the provided state management and vote tracking types
//!
//! ```rust,no_run,ignore
//! use consensus_core::{state::RoundState, votes::PendingVotes};
//! use consensus_traits::{Block, Vote};
//!
//! // Create a RoundState with your specific types
//! let state = RoundState::<MyBlock>::new(config);
//!
//! // Create a PendingVotes tracker
//! let mut pending_votes = PendingVotes::<MyBlock, MyVote>::new(10);
//! ```

pub mod consensus;
pub mod crypto;
pub mod dag;
pub mod epoch_manager;
pub mod liveness;
pub mod network;
pub mod pipeline;
pub mod proposal;
pub mod round_manager;
pub mod safety_rules;
pub mod state;
pub mod testing;
pub mod timeout;
pub mod types;
pub mod votes;

// Re-export commonly used types
pub use types::{Epoch, Round};
pub use safety_rules::{SafetyStateData, TwoChainSafetyRules};
pub use timeout::{TwoChainTimeout, TwoChainTimeoutCertificate};

/// Error type for consensus operations
pub use consensus_traits::core::Error;

/// Version of the AptosBFT protocol implemented by this library
pub const APTOSBFT_VERSION: &str = "0.1.0";
