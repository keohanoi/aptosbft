// Generated gRPC code for the ConsensusApp bridge service
//
// This module contains the Rust types and gRPC service definitions
// generated from consensus_app.proto. It provides the communication
// protocol between AptosBFT consensus (Rust) and dYdX application (Go).

pub mod consensus_app {
    tonic::include_proto!("aptosbft.consensus.v1");
}

// Re-export commonly used types for convenience
pub use consensus_app::{
    consensus_app_client::ConsensusAppClient,
    consensus_app_server::{ConsensusApp, ConsensusAppServer},
};

// Re-export message types
pub use consensus_app::{
    BuildBlockRequest,
    BuildBlockResponse,
    VerifyBlockRequest,
    VerifyBlockResponse,
    ExecuteBlockRequest,
    ExecuteBlockResponse,
    VoteExtensionRequest,
    VoteExtensionResponse,
    CommitBlockRequest,
    CommitBlockResponse,
    MempoolRequest,
    MempoolResponse,
    QueryRequest,
    QueryResponse,
    // Nested types
    StateUpdateHint,
    TxResult,
    Event,
    EventAttribute,
    ValidatorUpdate,
    // Mempool nested types
    FetchBatchRequest,
    OrderReadyNotification,
    BatchAck,
    TxBatch,
    BatchRequest,
    AckResponse,
};
