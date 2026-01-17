// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Network and validator traits for consensus.
//!
//! This module defines traits for network communication and validator management.

use crate::block::{Block, Vote};
use crate::core::NodeId;
use async_trait::async_trait;

/// Validator information.
///
/// Contains metadata about a validator in the network.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Serializable for network transmission
///
/// # Example
///
/// ```text
/// use consensus_traits::core::{NodeId, PublicKey};
/// use consensus_traits::network::ValidatorInfo;
///
/// #[derive(Clone)]
/// struct MyValidator {
///     id: [u8; 32],
///     public_key: MyPublicKey,
///     stake: u64,
/// }
///
/// impl ValidatorInfo for MyValidator {
///     type NodeId = [u8; 32];
///     type PublicKey = MyPublicKey;
///
///     fn id(&self) -> Self::NodeId {
///         self.id
///     }
///
///     fn public_key(&self) -> &Self::PublicKey {
///         &self.public_key
///     }
///
///     fn voting_power(&self) -> u64 {
///         self.stake
///     }
/// }
/// ```text
pub trait ValidatorInfo: Clone + Send + Sync + 'static {
    /// The node ID type.
    type NodeId: NodeId;

    /// The public key type.
    type PublicKey: crate::core::PublicKey;

    /// Get the validator's node ID.
    fn id(&self) -> Self::NodeId;

    /// Get the validator's public key.
    fn public_key(&self) -> &Self::PublicKey;

    /// Get the validator's voting power (stake).
    fn voting_power(&self) -> u64;
}

/// Validator set for an epoch.
///
/// Manages the active validators for a given epoch.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Queryable by validator ID
///
/// # Example
///
/// ```text
/// use consensus_traits::network::{ValidatorInfo, ValidatorSet};
///
/// #[derive(Clone)]
/// struct MyValidatorSet {
///     validators: Vec<MyValidator>,
/// }
///
/// impl ValidatorSet for MyValidatorSet {
///     type Validator = MyValidator;
///
///     fn validators(&self) -> &[Self::Validator] {
///         &self.validators
///     }
///
///     fn get_validator(&self, id: &[u8; 32]) -> Option<&Self::Validator> {
///         self.validators.iter().find(|v| v.id == *id)
///     }
///
///     fn total_voting_power(&self) -> u64 {
///         self.validators.iter().map(|v| v.voting_power()).sum()
///     }
///
///     fn quorum_threshold(&self) -> u64 {
///         // 2/3 + 1 of total voting power
///         (self.total_voting_power() * 2 / 3) + 1
///     }
/// }
/// ```text
pub trait ValidatorSet: Clone + Send + Sync + 'static {
    /// The validator info type.
    type Validator: ValidatorInfo;

    /// Get all validators in the set.
    fn validators(&self) -> &[Self::Validator];

    /// Get a validator by ID.
    fn get_validator(&self, id: &<Self::Validator as ValidatorInfo>::NodeId) -> Option<&Self::Validator>;

    /// Get the total voting power of all validators.
    fn total_voting_power(&self) -> u64;

    /// Get the quorum threshold (minimum voting power for decisions).
    fn quorum_threshold(&self) -> u64;
}

/// Consensus network message.
///
/// Messages that flow through the consensus network.
///
/// # Type Parameters
///
/// * `B` - The block type
/// * `V` - The vote type
#[derive(Clone, Debug)]
pub enum ConsensusMessage<B, V>
where
    B: Block,
    V: Vote,
{
    /// Block proposal message.
    Proposal(Box<B>),

    /// Vote message.
    Vote(Box<V>),

    /// Request for block data.
    BlockRequest(BlockRequest),

    /// Response with block data.
    BlockResponse(Box<BlockResponse<B>>),

    /// New round timeout.
    NewRound {
        /// The round that timed out.
        round: u64,
    },
}

/// Request for block retrieval.
#[derive(Clone, Debug)]
pub struct BlockRequest {
    /// The ID of the requested block.
    pub block_id: Vec<u8>,
}

/// Response with block data.
#[derive(Clone, Debug)]
pub struct BlockResponse<B>
where
    B: Block,
{
    /// The requested block, if found.
    pub block: Option<B>,
}

/// Async network interface for consensus.
///
/// Handles sending and receiving consensus messages.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Asynchronous (using async/await)
///
/// # Example
///
/// ```text
/// use consensus_traits::network::{ConsensusMessage, ConsensusNetwork};
/// use consensus_traits::block::{Block, Vote};
/// use async_trait::async_trait;
///
/// struct MyNetwork;
///
/// #[async_trait]
/// impl<B, V> ConsensusNetwork<B, V> for MyNetwork
/// where
///     B: Block + Send + Sync,
///     V: Vote + Send + Sync,
/// {
///     type NodeId = [u8; 32];
///     type Error = anyhow::Error;
///
///     async fn send_proposal(
///         &self,
///         to: Self::NodeId,
///         proposal: B,
///     ) -> Result<(), Self::Error> {
///         // Send proposal to peer
///         Ok(())
///     }
///
///     async fn broadcast_proposal(&self, proposal: B) -> Result<(), Self::Error> {
///         // Broadcast to all validators
///         Ok(())
///     }
///
///     async fn receive_message(&self) -> Result<ConsensusMessage<B, V>, Self::Error> {
///         // Receive incoming message
///         unimplemented!()
///     }
/// }
/// ```text
#[async_trait]
pub trait ConsensusNetwork<B, V>: Send + Sync
where
    B: Block,
    V: Vote,
{
    /// The node ID type.
    type NodeId: NodeId;

    /// Error type for network operations.
    type Error: std::error::Error + Send + Sync;

    /// Send a proposal to a specific peer.
    async fn send_proposal(
        &self,
        to: Self::NodeId,
        proposal: B,
    ) -> Result<(), Self::Error>;

    /// Broadcast a proposal to all validators.
    async fn broadcast_proposal(&self, proposal: B) -> Result<(), Self::Error>;

    /// Send a vote to a specific peer.
    async fn send_vote(
        &self,
        to: Self::NodeId,
        vote: V,
    ) -> Result<(), Self::Error>;

    /// Broadcast a vote to all validators.
    async fn broadcast_vote(&self, vote: V) -> Result<(), Self::Error>;

    /// Receive a message from the network.
    ///
    /// This should block until a message is available.
    async fn receive_message(&self) -> Result<ConsensusMessage<B, V>, Self::Error>;

    /// Get the list of current validator peers.
    fn validator_peers(&self) -> Vec<Self::NodeId>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{NodeId, PublicKey};

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestNodeId([u8; 32]);

    impl crate::core::Hash for TestNodeId {
        fn zero() -> Self {
            TestNodeId([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, crate::core::Error> {
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
        fn from_bytes(bytes: &[u8]) -> Result<Self, crate::core::Error> {
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

    #[derive(Clone, Debug)]
    struct TestPublicKey(Vec<u8>);

    impl PublicKey for TestPublicKey {
        fn to_bytes(&self) -> Vec<u8> {
            self.0.clone()
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, crate::core::Error> {
            Ok(TestPublicKey(bytes.to_vec()))
        }
    }

    #[derive(Clone)]
    struct TestValidator {
        id: TestNodeId,
        public_key: TestPublicKey,
        stake: u64,
    }

    impl ValidatorInfo for TestValidator {
        type NodeId = TestNodeId;
        type PublicKey = TestPublicKey;

        fn id(&self) -> Self::NodeId {
            self.id
        }

        fn public_key(&self) -> &Self::PublicKey {
            &self.public_key
        }

        fn voting_power(&self) -> u64 {
            self.stake
        }
    }

    #[derive(Clone)]
    struct TestValidatorSet {
        validators: Vec<TestValidator>,
    }

    impl ValidatorSet for TestValidatorSet {
        type Validator = TestValidator;

        fn validators(&self) -> &[Self::Validator] {
            &self.validators
        }

        fn get_validator(&self, id: &TestNodeId) -> Option<&Self::Validator> {
            self.validators.iter().find(|v| v.id == *id)
        }

        fn total_voting_power(&self) -> u64 {
            self.validators.iter().map(|v| v.stake).sum()
        }

        fn quorum_threshold(&self) -> u64 {
            (self.total_voting_power() * 2 / 3) + 1
        }
    }

    #[test]
    fn test_validator_set() {
        let validators = vec![
            TestValidator {
                id: TestNodeId([1u8; 32]),
                public_key: TestPublicKey(vec![1, 2, 3]),
                stake: 100,
            },
            TestValidator {
                id: TestNodeId([2u8; 32]),
                public_key: TestPublicKey(vec![4, 5, 6]),
                stake: 200,
            },
        ];
        let set = TestValidatorSet { validators };

        assert_eq!(set.total_voting_power(), 300);
        assert_eq!(set.quorum_threshold(), 201); // 2/3 + 1
    }
}
