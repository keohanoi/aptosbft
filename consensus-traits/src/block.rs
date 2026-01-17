// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block and transaction traits for consensus.
//!
//! This module defines traits for blocks, transactions, votes, and quorum certificates.
//! These are the core data structures that flow through the consensus protocol.

use crate::core::{Error, Hash, NodeId, Signature};

/// Generic transaction type.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Hashable for deduplication
/// - Size-bounded for block inclusion
///
/// # Example
///
/// ```text
/// use consensus_traits::core::Hash;
/// use consensus_traits::block::Transaction;
///
/// #[derive(Clone)]
/// struct MyTransaction {
///     data: Vec<u8>,
/// }
///
/// impl Transaction for MyTransaction {
///     type Hash = [u8; 32];
///
///     fn hash(&self) -> Self::Hash {
///         // Compute transaction hash
///         [0u8; 32]
///     }
///
///     fn size(&self) -> usize {
///         self.data.len()
///     }
/// }
/// ```text
pub trait Transaction: Clone + Send + Sync + 'static {
    /// The hash type used for this transaction.
    type Hash: Hash;

    /// Compute the hash of this transaction.
    ///
    /// This must be deterministic and collision-resistant.
    fn hash(&self) -> Self::Hash;

    /// Get the serialized size of this transaction in bytes.
    ///
    /// This is used for block size calculations and gas metering.
    fn size(&self) -> usize;
}

/// Block metadata independent of transaction type.
///
/// Contains protocol-level information about a block such as epoch, round,
/// author, and parent block reference.
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
/// use consensus_traits::core::{Hash, NodeId};
/// use consensus_traits::block::BlockMetadata;
///
/// #[derive(Clone)]
/// struct MyBlockMetadata {
///     epoch: u64,
///     round: u64,
///     author: [u8; 32],
///     parent_id: [u8; 32],
///     timestamp: u64,
/// }
///
/// impl BlockMetadata for MyBlockMetadata {
///     type NodeId = [u8; 32];
///     type Hash = [u8; 32];
///
///     fn epoch(&self) -> u64 {
///         self.epoch
///     }
///
///     fn round(&self) -> u64 {
///         self.round
///     }
///
///     fn author(&self) -> Self::NodeId {
///         self.author
///     }
///
///     fn parent_id(&self) -> Self::Hash {
///         self.parent_id
///     }
///
///     fn timestamp(&self) -> u64 {
///         self.timestamp
///     }
/// }
/// ```text
/// Quorum certificate for block commit.
///
/// A quorum certificate (QC) proves that a sufficient set of validators
/// have voted for a block, allowing it to be committed.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Verifiable (signature validation)
/// - Serializable
pub trait QuorumCertificate: Clone + Send + Sync + 'static {
    /// The block metadata type.
    type BlockMetadata: BlockMetadata;

    /// The hash type.
    type Hash: Hash;

    /// Get the certified block metadata.
    fn certified_block(&self) -> &Self::BlockMetadata;

    /// Get the hash of the certified block.
    fn block_id(&self) -> Self::Hash;

    /// Get the epoch of this quorum certificate.
    fn epoch(&self) -> u64 {
        self.certified_block().epoch()
    }

    /// Get the round of this quorum certificate.
    fn round(&self) -> u64 {
        self.certified_block().round()
    }

    /// Verify this quorum certificate.
    ///
    /// # Errors
    ///
    /// Returns an error if the QC is invalid.
    fn verify(&self) -> Result<(), Error>;

    /// Create a quorum certificate from a collection of votes and aggregated signature.
    ///
    /// This is used to form a QC after collecting enough votes to reach quorum.
    ///
    /// # Parameters
    ///
    /// * `votes` - Slice of votes that form the quorum
    /// * `aggregated_signature` - The aggregated signature from all votes
    ///
    /// # Errors
    ///
    /// Returns an error if the votes cannot form a valid QC.
    fn from_votes<B, V>(
        votes: &[std::sync::Arc<V>],
        aggregated_signature: <V::Signature as crate::core::Signature>::Aggregated,
    ) -> Result<Self, Error>
    where
        B: Block,
        V: Vote<Block = B>;
}

pub trait BlockMetadata: Clone + Send + Sync + 'static {
    /// The quorum certificate type for this blockchain.
    type QuorumCert: QuorumCertificate;

    /// The node ID type used in this blockchain.
    type NodeId: NodeId;

    /// The hash type used in this blockchain.
    type Hash: Hash;

    /// Get the epoch number for this block.
    fn epoch(&self) -> u64;

    /// Get the round number within the epoch.
    fn round(&self) -> u64;

    /// Get the author (proposer) of this block.
    fn author(&self) -> Self::NodeId;

    /// Get the hash of the parent block.
    fn parent_id(&self) -> Self::Hash;

    /// Get the timestamp when this block was proposed.
    fn timestamp(&self) -> u64;
}

/// Generic block trait for consensus.
///
/// Blocks contain transactions and metadata required for the consensus protocol.
/// Every blockchain implementing AptosBFT must provide a block type.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Verifiable (signature validation)
/// - Identifiable (unique hash)
///
/// # Example
///
/// ```text
/// use consensus_traits::block::{Block, BlockMetadata, Transaction};
/// use consensus_traits::core::{Error, Hash, Signature};
///
/// #[derive(Clone)]
/// struct MyBlock {
///     id: [u8; 32],
///     metadata: MyBlockMetadata,
///     transactions: Vec<MyTransaction>,
///     signature: Option<MySignature>,
/// }
///
/// impl Block for MyBlock {
///     type Transaction = MyTransaction;
///     type Metadata = MyBlockMetadata;
///     type Signature = MySignature;
///     type Hash = [u8; 32];
///
///     fn id(&self) -> Self::Hash {
///         self.id
///     }
///
///     fn metadata(&self) -> &Self::Metadata {
///         &self.metadata
///     }
///
///     fn transactions(&self) -> &[Self::Transaction] {
///         &self.transactions
///     }
///
///     fn signature(&self) -> Option<&Self::Signature> {
///         self.signature.as_ref()
///     }
///
///     fn parent_id(&self) -> Self::Hash {
///         self.metadata.parent_id()
///     }
///
///     fn is_genesis(&self) -> bool {
///         self.metadata.epoch() == 0 && self.metadata.round() == 0
///     }
///
///     fn is_nil(&self) -> bool {
///         self.transactions.is_empty()
///     }
///
///     fn verify_signature(&self) -> Result<(), Error> {
///         if let Some(sig) = &self.signature {
///             // Verify signature logic
///             Ok(())
///         } else {
///             Err(anyhow::anyhow!("Missing signature"))
///         }
///     }
///
///     fn new_genesis() -> Self {
///         // Create genesis block
///         unimplemented!()
///     }
/// }
/// ```text
pub trait Block: Clone + Send + Sync + 'static {
    /// The transaction type contained in blocks.
    type Transaction: Transaction;

    /// The metadata type for this block.
    type Metadata: BlockMetadata;

    /// The signature type for this block.
    type Signature: Signature;

    /// The hash type used for block identification.
    type Hash: Hash;

    /// Get the unique identifier (hash) of this block.
    fn id(&self) -> Self::Hash;

    /// Get the block metadata.
    fn metadata(&self) -> &Self::Metadata;

    /// Get the list of transactions in this block.
    fn transactions(&self) -> &[Self::Transaction];

    /// Get the signature on this block, if present.
    ///
    /// Genesis blocks may not have signatures.
    fn signature(&self) -> Option<&Self::Signature>;

    /// Get the hash of the parent block.
    fn parent_id(&self) -> Self::Hash;

    /// Check if this is the genesis block.
    fn is_genesis(&self) -> bool;

    /// Check if this is a nil (empty) block.
    ///
    /// Nil blocks are used for rounds where no transactions are available.
    fn is_nil(&self) -> bool;

    /// Verify the signature on this block.
    ///
    /// # Errors
    ///
    /// Returns an error if the signature is invalid or missing.
    fn verify_signature(&self) -> Result<(), Error>;

    /// Create a new genesis block.
    fn new_genesis() -> Self;
}

/// Vote data containing block information for voting.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Verifiable (consistent with consensus rules)
///
/// # Example
///
/// ```text
/// use consensus_traits::block::{VoteData, BlockMetadata};
/// use consensus_traits::core::Error;
///
/// #[derive(Clone)]
/// struct MyVoteData {
///     proposed_block: MyBlockMetadata,
///     parent_block: MyBlockMetadata,
/// }
///
/// impl VoteData for MyVoteData {
///     type BlockMetadata = MyBlockMetadata;
///
///     fn proposed_block(&self) -> &Self::BlockMetadata {
///         &self.proposed_block
///     }
///
///     fn parent_block(&self) -> &Self::BlockMetadata {
///         &self.parent_block
///     }
///
///     fn verify(&self) -> Result<(), Error> {
///         // Verify vote data is consistent
///         Ok(())
///     }
/// }
/// ```text
pub trait VoteData: Clone + Send + Sync + 'static {
    /// The block metadata type.
    type BlockMetadata: BlockMetadata;

    /// Get the block being voted on.
    fn proposed_block(&self) -> &Self::BlockMetadata;

    /// Get the parent of the block being voted on.
    fn parent_block(&self) -> &Self::BlockMetadata;

    /// Verify that this vote data is consistent.
    ///
    /// # Errors
    ///
    /// Returns an error if the vote data violates consensus rules.
    fn verify(&self) -> Result<(), Error>;
}

/// Generic vote trait.
///
/// Votes are sent by validators to indicate their support for blocks.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Verifiable (signature and data validation)
///
/// # Example
///
/// ```text
/// use consensus_traits::block::{Vote, VoteData, LedgerInfo};
/// use consensus_traits::core::{Error, NodeId, Signature};
///
/// #[derive(Clone)]
/// struct MyVote {
///     vote_data: MyVoteData,
///     author: [u8; 32],
///     ledger_info: MyLedgerInfo,
///     signature: MySignature,
/// }
///
/// impl Vote for MyVote {
///     type VoteData = MyVoteData;
///     type NodeId = [u8; 32];
///     type LedgerInfo = MyLedgerInfo;
///     type Signature = MySignature;
///
///     fn vote_data(&self) -> &Self::VoteData {
///         &self.vote_data
///     }
///
///     fn author(&self) -> Self::NodeId {
///         self.author
///     }
///
///     fn ledger_info(&self) -> &Self::LedgerInfo {
///         &self.ledger_info
///     }
///
///     fn signature(&self) -> &Self::Signature {
///         &self.signature
///     }
///
///     fn verify(&self, verifier: &dyn ValidatorVerifier) -> Result<(), Error> {
///         // Verify vote signature and data
///         Ok(())
///     }
/// }
/// ```text
pub trait Vote: Clone + Send + Sync + 'static {
    /// The block type this vote is for.
    type Block: Block;

    /// The hash type used in block identification.
    type Hash: Hash;

    /// The vote data type.
    type VoteData: VoteData;

    /// The node ID type.
    type NodeId: NodeId;

    /// The ledger info type.
    type LedgerInfo: LedgerInfo;

    /// The signature type.
    type Signature: Signature;

    /// Get the vote data.
    fn vote_data(&self) -> &Self::VoteData;

    /// Get the author (voter) of this vote.
    fn author(&self) -> Self::NodeId;

    /// Get the ledger info committed to by this vote.
    fn ledger_info(&self) -> &Self::LedgerInfo;

    /// Get the signature on this vote.
    fn signature(&self) -> &Self::Signature;

    /// Get the ID of the block this vote is for.
    fn block_id(&self) -> Self::Hash;

    /// Get the round this vote is for.
    fn round(&self) -> u64;

    /// Verify this vote.
    ///
    /// # Errors
    ///
    /// Returns an error if the vote is invalid.
    fn verify<B, VV>(&self, verifier: &VV) -> Result<(), Error>
    where
        B: Block,
        VV: ValidatorVerifier<B, Self>;
}

/// Ledger information for committed blocks.
///
/// Contains the commit certificate and accumulated state root.
///
/// # Requirements
///
/// Implementations must be:
/// - Thread-safe (Send + Sync)
/// - Serializable for network transmission
pub trait LedgerInfo: Clone + Send + Sync + 'static {
    /// The hash type for state roots.
    type Hash: Hash;

    /// The commit info type.
    type CommitInfo: CommitInfo<Hash = Self::Hash>;

    /// Get the commit information.
    fn commit_info(&self) -> &Self::CommitInfo;

    /// Get the epoch number.
    fn epoch(&self) -> u64;

    /// Get the round number.
    fn round(&self) -> u64;

    /// Get the accumulated state hash (Merkle root).
    fn accumulated_state(&self) -> &Self::Hash;
}

/// Commit information for a block.
///
/// Contains the essential data needed to verify a committed block.
pub trait CommitInfo: Clone + Send + Sync + 'static {
    /// The hash type for block IDs.
    type Hash: Hash;

    /// Get the committed block ID.
    fn block_id(&self) -> Self::Hash;

    /// Get the round number.
    fn round(&self) -> u64;

    /// Get the epoch number.
    fn epoch(&self) -> u64;

    /// Get the version (transaction count) of the committed state.
    fn version(&self) -> u64;

    /// Get the timestamp of the committed block.
    fn timestamp(&self) -> u64;
}

/// Validator verifier for checking signatures.
///
/// This trait is generic over the Block and Vote types to allow for
/// type-safe verification without dynamic dispatch.
pub trait ValidatorVerifier<B, V>: Send + Sync
where
    B: Block,
    V: Vote,
{
    /// Verify a vote.
    ///
    /// # Errors
    ///
    /// Returns an error if the vote signature is invalid.
    fn verify_vote(&self, vote: &V) -> Result<(), Error>;

    /// Verify a block.
    ///
    /// # Errors
    ///
    /// Returns an error if the block signature is invalid.
    fn verify_block(&self, block: &B) -> Result<(), Error>;

    /// Get the voting power for a specific validator.
    ///
    /// Returns `None` if the validator is not known.
    fn get_voting_power(&self, validator_id: &V::NodeId) -> Option<u64>;

    /// Get the total voting power of all validators.
    fn total_voting_power(&self) -> u128;

    /// Get the minimum voting power required for quorum.
    ///
    /// This is typically 2/3 + 1 of the total voting power to ensure
    /// Byzantine fault tolerance.
    fn quorum_voting_power(&self) -> u128;

    /// Check if the given set of signers meets the quorum threshold.
    ///
    /// # Parameters
    ///
    /// * `signers` - Iterator of validator identifiers who have signed
    /// * `check_quorum` - If true, verify quorum threshold is met
    ///
    /// # Errors
    ///
    /// Returns `Err(VerifyError::TooLittleVotingPower)` if insufficient voting power.
    fn check_voting_power<'a>(
        &self,
        signers: impl Iterator<Item = &'a V::NodeId>,
        check_quorum: bool,
    ) -> Result<(), crate::core::VerifyError>;

    /// Aggregate signatures from multiple votes into a single signature.
    ///
    /// This is used for creating quorum certificates from individual votes.
    /// BLS signatures support efficient aggregation.
    ///
    /// # Errors
    ///
    /// Returns an error if the signatures cannot be aggregated.
    fn aggregate_signatures<'a>(
        &self,
        signatures: impl Iterator<Item = &'a V::Signature>,
    ) -> Result<<V::Signature as crate::core::Signature>::Aggregated, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hash;

    #[derive(Clone)]
    struct TestTransaction {
        data: Vec<u8>,
    }

    #[derive(Clone, Copy, PartialEq, Eq, std::hash::Hash, Debug)]
    struct TestHash([u8; 32]);

    impl Hash for TestHash {
        fn zero() -> Self {
            TestHash([0u8; 32])
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, crate::core::Error> {
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

    impl Transaction for TestTransaction {
        type Hash = TestHash;

        fn hash(&self) -> Self::Hash {
            TestHash([0u8; 32])
        }

        fn size(&self) -> usize {
            self.data.len()
        }
    }

    #[test]
    fn test_transaction() {
        let tx = TestTransaction { data: vec![1, 2, 3] };
        assert_eq!(tx.size(), 3);
    }
}
