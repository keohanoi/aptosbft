// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Concrete DAG type implementations.
//!
//! This module provides generic implementations of the DAG traits that can be
//! used with any blockchain implementation.

use consensus_traits::{
    dag::{
        DagNodeId as DagNodeIdTrait, DagNodeMetadata, DagPayload, DagNode,
        ParentCertificates, NodeCertificate, CertifiedNode, DagVote,
    },
    core::Hash as HashTrait,
};
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::Hash as StdHash,
    sync::Arc,
};

/// Generic DAG node ID implementation.
///
/// This is a concrete implementation of `DagNodeId` that works with any
/// epoch, round, and author types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DagNodeId<E, R, A> {
    epoch: E,
    round: R,
    author: A,
}

impl<E, R, A> DagNodeId<E, R, A>
where
    E: Clone + PartialEq + Eq + StdHash + fmt::Debug + Send + Sync + 'static,
    R: Clone + PartialEq + Eq + Ord + PartialOrd + StdHash + fmt::Debug + Send + Sync + 'static,
    A: Clone + PartialEq + Eq + StdHash + fmt::Debug + Send + Sync + HashTrait + 'static,
{
    /// Create a new DAG node ID.
    pub fn new(epoch: E, round: R, author: A) -> Self {
        Self { epoch, round, author }
    }

    /// Get the epoch
    pub fn epoch(&self) -> &E {
        &self.epoch
    }

    /// Get the round
    pub fn round(&self) -> &R {
        &self.round
    }

    /// Get the author
    pub fn author(&self) -> &A {
        &self.author
    }
}

impl<E, R, A> DagNodeIdTrait for DagNodeId<E, R, A>
where
    E: Clone + Display + PartialEq + Eq + StdHash + fmt::Debug + Send + Sync + HashTrait + 'static,
    R: Clone + Display + PartialEq + Eq + Ord + PartialOrd + StdHash + fmt::Debug + Send + Sync + 'static,
    A: Clone + Display + PartialEq + Eq + StdHash + fmt::Debug + Send + Sync + HashTrait + 'static,
{
    type Epoch = E;
    type Round = R;
    type Author = A;

    fn new(epoch: Self::Epoch, round: Self::Round, author: Self::Author) -> Self {
        Self { epoch, round, author }
    }

    fn epoch(&self) -> &Self::Epoch {
        &self.epoch
    }

    fn round(&self) -> &Self::Round {
        &self.round
    }

    fn author(&self) -> &Self::Author {
        &self.author
    }
}

impl<E, R, A> PartialOrd for DagNodeId<E, R, A>
where
    E: PartialEq + Eq,
    R: PartialOrd,
    A: PartialEq + Eq,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.round.partial_cmp(&other.round) {
            Some(Ordering::Equal) => Some(Ordering::Equal),
            other => other,
        }
    }
}

impl<E, R, A> Ord for DagNodeId<E, R, A>
where
    E: PartialEq + Eq,
    R: Ord,
    A: PartialEq + Eq,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.round.cmp(&other.round)
    }
}

impl<E, R, A> StdHash for DagNodeId<E, R, A>
where
    E: StdHash,
    R: StdHash,
    A: StdHash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.epoch.hash(state);
        self.round.hash(state);
        self.author.hash(state);
    }
}

impl<E, R, A> Display for DagNodeId<E, R, A>
where
    E: Display,
    R: Display,
    A: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "DagNodeId[epoch={}, round={}, author={}]", self.epoch, self.round, self.author)
    }
}

/// Generic DAG node metadata implementation.
#[derive(Clone, Debug)]
pub struct DagNodeMetadataImpl<ID, H> {
    node_id: ID,
    timestamp: u64,
    digest: H,
}

impl<ID, H> DagNodeMetadataImpl<ID, H>
where
    ID: DagNodeIdTrait,
    H: HashTrait,
{
    /// Create new DAG node metadata.
    pub fn new(node_id: ID, timestamp: u64, digest: H) -> Self {
        Self {
            node_id,
            timestamp,
            digest,
        }
    }

    /// Get the node ID
    pub fn node_id(&self) -> &ID {
        &self.node_id
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get the digest
    pub fn digest(&self) -> &H {
        &self.digest
    }
}

impl<ID, H> DagNodeMetadata for DagNodeMetadataImpl<ID, H>
where
    ID: DagNodeIdTrait + 'static,
    H: HashTrait + 'static,
{
    type NodeId = ID;
    type Hash = H;

    fn node_id(&self) -> &Self::NodeId {
        &self.node_id
    }

    fn epoch(&self) -> <Self::NodeId as DagNodeIdTrait>::Epoch {
        self.node_id.epoch().clone()
    }

    fn round(&self) -> <Self::NodeId as DagNodeIdTrait>::Round {
        self.node_id.round().clone()
    }

    fn author(&self) -> &<Self::NodeId as DagNodeIdTrait>::Author {
        self.node_id.author()
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn digest(&self) -> &Self::Hash {
        &self.digest
    }
}

/// Generic DAG payload implementation.
#[derive(Clone, Debug)]
pub struct DagPayloadImpl<H> {
    data: Vec<u8>,
    hash: H,
}

impl<H> DagPayloadImpl<H>
where
    H: HashTrait,
{
    /// Create a new DAG payload from raw data.
    pub fn new(data: Vec<u8>, hash: H) -> Self {
        Self { data, hash }
    }

    /// Get the raw payload data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Create an empty payload
    pub fn empty() -> Self
    where
        H: HashTrait,
    {
        Self {
            data: vec![],
            hash: H::zero(),
        }
    }
}

impl<H> DagPayload for DagPayloadImpl<H>
where
    H: HashTrait + 'static,
{
    type Hash = H;

    fn hash(&self) -> Self::Hash {
        self.hash
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn size(&self) -> usize {
        self.data.len()
    }
}

/// Generic parent certificates implementation.
#[derive(Clone, Debug)]
pub struct ParentCertificatesImpl<M> {
    parents: Vec<M>,
}

impl<M> ParentCertificatesImpl<M>
where
    M: DagNodeMetadata,
{
    /// Create new parent certificates from metadata.
    pub fn new(parents: Vec<M>) -> Self {
        Self { parents }
    }

    /// Get the parent metadata
    pub fn parents(&self) -> &[M] {
        &self.parents
    }

    /// Create empty parents (for genesis/round 1)
    pub fn empty() -> Self {
        Self { parents: vec![] }
    }

    /// Add a parent certificate
    pub fn add(&mut self, certificate: M) {
        self.parents.push(certificate);
    }
}

impl<M> ParentCertificates for ParentCertificatesImpl<M>
where
    M: DagNodeMetadata + 'static,
{
    type CertificateMetadata = M;

    type Iter<'a> = std::slice::Iter<'a, M>
    where
        Self: 'a;

    fn len(&self) -> usize {
        self.parents.len()
    }

    fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.parents.iter()
    }

    fn add(&mut self, certificate: Self::CertificateMetadata) {
        self.parents.push(certificate);
    }
}

/// Generic DAG node implementation.
#[derive(Clone, Debug)]
pub struct DagNodeImpl<M, P, Parents> {
    metadata: M,
    payload: P,
    parents: Parents,
}

impl<M, P, Parents> DagNodeImpl<M, P, Parents>
where
    M: DagNodeMetadata,
    P: DagPayload,
    Parents: ParentCertificates<CertificateMetadata = M>,
{
    /// Create a new DAG node.
    pub fn new(metadata: M, payload: P, parents: Parents) -> Self {
        Self {
            metadata,
            payload,
            parents,
        }
    }

    /// Get the metadata
    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    /// Get the payload
    pub fn payload(&self) -> &P {
        &self.payload
    }

    /// Get the parents
    pub fn parents(&self) -> &Parents {
        &self.parents
    }
}

impl<M, P, Parents> DagNode for DagNodeImpl<M, P, Parents>
where
    M: DagNodeMetadata + 'static,
    P: DagPayload + 'static,
    Parents: ParentCertificates<CertificateMetadata = M> + 'static,
{
    type Metadata = M;
    type Payload = P;
    type Parents = Parents;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn payload(&self) -> &Self::Payload {
        &self.payload
    }

    fn parents(&self) -> &Self::Parents {
        &self.parents
    }
}

/// Generic node certificate implementation.
#[derive(Clone, Debug)]
pub struct NodeCertificateImpl<M, Sig> {
    metadata: M,
    signatures: Sig,
}

impl<M, Sig> NodeCertificateImpl<M, Sig>
where
    M: DagNodeMetadata,
{
    /// Create a new node certificate.
    pub fn new(metadata: M, signatures: Sig) -> Self {
        Self {
            metadata,
            signatures,
        }
    }

    /// Get the metadata
    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    /// Get the signatures
    pub fn signatures(&self) -> &Sig {
        &self.signatures
    }
}

impl<M, Sig> NodeCertificate for NodeCertificateImpl<M, Sig>
where
    M: DagNodeMetadata + 'static,
    Sig: Clone + fmt::Debug + Send + Sync + 'static,
{
    type Metadata = M;
    type AggregatedSignature = Sig;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn signatures(&self) -> &Self::AggregatedSignature {
        &self.signatures
    }
}

/// Generic certified node implementation.
#[derive(Clone, Debug)]
pub struct CertifiedNodeImpl<N, C> {
    node: Arc<N>,
    certificate: C,
}

impl<N, C> CertifiedNodeImpl<N, C>
where
    N: DagNode,
    C: NodeCertificate<Metadata = N::Metadata>,
{
    /// Create a new certified node.
    pub fn new(node: N, certificate: C) -> Self {
        Self {
            node: Arc::new(node),
            certificate,
        }
    }

    /// Get the node
    pub fn node(&self) -> &N {
        &self.node
    }

    /// Get the certificate
    pub fn certificate(&self) -> &C {
        &self.certificate
    }
}

impl<N, C> CertifiedNode for CertifiedNodeImpl<N, C>
where
    N: DagNode + 'static,
    C: NodeCertificate<Metadata = <N as DagNode>::Metadata> + 'static,
{
    type Node = N;
    type Certificate = C;

    fn node(&self) -> &Self::Node {
        &self.node
    }

    fn certificate(&self) -> &Self::Certificate {
        &self.certificate
    }
}

/// Generic DAG vote implementation.
#[derive(Clone, Debug)]
pub struct DagVoteImpl<M, S> {
    metadata: M,
    signature: S,
}

impl<M, S> DagVoteImpl<M, S>
where
    M: DagNodeMetadata,
{
    /// Create a new DAG vote.
    pub fn new(metadata: M, signature: S) -> Self {
        Self {
            metadata,
            signature,
        }
    }

    /// Get the metadata
    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    /// Get the signature
    pub fn signature(&self) -> &S {
        &self.signature
    }
}

impl<M, S> DagVote for DagVoteImpl<M, S>
where
    M: DagNodeMetadata + 'static,
    S: consensus_traits::core::Signature + 'static,
{
    type Metadata = M;
    type Signature = S;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn signature(&self) -> &Self::Signature {
        &self.signature
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::hash::Hash as StdHash;

    // Simple test types with HashTrait implementation
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
    struct TestEpoch(u64);

    impl fmt::Display for TestEpoch {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestEpoch {
        fn zero() -> Self {
            TestEpoch(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestEpoch(u64::from_be_bytes(
                bytes.try_into().unwrap_or([0u8; 8]),
            )))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, StdHash)]
    struct TestRound(u64);

    impl fmt::Display for TestRound {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestRound {
        fn zero() -> Self {
            TestRound(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestRound(u64::from_be_bytes(
                bytes.try_into().unwrap_or([0u8; 8]),
            )))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    struct TestAuthor(u8);

    impl fmt::Display for TestAuthor {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HashTrait for TestAuthor {
        fn zero() -> Self {
            TestAuthor(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestAuthor(bytes.get(0).copied().unwrap_or(0)))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, StdHash)]
    struct TestHash(u8);

    impl HashTrait for TestHash {
        fn zero() -> Self {
            TestHash(0)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
            Ok(TestHash(bytes.get(0).copied().unwrap_or(0)))
        }

        fn as_bytes(&self) -> &[u8] {
            &[]
        }
    }

    type TestDagNodeId = DagNodeId<TestEpoch, TestRound, TestAuthor>;

    #[test]
    fn test_dag_node_id_creation() {
        let id = TestDagNodeId::new(TestEpoch(1), TestRound(2), TestAuthor(3));
        assert_eq!(id.epoch(), &TestEpoch(1));
        assert_eq!(id.round(), &TestRound(2));
        assert_eq!(id.author(), &TestAuthor(3));
    }

    #[test]
    fn test_dag_node_id_ord() {
        let id1 = TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1));
        let id2 = TestDagNodeId::new(TestEpoch(1), TestRound(2), TestAuthor(1));
        assert!(id1 < id2);
    }

    #[test]
    fn test_parent_certificates_empty() {
        let parents = ParentCertificatesImpl::<DagNodeMetadataImpl<TestDagNodeId, TestHash>>::empty();
        assert!(parents.is_empty());
        assert_eq!(parents.len(), 0);
    }

    #[test]
    fn test_parent_certificates_add() {
        let mut parents = ParentCertificatesImpl::new(vec![]);
        assert!(parents.is_empty());

        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        parents.add(metadata.clone());
        assert_eq!(parents.len(), 1);
        assert!(!parents.is_empty());
    }

    #[test]
    fn test_dag_payload_empty() {
        let payload = DagPayloadImpl::<TestHash>::empty();
        assert!(payload.is_empty());
        assert_eq!(payload.size(), 0);
    }

    #[test]
    fn test_dag_payload_hash() {
        let payload = DagPayloadImpl::new(vec![1, 2, 3], TestHash(42));
        assert_eq!(payload.hash(), TestHash(42));
        assert!(!payload.is_empty());
        assert_eq!(payload.size(), 3);
    }

    #[test]
    fn test_dag_node_metadata_impl() {
        let id = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));
        let metadata = DagNodeMetadataImpl::new(id.clone(), 12345, TestHash(99));

        assert_eq!(metadata.node_id(), &id);
        assert_eq!(metadata.timestamp(), 12345);
        assert_eq!(metadata.digest(), &TestHash(99));
    }

    #[test]
    fn test_dag_node_metadata_impl_display() {
        let id = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));
        let metadata = DagNodeMetadataImpl::new(id, 12345, TestHash(99));

        let display = format!("{:?}", metadata);
        assert!(display.contains("DagNodeMetadataImpl"));
    }

    #[test]
    fn test_parent_certificates_iter() {
        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let parents = ParentCertificatesImpl::new(vec![metadata.clone()]);

        let mut count = 0;
        for p in parents.iter() {
            assert_eq!(p.timestamp(), 100);
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_parent_certificates_parents() {
        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let parents = ParentCertificatesImpl::new(vec![metadata.clone()]);

        assert_eq!(parents.parents().len(), 1);
        assert_eq!(parents.parents()[0].timestamp(), 100);
    }

    #[test]
    fn test_parent_certificates_add_via_trait() {
        let mut parents = ParentCertificatesImpl::new(vec![]);
        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        parents.add(metadata.clone());
        assert_eq!(parents.len(), 1);
    }

    #[test]
    fn test_dag_node_impl() {
        use consensus_traits::dag::DagNode;

        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let payload = DagPayloadImpl::new(vec![1, 2], TestHash(42));
        let parents = ParentCertificatesImpl::empty();

        let node = DagNodeImpl::new(metadata.clone(), payload, parents);

        assert_eq!(node.metadata().timestamp(), 100);
        assert_eq!(node.payload().hash(), TestHash(42));
    }

    #[test]
    fn test_node_certificate_impl() {
        use consensus_traits::dag::NodeCertificate;

        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let sig = TestHash(42);

        let cert = NodeCertificateImpl::new(metadata.clone(), sig);
        assert_eq!(cert.metadata().timestamp(), 100);
        assert_eq!(cert.signatures(), &sig);
    }

    #[test]
    fn test_certified_node_impl() {
        use consensus_traits::dag::CertifiedNode;

        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let payload = DagPayloadImpl::new(vec![1, 2], TestHash(42));
        let parents = ParentCertificatesImpl::empty();
        let node = DagNodeImpl::new(metadata.clone(), payload, parents);

        let cert = NodeCertificateImpl::new(metadata, TestHash(99));
        let certified = CertifiedNodeImpl::new(node, cert);

        assert_eq!(certified.certificate().signatures(), &TestHash(99));
    }

    #[test]
    fn test_dag_vote_impl() {
        use consensus_traits::dag::DagVote;

        let metadata = DagNodeMetadataImpl::new(
            TestDagNodeId::new(TestEpoch(1), TestRound(1), TestAuthor(1)),
            100,
            TestHash(1),
        );
        let sig = TestHash(42);

        let vote = DagVoteImpl::new(metadata.clone(), sig);
        assert_eq!(vote.metadata().timestamp(), 100);
        assert_eq!(vote.signature(), &sig);
    }

    #[test]
    fn test_dag_node_id_display() {
        let id = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));
        let display = format!("{}", id);
        assert!(display.contains("epoch=5"));
        assert!(display.contains("round=10"));
        assert!(display.contains("author=3"));
    }

    #[test]
    fn test_dag_node_id_hash() {
        use std::hash::Hasher;

        let id1 = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));
        let id2 = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));

        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();

        id1.hash(&mut h1);
        id2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_dag_node_id_clone() {
        let id1 = TestDagNodeId::new(TestEpoch(5), TestRound(10), TestAuthor(3));
        let id2 = id1.clone();

        assert_eq!(id1.epoch(), id2.epoch());
        assert_eq!(id1.round(), id2.round());
        assert_eq!(id1.author(), id2.author());
    }

    #[test]
    fn test_dag_node_id_partial_ord_different_rounds() {
        let id1 = TestDagNodeId::new(TestEpoch(1), TestRound(5), TestAuthor(1));
        let id2 = TestDagNodeId::new(TestEpoch(1), TestRound(10), TestAuthor(1));

        assert_eq!(id1.partial_cmp(&id2), Some(std::cmp::Ordering::Less));
        assert_eq!(id2.partial_cmp(&id1), Some(std::cmp::Ordering::Greater));
    }

    #[test]
    fn test_dag_node_id_partial_ord_same_round() {
        let id1 = TestDagNodeId::new(TestEpoch(1), TestRound(5), TestAuthor(1));
        let id2 = TestDagNodeId::new(TestEpoch(1), TestRound(5), TestAuthor(2));

        // Same round = Equal
        assert_eq!(id1.partial_cmp(&id2), Some(std::cmp::Ordering::Equal));
    }

    #[test]
    fn test_dag_node_id_cmp() {
        let id1 = TestDagNodeId::new(TestEpoch(1), TestRound(5), TestAuthor(1));
        let id2 = TestDagNodeId::new(TestEpoch(1), TestRound(10), TestAuthor(1));

        assert!(id1.cmp(&id2) == std::cmp::Ordering::Less);
        assert!(id2.cmp(&id1) == std::cmp::Ordering::Greater);
    }
}
