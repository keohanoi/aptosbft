// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Proposal generation for consensus.
//!
//! This module provides utilities for generating block proposals,
//! including parent block selection and transaction collection.

mod generator;

pub use generator::{ProposalConfig, ProposalGenerator};
