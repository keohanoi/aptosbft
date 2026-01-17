// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Vote aggregation and pending vote tracking
//!
//! This module provides generic vote tracking that collects votes for blocks
//! and detects when a quorum has been reached.

mod pending_votes;

pub use pending_votes::{PendingVotes, VoteReceptionResult, VoteStatus};
