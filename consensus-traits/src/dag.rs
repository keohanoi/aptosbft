// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! # DAG Consensus Traits
//!
//! This module defines traits for DAG-based consensus protocols.
//!
//! ## Overview
//!
//! DAG (Directed Acyclic Graph) consensus enables parallel block proposal
//! and improved throughput compared to linear consensus. Instead of a single
//! block per round, validators can propose nodes in parallel, forming a DAG
//! structure.
//!
//! ## Key Concepts
//!
//! - **DAG Node**: A proposed block with parents from previous round
//! - **Node Certificate**: Quorum signatures over a node digest
//! - **Certified Node**: A node with enough votes to be certified
//! - **Anchor**: Special nodes selected for commit decisions
//! - **Ordering**: The process of linearizing the DAG into commits

use crate::core::{Hash as HashTrait, Signature};
use std::fmt::{Debug, Display};

/// Unique identifier for a DAG node.
///
/// A DAG node is uniquely identified by its epoch, round, and author.
pub trait DagNodeId: Clone + Debug + Display + Eq + PartialEq + Ord + PartialOrd + Send + Sync + 'static {
    /// The epoch type
    type Epoch: Clone + Debug + Display + Eq + PartialEq + Send + Sync + 'static;

    /// The round type
    type Round: Clone + Debug + Display + Eq + PartialEq + Ord + PartialOrd + Send + Sync + 'static;

    /// The node identifier (author) type
    type Author: Clone + Debug + Display + Eq + PartialEq + HashTrait + Send + Sync + 'static;

    /// Create a new DAG node ID
    fn new(epoch: Self::Epoch, round: Self::Round, author: Self::Author) -> Self;

    /// Get the epoch
    fn epoch(&self) -> &Self::Epoch;

    /// Get the round
    fn round(&self) -> &Self::Round;

    /// Get the author
    fn author(&self) -> &Self::Author;
}

/// Metadata for a DAG node, without payload or parents.
///
/// This contains the information needed to identify and verify a node,
/// excluding the payload and parent certificates (which are only needed
/// for full node data).
pub trait DagNodeMetadata: Clone + Debug + Send + Sync + 'static {
    /// The node ID type
    type NodeId: DagNodeId;

    /// The hash type for digests
    type Hash: HashTrait;

    /// Get the node ID
    fn node_id(&self) -> &Self::NodeId;

    /// Get the epoch
    fn epoch(&self) -> <<Self as DagNodeMetadata>::NodeId as DagNodeId>::Epoch;

    /// Get the round
    fn round(&self) -> <<Self as DagNodeMetadata>::NodeId as DagNodeId>::Round;

    /// Get the author
    fn author(&self) -> &<<Self as DagNodeMetadata>::NodeId as DagNodeId>::Author;

    /// Get the timestamp when the node was created
    fn timestamp(&self) -> u64;

    /// Get the node digest (hash of node contents)
    fn digest(&self) -> &Self::Hash;
}

/// Payload for a DAG node.
///
/// The payload contains the actual data to be committed (transactions,
/// state updates, etc.).
pub trait DagPayload: Clone + Debug + Send + Sync + 'static {
    /// The hash type
    type Hash: HashTrait;

    /// Get the hash of this payload
    fn hash(&self) -> Self::Hash;

    /// Check if the payload is empty
    fn is_empty(&self) -> bool;

    /// Get the size of the payload in bytes
    fn size(&self) -> usize;
}

/// Parent certificates for a DAG node.
///
/// Parents are certified nodes from the previous round that this node
/// builds upon.
pub trait ParentCertificates: Clone + Debug + Send + Sync + 'static {
    /// The certificate metadata type
    type CertificateMetadata: DagNodeMetadata;

    /// Iterator over parent certificates
    type Iter<'a>: Iterator<Item = &'a Self::CertificateMetadata>
    where
        Self: 'a;

    /// Get the number of parents
    fn len(&self) -> usize;

    /// Check if there are no parents
    fn is_empty(&self) -> bool;

    /// Iterate over parent metadata
    fn iter(&self) -> Self::Iter<'_>;

    /// Add a parent certificate
    fn add(&mut self, certificate: Self::CertificateMetadata);
}

/// A DAG node with payload and metadata.
///
/// This is the main unit of the DAG consensus protocol. Each node contains
/// a payload, metadata identifying it, and references to parent nodes from
/// the previous round.
pub trait DagNode: Clone + Debug + Send + Sync + 'static {
    /// The metadata type
    type Metadata: DagNodeMetadata;

    /// The payload type
    type Payload: DagPayload;

    /// The parent certificates type
    type Parents: ParentCertificates<CertificateMetadata = Self::Metadata>;

    /// Get the node metadata
    fn metadata(&self) -> &Self::Metadata;

    /// Get the node payload
    fn payload(&self) -> &Self::Payload;

    /// Get the parent certificates
    fn parents(&self) -> &Self::Parents;

    /// Get the timestamp
    fn timestamp(&self) -> u64 {
        self.metadata().timestamp()
    }

    /// Get the node ID
    fn node_id(&self) -> &<Self::Metadata as DagNodeMetadata>::NodeId {
        self.metadata().node_id()
    }
}

/// Node certificate with quorum signatures.
///
/// A node certificate proves that a quorum of validators have signed
/// the node's metadata (digest).
pub trait NodeCertificate: Clone + Debug + Send + Sync + 'static {
    /// The metadata type
    type Metadata: DagNodeMetadata;

    /// The aggregated signature type
    type AggregatedSignature: Clone + Debug + Send + Sync + 'static;

    /// Get the node metadata
    fn metadata(&self) -> &Self::Metadata;

    /// Get the aggregated signature
    fn signatures(&self) -> &Self::AggregatedSignature;
}

/// A certified DAG node.
///
/// A certified node is a node with a certificate proving quorum signatures.
/// This is the unit that can be relied upon for ordering decisions.
pub trait CertifiedNode: Clone + Debug + Send + Sync + 'static {
    /// The node type
    type Node: DagNode;

    /// The certificate type
    type Certificate: NodeCertificate<Metadata = <Self::Node as DagNode>::Metadata>;

    /// Get the underlying node
    fn node(&self) -> &Self::Node;

    /// Get the certificate
    fn certificate(&self) -> &Self::Certificate;
}

/// A vote for a DAG node.
///
/// Votes are individual validator signatures on a node's metadata.
/// When enough votes are collected, they form a NodeCertificate.
pub trait DagVote: Clone + Debug + Send + Sync + 'static {
    /// The metadata type
    type Metadata: DagNodeMetadata;

    /// The signature type
    type Signature: Signature;

    /// Get the node metadata being voted on
    fn metadata(&self) -> &Self::Metadata;

    /// Get the signature
    fn signature(&self) -> &Self::Signature;
}

/// Status of a DAG node in the ordering process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    /// Node has been created but not yet ordered
    Unordered,
    /// Node has been ordered (can be committed)
    Ordered,
    /// Node has been committed to the chain
    Committed,
}

/// Result of attempting to order a node.
#[derive(Clone, Debug)]
pub enum OrderDecision<N>
where
    N: CertifiedNode,
{
    /// Node can be ordered
    Ordered(N),
    /// Node cannot be ordered yet (waiting for more votes/ancestors)
    Pending,
    /// Node is invalid or cannot be ordered (with error message)
    Invalid(String),
}

/// DAG-related errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DagError {
    /// Node not found in DAG
    NodeNotFound,
    /// Invalid node structure (e.g., wrong parent round)
    InvalidNodeStructure,
    /// Invalid parent certificates
    InvalidParents,
    /// Certificate verification failed
    CertificateVerificationFailed,
    /// Not enough voting power for quorum
    InsufficientVotingPower,
    /// Invalid round number
    InvalidRound,
    /// Node already exists
    NodeAlreadyExists,
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagError::NodeNotFound => write!(f, "Node not found in DAG"),
            DagError::InvalidNodeStructure => write!(f, "Invalid node structure"),
            DagError::InvalidParents => write!(f, "Invalid parent certificates"),
            DagError::CertificateVerificationFailed => write!(f, "Certificate verification failed"),
            DagError::InsufficientVotingPower => write!(f, "Insufficient voting power for quorum"),
            DagError::InvalidRound => write!(f, "Invalid round number"),
            DagError::NodeAlreadyExists => write!(f, "Node already exists"),
        }
    }
}

impl std::error::Error for DagError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_error_display() {
        assert_eq!(format!("{}", DagError::NodeNotFound), "Node not found in DAG");
        assert_eq!(format!("{}", DagError::InvalidNodeStructure), "Invalid node structure");
    }

    #[test]
    fn test_node_status_equality() {
        assert_eq!(NodeStatus::Unordered, NodeStatus::Unordered);
        assert_ne!(NodeStatus::Unordered, NodeStatus::Ordered);
    }

    #[test]
    fn test_dag_error_into_anyhow() {
        // Ensure DagError can be converted to anyhow::Error
        let err: anyhow::Error = DagError::NodeNotFound.into();
        assert!(err.to_string().contains("Node not found"));
    }

    #[test]
    fn test_dag_error_display_all_variants() {
        assert_eq!(format!("{}", DagError::NodeNotFound), "Node not found in DAG");
        assert_eq!(format!("{}", DagError::InvalidNodeStructure), "Invalid node structure");
        assert_eq!(format!("{}", DagError::InvalidParents), "Invalid parent certificates");
        assert_eq!(format!("{}", DagError::CertificateVerificationFailed), "Certificate verification failed");
        assert_eq!(format!("{}", DagError::InsufficientVotingPower), "Insufficient voting power for quorum");
        assert_eq!(format!("{}", DagError::InvalidRound), "Invalid round number");
        assert_eq!(format!("{}", DagError::NodeAlreadyExists), "Node already exists");
    }

    #[test]
    fn test_node_status_all_variants() {
        // Test equality
        assert_eq!(NodeStatus::Unordered, NodeStatus::Unordered);
        assert_eq!(NodeStatus::Ordered, NodeStatus::Ordered);
        assert_eq!(NodeStatus::Committed, NodeStatus::Committed);

        // Test inequality
        assert_ne!(NodeStatus::Unordered, NodeStatus::Ordered);
        assert_ne!(NodeStatus::Ordered, NodeStatus::Committed);
        assert_ne!(NodeStatus::Unordered, NodeStatus::Committed);
    }

    #[test]
    fn test_node_status_equality_reflexive() {
        // Test equality is reflexive (x == x)
        let status = NodeStatus::Ordered;
        assert_eq!(status, status);

        // Test all variants
        assert_eq!(NodeStatus::Unordered, NodeStatus::Unordered);
        assert_eq!(NodeStatus::Ordered, NodeStatus::Ordered);
        assert_eq!(NodeStatus::Committed, NodeStatus::Committed);
    }

    #[test]
    fn test_dag_error_clone_all_variants() {
        let err1 = DagError::NodeNotFound;
        let err2 = err1.clone();
        assert_eq!(err1, err2);

        let err3 = DagError::InvalidRound;
        let err4 = err3.clone();
        assert_eq!(err3, err4);

        let err5 = DagError::InsufficientVotingPower;
        let err6 = err5.clone();
        assert_eq!(err5, err6);
    }

    #[test]
    fn test_dag_error_partial_eq_all_variants() {
        // Test that all error types support PartialEq
        assert_eq!(DagError::NodeNotFound, DagError::NodeNotFound);
        assert_eq!(DagError::InvalidNodeStructure, DagError::InvalidNodeStructure);
        assert_eq!(DagError::InvalidParents, DagError::InvalidParents);

        // Test inequality
        assert_ne!(DagError::NodeNotFound, DagError::InvalidRound);
        assert_ne!(DagError::NodeAlreadyExists, DagError::InvalidParents);
    }

    #[test]
    fn test_dag_error_send_sync_static() {
        // Verify all error types are Send + Sync + 'static
        fn is_send_sync<T: std::marker::Send + std::marker::Sync>() {}

        // All DagError variants should be Send + Sync + 'static
        is_send_sync::<DagError>();
    }

    #[test]
    fn test_node_status_copy_and_clone() {
        // Test that NodeStatus supports Copy and Clone traits
        let status1 = NodeStatus::Unordered;
        let status2 = status1; // Copy happens here

        // Both should be valid after copy
        assert_eq!(status1, NodeStatus::Unordered);
        assert_eq!(status2, NodeStatus::Unordered);

        // Test explicit clone
        let status3 = NodeStatus::Ordered;
        let status4 = status3.clone();
        assert_eq!(status3, status4);
    }
}
