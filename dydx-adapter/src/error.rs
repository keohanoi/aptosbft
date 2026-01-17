// Error types for dYdX adapter

use consensus_traits::core::VerifyError;
use thiserror::Error;

/// Main error type for dYdX adapter operations
///
/// This error type implements std::error::Error to satisfy
/// the StateComputer trait bound requirements.
#[derive(Debug, Error)]
pub enum DydxAdapterError {
    /// gRPC communication error
    #[error("gRPC error: {0}")]
    Grpc(String),

    /// Block execution failed
    #[error("Block execution failed at round {round}: {message}")]
    ExecutionFailed { round: u64, message: String },

    /// State root validation failed
    #[error("Invalid state root: {0}")]
    InvalidStateRoot(String),

    /// Connection error
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Verification error from consensus traits
    #[error("Verification error: {0}")]
    Verification(#[from] VerifyError),

    /// Generic error message
    #[error("{0}")]
    Message(String),
}

impl DydxAdapterError {
    /// Create a new gRPC error
    pub fn grpc(msg: impl Into<String>) -> Self {
        Self::Grpc(msg.into())
    }

    /// Create a new execution failed error
    pub fn execution_failed(round: u64, msg: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            round,
            message: msg.into(),
        }
    }

    /// Create a new message error
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DydxAdapterError::grpc("connection refused");
        assert_eq!(format!("{}", err), "gRPC error: connection refused");

        let err = DydxAdapterError::execution_failed(100, "state root mismatch");
        assert_eq!(format!("{}", err), "Block execution failed at round 100: state root mismatch");

        let err = DydxAdapterError::msg("generic error");
        assert_eq!(format!("{}", err), "generic error");
    }

    #[test]
    fn test_error_std_trait() {
        // Verify that our error implements std::error::Error
        fn assert_std_error<E: std::error::Error + Send + Sync>() {}
        assert_std_error::<DydxAdapterError>();
    }
}
