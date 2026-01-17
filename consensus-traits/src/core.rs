// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Core type traits for consensus.
//!
//! This module defines fundamental traits that all consensus implementations require.
//! These traits abstract over specific blockchain implementations, allowing the
//! consensus algorithm to work with any blockchain.

use std::fmt::Debug;

/// Error type for consensus operations.
pub type Error = anyhow::Error;

/// Errors possible during signature and voting power verification.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("Author is unknown")]
    UnknownAuthor,

    #[error(
        "The voting power ({voting_power}) is less than expected ({expected_voting_power})"
    )]
    TooLittleVotingPower {
        voting_power: u128,
        expected_voting_power: u128,
    },

    #[error("Signature verification failed")]
    InvalidSignature,

    #[error("Aggregated signature is invalid")]
    InvalidAggregatedSignature,
}

/// Unique identifier for a node in the consensus network.
///
/// # Requirements
///
/// Implementations must be:
/// - Cheap to copy (Clone + Copy)
/// - Comparable and hashable (PartialEq + Eq + Hash)
/// - Thread-safe (Send + Sync)
/// - Serializable to/from bytes
///
/// # Example
///
/// ```text
/// use consensus_traits::core::NodeId;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// struct MyNodeId([u8; 32]);
///
/// impl NodeId for MyNodeId {
///     fn from_bytes(bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
///         if bytes.len() != 32 {
///             return Err(anyhow::anyhow!("Invalid node ID length"));
///         }
///         let mut id = [0u8; 32];
///         id.copy_from_slice(bytes);
///         Ok(MyNodeId(id))
///     }
///
///     fn as_bytes(&self) -> &[u8] {
///         &self.0
///     }
/// }
/// ```text
pub trait NodeId: Clone + Copy + PartialEq + Eq + Hash + Send + Sync + Debug + 'static {
    /// Create a node ID from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the byte slice has an invalid length or format.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>;

    /// Get the byte representation of this node ID.
    fn as_bytes(&self) -> &[u8];
}

/// Cryptographic hash output.
///
/// # Requirements
///
/// Implementations must be:
/// - Cheap to copy (Clone + Copy)
/// - Comparable and hashable (PartialEq + Eq + Hash)
/// - Thread-safe (Send + Sync)
/// - Fixed size and serializable
///
/// # Example
///
/// ```text
/// use consensus_traits::core::Hash;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// struct MyHash([u8; 32]);
///
/// impl Hash for MyHash {
///     fn zero() -> Self {
///         MyHash([0u8; 32])
///     }
///
///     fn from_bytes(bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
///         if bytes.len() != 32 {
///             return Err(anyhow::anyhow!("Invalid hash length"));
///         }
///         let mut hash = [0u8; 32];
///         hash.copy_from_slice(bytes);
///         Ok(MyHash(hash))
///     }
///
///     fn as_bytes(&self) -> &[u8] {
///         &self.0
///     }
/// }
/// ```text
pub trait Hash: Clone + Copy + PartialEq + Eq + std::hash::Hash + Send + Sync + Debug + 'static {
    /// Create a zero hash value.
    fn zero() -> Self;

    /// Create a hash from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the byte slice has an invalid length.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>;

    /// Get the byte representation of this hash.
    fn as_bytes(&self) -> &[u8];
}

/// Digital signature for blocks and votes.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Verifiable against a public key
/// - Serializable to/from bytes
///
/// # Example
///
/// ```text
/// use consensus_traits::core::{Signature, PublicKey};
///
/// struct MySignature(Vec<u8>);
///
/// impl Signature for MySignature {
///     type VerifyError = anyhow::Error;
///     type PublicKey = MyPublicKey;
///
///     fn verify(
///         &self,
///         message: &[u8],
///         public_key: &MyPublicKey,
///     ) -> Result<(), Self::VerifyError> {
///         // Verify signature logic
///         Ok(())
///     }
///
///     fn to_bytes(&self) -> Vec<u8> {
///         self.0.clone()
///     }
///
///     fn from_bytes(bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
///         Ok(MySignature(bytes.to_vec()))
///     }
/// }
/// ```text
pub trait Signature: Clone + PartialEq + Eq + std::hash::Hash + Send + Sync + Debug + 'static {
    /// Error type for signature verification failures.
    type VerifyError: std::error::Error + Send + Sync;

    /// Associated public key type for signature verification.
    type PublicKey: PublicKey;

    /// Aggregated signature type (for BLS signature aggregation).
    ///
    /// This allows multiple signatures to be combined into a single compact signature,
    /// which is essential for efficient quorum certificate formation.
    type Aggregated: Clone + Send + Sync + Debug + 'static;

    /// Verify this signature against a message and public key.
    ///
    /// # Errors
    ///
    /// Returns an error if the signature is invalid.
    fn verify(&self, message: &[u8], public_key: &Self::PublicKey) -> Result<(), Self::VerifyError>;

    /// Serialize this signature to bytes.
    fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize a signature from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes are malformed.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>;

    /// Aggregate multiple signatures into a single signature.
    ///
    /// This is used for creating quorum certificates from individual validator signatures.
    /// BLS signatures support this natively, allowing efficient aggregation.
    ///
    /// # Errors
    ///
    /// Returns an error if the signatures cannot be aggregated.
    fn aggregate<'a>(signatures: impl Iterator<Item = &'a Self>) -> Result<Self::Aggregated, Error>;
}

/// Public key for signature verification.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Serializable to/from bytes
///
/// # Example
///
/// ```text
/// use consensus_traits::core::PublicKey;
///
/// #[derive(Clone)]
/// struct MyPublicKey(Vec<u8>);
///
/// impl PublicKey for MyPublicKey {
///     fn to_bytes(&self) -> Vec<u8> {
///         self.0.clone()
///     }
///
///     fn from_bytes(bytes: &[u8]) -> Result<Self, consensus_traits::core::Error> {
///         Ok(MyPublicKey(bytes.to_vec()))
///     }
/// }
/// ```text
pub trait PublicKey: Clone + Send + Sync + Debug + 'static {
    /// Serialize this public key to bytes.
    fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize a public key from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes are malformed.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Example implementations for testing
    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestNodeId([u8; 32]);

    impl Hash for TestNodeId {
        fn zero() -> Self {
            TestNodeId([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length"));
            }
            let mut id = [0u8; 32];
            id.copy_from_slice(bytes);
            Ok(TestNodeId(id))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    impl NodeId for TestNodeId {
        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length"));
            }
            let mut id = [0u8; 32];
            id.copy_from_slice(bytes);
            Ok(TestNodeId(id))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestHash([u8; 32]);

    impl Hash for TestHash {
        fn zero() -> Self {
            TestHash([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
            if bytes.len() != 32 {
                return Err(anyhow::anyhow!("Invalid length"));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(bytes);
            Ok(TestHash(hash))
        }

        fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    #[test]
    fn test_node_id_roundtrip() {
        let bytes = [1u8; 32];
        let id = <TestNodeId as NodeId>::from_bytes(&bytes).unwrap();
        assert_eq!(NodeId::as_bytes(&id), &bytes);
    }

    #[test]
    fn test_hash_zero() {
        let hash = TestHash::zero();
        assert_eq!(hash.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_hash_roundtrip() {
        let bytes = [42u8; 32];
        let hash = <TestHash as Hash>::from_bytes(&bytes).unwrap();
        assert_eq!(Hash::as_bytes(&hash), &bytes);
    }
}
