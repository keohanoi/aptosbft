// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Mock types for testing.
//!
//! This module provides simple implementations of the core traits
//! for testing purposes.

use consensus_traits::core::{Error, Hash as HashTrait, NodeId as NodeIdTrait, PublicKey, Signature};

/// Mock hash type for testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
pub struct MockHash(pub u8);

impl HashTrait for MockHash {
    fn zero() -> Self {
        MockHash(0)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Empty bytes").into());
        }
        Ok(MockHash(bytes[0]))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

impl NodeIdTrait for MockHash {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Empty bytes").into());
        }
        Ok(MockHash(bytes[0]))
    }

    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Mock node ID type for testing (alias to MockHash for simplicity).
pub type MockNodeId = MockHash;

/// Helper function to get the inner u8 value from MockHash.
impl MockHash {
    /// Get the inner byte value.
    pub fn inner(&self) -> u8 {
        self.0
    }
}

/// Mock signature type for testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, std::hash::Hash)]
pub struct MockSignature(pub u8);

impl Signature for MockSignature {
    type VerifyError = std::convert::Infallible;
    type PublicKey = MockPublicKey;
    type Aggregated = MockAggregatedSignature;

    fn verify(&self, _message: &[u8], _public_key: &Self::PublicKey) -> Result<(), Self::VerifyError> {
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        vec![self.0]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Empty bytes").into());
        }
        Ok(MockSignature(bytes[0]))
    }

    fn aggregate<'a>(_signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error> {
        Ok(MockAggregatedSignature)
    }
}

/// Mock aggregated signature type for testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MockAggregatedSignature;

/// Mock public key type for testing.
#[derive(Clone, Debug)]
pub struct MockPublicKey;

impl PublicKey for MockPublicKey {
    fn to_bytes(&self) -> Vec<u8> {
        vec![]
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, Error> {
        Ok(MockPublicKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_hash_zero() {
        let hash = MockHash::zero();
        assert_eq!(hash, MockHash(0));
    }

    #[test]
    fn test_mock_hash_from_bytes() {
        let hash = <MockHash as consensus_traits::core::Hash>::from_bytes(&[42]).unwrap();
        assert_eq!(hash, MockHash(42));
    }

    #[test]
    fn test_mock_hash_from_bytes_empty() {
        assert!(<MockHash as consensus_traits::core::Hash>::from_bytes(&[]).is_err());
    }

    #[test]
    fn test_mock_signature_to_bytes() {
        let sig = MockSignature(42);
        assert_eq!(sig.to_bytes(), vec![42]);
    }

    #[test]
    fn test_mock_signature_from_bytes() {
        let sig = MockSignature::from_bytes(&[42]).unwrap();
        assert_eq!(sig, MockSignature(42));
    }

    #[test]
    fn test_mock_signature_verify() {
        let sig = MockSignature(0);
        let pk = MockPublicKey;
        assert!(sig.verify(&[], &pk).is_ok());
    }

    #[test]
    fn test_mock_signature_aggregate() {
        let sigs = vec![MockSignature(1), MockSignature(2), MockSignature(3)];
        let aggregated = MockSignature::aggregate(sigs.iter()).unwrap();
        // Just check it returns Ok
        let _ = aggregated;
    }

    #[test]
    fn test_mock_node_id_from_bytes() {
        let node_id = <MockHash as NodeIdTrait>::from_bytes(&[99]).unwrap();
        assert_eq!(node_id, MockHash(99));
    }

    #[test]
    fn test_mock_node_id_from_bytes_empty() {
        assert!(<MockHash as NodeIdTrait>::from_bytes(&[]).is_err());
    }

    #[test]
    fn test_mock_node_id_as_bytes() {
        let node_id = MockHash(42);
        let bytes = <MockHash as NodeIdTrait>::as_bytes(&node_id);
        assert_eq!(bytes, &[] as &[u8]);
    }

    #[test]
    fn test_mock_hash_inner() {
        let hash = MockHash(123);
        assert_eq!(hash.inner(), 123);
    }

    #[test]
    fn test_mock_signature_from_bytes_empty() {
        assert!(MockSignature::from_bytes(&[]).is_err());
    }

    #[test]
    fn test_mock_public_key_to_bytes() {
        let pk = MockPublicKey;
        assert_eq!(pk.to_bytes(), vec![]);
    }

    #[test]
    fn test_mock_public_key_from_bytes() {
        let pk = <MockPublicKey as PublicKey>::from_bytes(&[1, 2, 3]).unwrap();
        let _ = pk;
    }
}
