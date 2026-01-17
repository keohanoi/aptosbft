// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! 2-chain timeout implementation.
//!
//! This module provides timeout certificates for the 2-chain commit protocol.
//! When a round times out, validators can generate a timeout certificate (TC)
//! that proves 2f+1 validators timed out, allowing safe round advancement.

use consensus_traits::{block::Block, core::Error, TimeoutCertificate};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A timeout message for the 2-chain protocol.
///
/// When a validator times out in a round, it creates a TwoChainTimeout message
/// that references the highest quorum certificate (HQC) it knows about. This
/// ensures that timeouts don't violate the 2-chain safety rule.
///
/// # Fields
///
/// - `epoch`: The epoch this timeout is for
/// - `round`: The round that timed out
/// - `hqc_round`: The round of the highest quorum certificate known
///
/// # Safety
///
/// The timeout is only safe if hqc_round >= one_chain_round. This ensures
/// we don't timeout past the 1-chain we've committed to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoChainTimeout {
    /// The epoch this timeout is for
    pub epoch: u64,

    /// The round that timed out
    pub round: u64,

    /// The round of the highest quorum certificate known
    ///
    /// This is used in the safety rule: we can only timeout if this
    /// is >= our one_chain_round
    pub hqc_round: u64,
}

impl Default for TwoChainTimeout {
    fn default() -> Self {
        Self {
            epoch: 0,
            round: 0,
            hqc_round: 0,
        }
    }
}

impl TwoChainTimeout {
    /// Create a new 2-chain timeout
    pub fn new(epoch: u64, round: u64, hqc_round: u64) -> Self {
        Self {
            epoch,
            round,
            hqc_round,
        }
    }

    /// Get the epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Get the round
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Get the highest quorum certificate round
    pub fn hqc_round(&self) -> u64 {
        self.hqc_round
    }

    /// Verify the timeout is internally consistent
    ///
    /// A timeout is valid if hqc_round < round (can't certify a future round)
    pub fn verify(&self) -> Result<(), Error> {
        if self.hqc_round >= self.round {
            return Err(Error::msg(format!(
                "Invalid timeout: hqc_round ({}) >= round ({})",
                self.hqc_round, self.round
            )));
        }
        Ok(())
    }
}

/// A timeout certificate for the 2-chain protocol.
///
/// A timeout certificate proves that 2f+1 validators have timed out in a round,
/// allowing all validators to safely advance to the next round without violating
/// the 2-chain safety rule.
///
/// # Generic over Block
///
/// Like other consensus types, this is generic over the Block type.
///
/// # Fields
///
/// - `epoch`: The epoch this TC is for
/// - `round`: The round that timed out
/// - `hqc_round`: The round of the highest QC referenced in the timeout
/// - `highest_tc_round`: The highest TC round referenced (if TCs chain)
///
/// # Safety
///
/// The TC allows advancing to round+1 if:
/// - round == hqc_round + 1 OR round == highest_tc_round + 1
/// - hqc_round >= one_chain_round
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoChainTimeoutCertificate<B>
where
    B: Block,
{
    /// The epoch this timeout certificate is for
    pub epoch: u64,

    /// The round that timed out
    pub round: u64,

    /// The round of the highest quorum certificate referenced
    pub hqc_round: u64,

    /// The highest timeout certificate round referenced (for chained TCs)
    pub highest_tc_round: u64,

    /// The timeout object that was certified
    #[serde(skip)]
    pub timeout: Arc<TwoChainTimeout>,

    /// The aggregated signature from all validators who timed out
    pub aggregated_signature: B::Signature,
}

impl<B> TwoChainTimeoutCertificate<B>
where
    B: Block,
{
    /// Create a new 2-chain timeout certificate
    pub fn new(
        epoch: u64,
        round: u64,
        hqc_round: u64,
        highest_tc_round: u64,
        timeout: Arc<TwoChainTimeout>,
        aggregated_signature: B::Signature,
    ) -> Self {
        Self {
            epoch,
            round,
            hqc_round,
            highest_tc_round,
            timeout,
            aggregated_signature,
        }
    }

    /// Get the epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Get the round
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Get the highest quorum certificate round
    pub fn hqc_round(&self) -> u64 {
        self.hqc_round
    }

    /// Get the highest timeout certificate round
    pub fn highest_tc_round(&self) -> u64 {
        self.highest_tc_round
    }

    /// Get the timeout object
    pub fn timeout(&self) -> &TwoChainTimeout {
        &self.timeout
    }

    /// Verify this timeout certificate
    ///
    /// In a full implementation, this would verify the aggregated signature
    /// against the validator set. For now, we just check internal consistency.
    pub fn verify(&self) -> Result<(), Error> {
        // Verify the underlying timeout
        self.timeout.verify()?;

        // Verify internal consistency
        if self.hqc_round >= self.round {
            return Err(Error::msg(format!(
                "Invalid TC: hqc_round ({}) >= round ({})",
                self.hqc_round, self.round
            )));
        }

        if self.highest_tc_round >= self.round {
            return Err(Error::msg(format!(
                "Invalid TC: highest_tc_round ({}) >= round ({})",
                self.highest_tc_round, self.round
            )));
        }

        Ok(())
    }

    /// Check if this TC allows advancing to the next round
    ///
    /// According to the 2-chain timeout rule, we can advance if:
    /// - round == hqc_round + 1 OR round == highest_tc_round + 1
    pub fn allows_advance(&self) -> bool {
        self.round == self.hqc_round + 1 || self.round == self.highest_tc_round + 1
    }

    /// Get the "highest HQC round" for safety checks
    ///
    /// When we have a TC, the effective highest HQC round is max(hqc_round, highest_tc_round)
    pub fn effective_hqc_round(&self) -> u64 {
        self.hqc_round.max(self.highest_tc_round)
    }
}

impl<B> TimeoutCertificate for TwoChainTimeoutCertificate<B>
where
    B: Block,
{
    type Block = B;

    fn round(&self) -> u64 {
        self.round
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn highest_qc_round(&self) -> u64 {
        self.hqc_round
    }

    fn highest_tc_round(&self) -> u64 {
        self.highest_tc_round
    }

    fn verify(&self) -> Result<(), Error> {
        self.verify()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_traits::core::Hash;
    use std::hash::Hash as StdHash;

    // Mock wrapper types that implement required traits
    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    pub struct MockHash(pub u64);

    impl consensus_traits::core::Hash for MockHash {
        fn zero() -> Self {
            MockHash(0)
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockHash(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    impl consensus_traits::core::NodeId for MockHash {
        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockHash(0))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Debug)]
    struct MockTransaction;

    impl consensus_traits::block::Transaction for MockTransaction {
        type Hash = MockHash;

        fn hash(&self) -> Self::Hash {
            MockHash(0)
        }

        fn size(&self) -> usize {
            0
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, StdHash, Serialize, Deserialize)]
    struct MockSignature;

    impl consensus_traits::core::Signature for MockSignature {
        type VerifyError = std::convert::Infallible;
        type PublicKey = MockPublicKey;
        type Aggregated = MockAggregatedSignature;

        fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
            Ok(())
        }

        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockSignature)
        }

        fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
            Ok(MockAggregatedSignature)
        }
    }

    #[derive(Clone, Debug)]
    struct MockPublicKey;

    impl consensus_traits::core::PublicKey for MockPublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
            Ok(MockPublicKey)
        }
    }

    #[derive(Clone, Debug)]
    struct MockAggregatedSignature;

    #[derive(Clone, Debug)]
    struct MockQuorumCert;

    impl consensus_traits::block::QuorumCertificate for MockQuorumCert {
        type BlockMetadata = MockMetadata;
        type Hash = MockHash;

        fn certified_block(&self) -> &Self::BlockMetadata {
            static BLOCK: MockMetadata = MockMetadata;
            &BLOCK
        }

        fn block_id(&self) -> Self::Hash {
            MockHash(0)
        }

        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }

        fn from_votes<B, V>(
            _votes: &[std::sync::Arc<V>],
            _aggregated_signature: <V::Signature as consensus_traits::core::Signature>::Aggregated,
        ) -> Result<Self, Error>
        where
            B: Block,
            V: consensus_traits::Vote<Block = B>,
        {
            Ok(MockQuorumCert)
        }
    }

    // Mock block for testing
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockBlock;

    impl Block for MockBlock {
        type Transaction = MockTransaction;
        type Metadata = MockMetadata;
        type Signature = MockSignature;
        type Hash = MockHash;

        fn id(&self) -> Self::Hash {
            MockHash(0)
        }

        fn parent_id(&self) -> Self::Hash {
            MockHash(0)
        }

        fn transactions(&self) -> &[Self::Transaction] {
            &[]
        }

        fn metadata(&self) -> &Self::Metadata {
            static METADATA: MockMetadata = MockMetadata;
            &METADATA
        }

        fn signature(&self) -> Option<&Self::Signature> {
            static SIG: MockSignature = MockSignature;
            Some(&SIG)
        }

        fn verify_signature(&self) -> Result<(), Error> {
            Ok(())
        }

        fn is_genesis(&self) -> bool {
            true
        }

        fn is_nil(&self) -> bool {
            true
        }

        fn new_genesis() -> Self {
            MockBlock
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct MockMetadata;

    impl consensus_traits::block::BlockMetadata for MockMetadata {
        type QuorumCert = MockQuorumCert;
        type NodeId = MockHash;
        type Hash = MockHash;

        fn epoch(&self) -> u64 {
            0
        }

        fn round(&self) -> u64 {
            0
        }

        fn author(&self) -> Self::NodeId {
            MockHash(0)
        }

        fn parent_id(&self) -> Self::Hash {
            MockHash(0)
        }

        fn timestamp(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_timeout_creation() {
        let timeout = TwoChainTimeout::new(0, 5, 4);
        assert_eq!(timeout.epoch(), 0);
        assert_eq!(timeout.round(), 5);
        assert_eq!(timeout.hqc_round(), 4);
    }

    #[test]
    fn test_timeout_verify_valid() {
        let timeout = TwoChainTimeout::new(0, 5, 4);
        assert!(timeout.verify().is_ok());
    }

    #[test]
    fn test_timeout_verify_invalid() {
        let timeout = TwoChainTimeout::new(0, 5, 5);
        assert!(timeout.verify().is_err());
    }

    #[test]
    fn test_tc_allows_advance() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 5, 4));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 5, 4, 0, timeout, MockSignature,
        );

        // round (5) == hqc_round (4) + 1, so allows advance
        assert!(tc.allows_advance());
    }

    #[test]
    fn test_tc_effective_hqc_round() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 5, 8, timeout, MockSignature,
        );

        // effective_hqc_round should be max(5, 8) = 8
        assert_eq!(tc.effective_hqc_round(), 8);
    }

    #[test]
    fn test_timeout_default() {
        let timeout = TwoChainTimeout::default();
        assert_eq!(timeout.epoch(), 0);
        assert_eq!(timeout.round(), 0);
        assert_eq!(timeout.hqc_round(), 0);
    }

    #[test]
    fn test_timeout_verify_error_message() {
        let timeout = TwoChainTimeout::new(1, 3, 5);
        let result = timeout.verify();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("hqc_round"));
        assert!(error_msg.contains("5"));
        assert!(error_msg.contains("3"));
    }

    #[test]
    fn test_timeout_clone() {
        let timeout1 = TwoChainTimeout::new(1, 10, 9);
        let timeout2 = timeout1.clone();
        assert_eq!(timeout1, timeout2);
    }

    #[test]
    fn test_timeout_partial_eq() {
        let timeout1 = TwoChainTimeout::new(1, 5, 4);
        let timeout2 = TwoChainTimeout::new(1, 5, 4);
        assert_eq!(timeout1, timeout2);

        let timeout3 = TwoChainTimeout::new(1, 5, 3);
        assert_ne!(timeout1, timeout3);
    }

    #[test]
    fn test_tc_verify_valid() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 5, 8, timeout, MockSignature,
        );
        assert!(tc.verify().is_ok());
    }

    #[test]
    fn test_tc_verify_invalid_hqc_round() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 5, 5)); // Invalid: hqc_round >= round
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 5, 5, 0, timeout, MockSignature,
        );
        let result = tc.verify();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // The underlying timeout.verify() fails first with its own error message
        // since hqc_round >= round in the timeout itself
        assert!(error_msg.contains("hqc_round") || error_msg.contains("round"));
    }

    #[test]
    fn test_tc_verify_invalid_highest_tc_round() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 5, 3));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 5, 3, 5, // Invalid: highest_tc_round >= round
            timeout, MockSignature,
        );
        let result = tc.verify();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid TC"));
        assert!(error_msg.contains("highest_tc_round"));
    }

    #[test]
    fn test_tc_allows_advance_with_highest_tc_round() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 3));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 3, 9, // round (10) == highest_tc_round (9) + 1
            timeout, MockSignature,
        );

        assert!(tc.allows_advance());
    }

    #[test]
    fn test_tc_does_not_allow_advance() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 3));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 3, 7, // Neither condition is met
            timeout, MockSignature,
        );

        assert!(!tc.allows_advance());
    }

    #[test]
    fn test_tc_effective_hqc_round_uses_hqc_round() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 8));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 8, 5, // hqc_round (8) > highest_tc_round (5)
            timeout, MockSignature,
        );

        assert_eq!(tc.effective_hqc_round(), 8);
    }

    #[test]
    fn test_tc_clone() {
        let timeout = Arc::new(TwoChainTimeout::new(1, 5, 4));
        let tc1 = TwoChainTimeoutCertificate::<MockBlock>::new(
            1, 5, 4, 0, timeout, MockSignature,
        );
        let tc2 = tc1.clone();
        assert_eq!(tc1, tc2);
    }

    #[test]
    fn test_tc_timeout_accessor() {
        let timeout = Arc::new(TwoChainTimeout::new(1, 10, 9));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            1, 10, 9, 0, timeout.clone(), MockSignature,
        );

        assert_eq!(tc.timeout(), &*timeout);
        assert_eq!(tc.timeout().round(), 10);
    }

    #[test]
    fn test_timeout_serialization() {
        let timeout = TwoChainTimeout::new(1, 5, 4);
        let serialized = bincode::serialize(&timeout).unwrap();
        let deserialized: TwoChainTimeout = bincode::deserialize(&serialized).unwrap();
        assert_eq!(timeout, deserialized);
    }

    #[test]
    fn test_tc_serialization() {
        let timeout = Arc::new(TwoChainTimeout::new(1, 5, 4));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            1, 5, 4, 0, timeout, MockSignature,
        );

        // Note: aggregated_signature is skipped during serialization
        let serialized = bincode::serialize(&tc).unwrap();
        let deserialized: TwoChainTimeoutCertificate<MockBlock> = bincode::deserialize(&serialized).unwrap();

        assert_eq!(tc.epoch, deserialized.epoch);
        assert_eq!(tc.round, deserialized.round);
        assert_eq!(tc.hqc_round, deserialized.hqc_round);
        assert_eq!(tc.highest_tc_round, deserialized.highest_tc_round);
    }

    #[test]
    fn test_timeout_trait_impl_timeout_certificate() {
        let timeout = Arc::new(TwoChainTimeout::new(1, 10, 9));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            1, 10, 9, 0, timeout, MockSignature,
        );

        // Test TimeoutCertificate trait methods
        assert_eq!(tc.round(), 10);
        assert_eq!(tc.epoch(), 1);
        assert_eq!(tc.highest_qc_round(), 9);
        assert_eq!(tc.highest_tc_round(), 0);
    }

    #[test]
    fn test_timeout_new_zero_values() {
        let timeout = TwoChainTimeout::new(0, 0, 0);
        assert_eq!(timeout.epoch(), 0);
        assert_eq!(timeout.round(), 0);
        assert_eq!(timeout.hqc_round(), 0);

        // This should fail verification since hqc_round (0) >= round (0)
        assert!(timeout.verify().is_err());
    }

    #[test]
    fn test_timeout_debug_formatting() {
        let timeout = TwoChainTimeout::new(1, 5, 4);
        let debug_str = format!("{:?}", timeout);
        assert!(debug_str.contains("TwoChainTimeout"));
        assert!(debug_str.contains("epoch"));
        assert!(debug_str.contains("round"));
        assert!(debug_str.contains("hqc_round"));
    }

    #[test]
    fn test_tc_debug_formatting() {
        let timeout = Arc::new(TwoChainTimeout::new(1, 5, 4));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            1, 5, 4, 0, timeout, MockSignature,
        );
        let debug_str = format!("{:?}", tc);
        assert!(debug_str.contains("TwoChainTimeoutCertificate"));
    }

    #[test]
    fn test_timeout_with_epoch() {
        let timeout = TwoChainTimeout::new(10, 5, 4);
        assert_eq!(timeout.epoch(), 10);
        assert!(timeout.verify().is_ok());
    }

    #[test]
    fn test_tc_with_nonzero_epoch() {
        let timeout = Arc::new(TwoChainTimeout::new(5, 10, 9));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            5, 10, 9, 0, timeout, MockSignature,
        );
        assert_eq!(tc.epoch(), 5);
        assert_eq!(tc.round(), 10);
    }

    #[test]
    fn test_tc_all_fields_distinct() {
        let timeout = Arc::new(TwoChainTimeout::new(3, 20, 15));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            3, 20, 15, 12, timeout, MockSignature,
        );

        assert_eq!(tc.epoch(), 3);
        assert_eq!(tc.round(), 20);
        assert_eq!(tc.hqc_round(), 15);
        assert_eq!(tc.highest_tc_round(), 12);
        assert_eq!(tc.timeout().round(), 20);
    }

    #[test]
    fn test_timeout_partial_eq_different_epoch() {
        let timeout1 = TwoChainTimeout::new(1, 5, 4);
        let timeout2 = TwoChainTimeout::new(2, 5, 4);
        assert_ne!(timeout1, timeout2);
    }

    #[test]
    fn test_timeout_partial_eq_different_round() {
        let timeout1 = TwoChainTimeout::new(1, 5, 4);
        let timeout2 = TwoChainTimeout::new(1, 6, 4);
        assert_ne!(timeout1, timeout2);
    }

    #[test]
    fn test_timeout_partial_eq_different_hqc_round() {
        let timeout1 = TwoChainTimeout::new(1, 5, 4);
        let timeout2 = TwoChainTimeout::new(1, 5, 3);
        assert_ne!(timeout1, timeout2);
    }

    #[test]
    fn test_tc_verify_both_checks_fail() {
        // Create timeout where hqc_round >= round (fails first check)
        let timeout = Arc::new(TwoChainTimeout::new(0, 5, 5));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 5, 5, 5, // Both hqc_round and highest_tc_round >= round
            timeout, MockSignature,
        );
        let result = tc.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_tc_effective_hqc_round_equal() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 8));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 8, 8, // hqc_round == highest_tc_round
            timeout, MockSignature,
        );

        assert_eq!(tc.effective_hqc_round(), 8);
    }

    #[test]
    fn test_timeout_round_accessor() {
        let timeout = TwoChainTimeout::new(5, 100, 99);
        assert_eq!(timeout.round(), 100);
    }

    #[test]
    fn test_timeout_hqc_round_accessor() {
        let timeout = TwoChainTimeout::new(5, 100, 99);
        assert_eq!(timeout.hqc_round(), 99);
    }

    #[test]
    fn test_tc_hqc_round_accessor() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 5, 3, timeout, MockSignature,
        );

        assert_eq!(tc.hqc_round(), 5);
    }

    #[test]
    fn test_tc_highest_tc_round_accessor() {
        let timeout = Arc::new(TwoChainTimeout::new(0, 10, 5));
        let tc = TwoChainTimeoutCertificate::<MockBlock>::new(
            0, 10, 5, 7, timeout, MockSignature,
        );

        assert_eq!(tc.highest_tc_round(), 7);
    }
}
