// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Safety rules for Byzantine fault-tolerant consensus.
//!
//! This module provides implementations of the safety rules that enforce
//! the 2-chain commit protocol, ensuring consensus safety properties.

mod recovery;
mod state;
mod two_chain;

pub use recovery::{load_safety_rules, persist_safety_state, RecoveryError};
pub use state::SafetyStateData;
pub use two_chain::TwoChainSafetyRules;
