// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Round state management for the consensus protocol
//!
//! This module provides generic round state management that tracks the current state
//! of the consensus protocol for a specific round and epoch.

mod round_state;

pub use round_state::{RoundState, RoundStateConfig};
