// dYdX Adapter for AptosBFT
//
// This crate implements the bridge between AptosBFT consensus and the dYdX v4
// application, including:
// - Type definitions for consensus traits
// - StateComputer implementation for executing blocks
// - Vote extension generation and handling
// - Synthetic Tendermint header generation for IBC

// Note: Some modules are still under development.
// Uncomment them as they are completed.

pub mod error;
pub mod types;
pub mod state_computer;
pub mod mempool_stream;
pub mod ibc_headers;
pub mod dydx_service;
pub mod vote_extensions;

pub use error::DydxAdapterError;
pub use mempool_stream::{MempoolSyncStream, MempoolSyncConfig, create_mempool_sync_stream};
pub use ibc_headers::{
    SyntheticHeaderBuilder,
    create_header_builder,
    TendermintHeader,
    SignedHeader,
    HeaderBuilderConfig,
    Validator,
    ValidatorSet,
    Commit,
    Version,
    Timestamp,
    TmHash,
    Address,
    ConsensusAddress,
    Bytes,
};
pub use types::{
    DydxBlock,
    DydxVote,
    DydxTransaction,
    DydxHash,
    DydxNodeId,
    DydxSignature,
    DydxLedgerInfo,
    DydxBlockMetadata,
};
pub use state_computer::DydxStateComputer;
pub use vote_extensions::{
    DydxVoteExtension,
    DydxVoteExtensionGenerator,
    VoteExtensionGenerator,
};
