// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Cryptographic utilities for consensus.
//!
//! This module provides signature aggregation and voting power calculation
//! utilities used in forming quorum certificates.

mod signature_aggregator;

pub use signature_aggregator::SignatureAggregator;
