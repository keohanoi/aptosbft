// Error types for the ConsensusApp gRPC server
//
// This module defines the error types that can occur when handling
// gRPC requests from the AptosBFT consensus layer.

use tonic::{Code, Status};

/// Result type for server operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in the ConsensusApp service
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Block execution failed
    #[error("Block execution failed at height {height}: {message}")]
    ExecutionFailed { height: u64, message: String },

    /// Block verification failed
    #[error("Block verification failed: {0}")]
    VerificationFailed(String),

    /// State corruption detected
    #[error("State corruption: {0}")]
    StateCorruption(String),

    /// Internal application error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Vote extension generation failed
    #[error("Vote extension generation failed: {0}")]
    VoteExtensionFailed(String),

    /// Mempool sync error
    #[error("Mempool sync error: {0}")]
    MempoolError(String),

    /// Network error communicating with application
    #[error("Network error: {0}")]
    Network(String),

    /// Timeout waiting for application response
    #[error("Timeout: {0}")]
    Timeout(String),
}

impl Error {
    /// Convert this error into a gRPC Status with appropriate error code
    pub fn to_status(&self) -> Status {
        match self {
            Error::InvalidRequest(msg) => {
                Status::new(Code::InvalidArgument, msg.clone())
            }
            Error::ExecutionFailed { height, message } => {
                Status::new(
                    Code::Internal,
                    format!("Block execution failed at height {}: {}", height, message),
                )
            }
            Error::VerificationFailed(msg) => {
                Status::new(Code::InvalidArgument, msg.clone())
            }
            Error::StateCorruption(msg) => {
                Status::new(Code::DataLoss, msg.clone())
            }
            Error::Internal(msg) => {
                Status::new(Code::Internal, msg.clone())
            }
            Error::VoteExtensionFailed(msg) => {
                Status::new(Code::Internal, msg.clone())
            }
            Error::MempoolError(msg) => {
                Status::new(Code::Unavailable, msg.clone())
            }
            Error::Network(msg) => {
                Status::new(Code::Unavailable, msg.clone())
            }
            Error::Timeout(msg) => {
                Status::new(Code::DeadlineExceeded, msg.clone())
            }
        }
    }
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        err.to_status()
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(err: tonic::transport::Error) -> Self {
        Error::Network(err.to_string())
    }
}

impl From<tonic::Status> for Error {
    fn from(status: Status) -> Self {
        Error::Network(status.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_to_status_invalid_request() {
        let error = Error::InvalidRequest("bad parameter".to_string());
        let status = error.to_status();
        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[test]
    fn test_error_to_status_execution_failed() {
        let error = Error::ExecutionFailed {
            height: 100,
            message: "transaction failed".to_string(),
        };
        let status = error.to_status();
        assert_eq!(status.code(), Code::Internal);
        assert!(status.message().contains("100"));
    }

    #[test]
    fn test_error_to_status_verification_failed() {
        let error = Error::VerificationFailed("invalid signature".to_string());
        let status = error.to_status();
        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[test]
    fn test_error_to_status_state_corruption() {
        let error = Error::StateCorruption("merkle root mismatch".to_string());
        let status = error.to_status();
        assert_eq!(status.code(), Code::DataLoss);
    }

    #[test]
    fn test_error_to_status_timeout() {
        let error = Error::Timeout("operation took too long".to_string());
        let status = error.to_status();
        assert_eq!(status.code(), Code::DeadlineExceeded);
    }
}
