// gRPC Server implementation for AptosBFT consensus bridge
//
// This crate implements the gRPC server that exposes the ConsensusApp service,
// allowing AptosBFT consensus to communicate with the dYdX application.

pub mod server;
pub mod service;
pub mod error;

pub use server::ConsensusServer;
pub use service::{ConsensusAppService, DefaultConsensusAppService};
pub use error::{Error, Result};
