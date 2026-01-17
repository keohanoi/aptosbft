// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Timeout types for consensus round advancement.
//!
//! This module provides timeout certificate implementations for the 2-chain
//! commit protocol. Timeout certificates allow validators to advance rounds
//! even when the proposer is faulty or offline.

pub mod two_chain;

pub use two_chain::{TwoChainTimeout, TwoChainTimeoutCertificate};
