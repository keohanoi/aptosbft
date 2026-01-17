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

    /// Invalid input parameter
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Feature not yet implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

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

    /// Create a new invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a new not implemented error
    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self::NotImplemented(msg.into())
    }

    /// Create a new message error
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Grpc error tests
    #[test]
    fn test_error_grpc_display() {
        let err = DydxAdapterError::grpc("connection refused");
        assert_eq!(format!("{}", err), "gRPC error: connection refused");
        assert!(format!("{:?}", err).contains("Grpc"));
    }

    #[test]
    fn test_error_grpc_empty_message() {
        let err = DydxAdapterError::grpc("");
        assert_eq!(format!("{}", err), "gRPC error: ");
    }

    #[test]
    fn test_error_grpc_long_message() {
        let long_msg = "a".repeat(1000);
        let err = DydxAdapterError::grpc(&long_msg);
        assert!(format!("{}", err).contains(&long_msg[..100]));
        assert!(format!("{}", err).starts_with("gRPC error:"));
    }

    #[test]
    fn test_error_grpc_special_chars() {
        let err = DydxAdapterError::grpc("error: \n\t\r\x00");
        assert_eq!(format!("{}", err), "gRPC error: error: \n\t\r\x00");
    }

    // ExecutionFailed error tests
    #[test]
    fn test_error_execution_failed_display() {
        let err = DydxAdapterError::execution_failed(100, "state root mismatch");
        assert_eq!(format!("{}", err), "Block execution failed at round 100: state root mismatch");
    }

    #[test]
    fn test_error_execution_failed_round_zero() {
        let err = DydxAdapterError::execution_failed(0, "genesis block");
        assert_eq!(format!("{}", err), "Block execution failed at round 0: genesis block");
    }

    #[test]
    fn test_error_execution_failed_max_round() {
        let err = DydxAdapterError::execution_failed(u64::MAX, "max round");
        assert!(format!("{}", err).contains("Block execution failed at round"));
        assert!(format!("{}", err).contains(&format!("{}", u64::MAX)));
    }

    #[test]
    fn test_error_execution_failed_empty_message() {
        let err = DydxAdapterError::execution_failed(50, "");
        assert_eq!(format!("{}", err), "Block execution failed at round 50: ");
    }

    // InvalidStateRoot error tests
    #[test]
    fn test_error_invalid_state_root_display() {
        let err = DydxAdapterError::InvalidStateRoot("hash mismatch".to_string());
        assert_eq!(format!("{}", err), "Invalid state root: hash mismatch");
    }

    #[test]
    fn test_error_invalid_state_root_empty_hash() {
        let err = DydxAdapterError::InvalidStateRoot("".to_string());
        assert_eq!(format!("{}", err), "Invalid state root: ");
    }

    // Connection error tests
    #[test]
    fn test_error_connection_display() {
        let err = DydxAdapterError::Connection("timeout".to_string());
        assert_eq!(format!("{}", err), "Connection failed: timeout");
    }

    #[test]
    fn test_error_connection_empty_message() {
        let err = DydxAdapterError::Connection("".to_string());
        assert_eq!(format!("{}", err), "Connection failed: ");
    }

    // InvalidInput error tests
    #[test]
    fn test_error_invalid_input_display() {
        let err = DydxAdapterError::invalid_input("negative amount");
        assert_eq!(format!("{}", err), "Invalid input: negative amount");
    }

    #[test]
    fn test_error_invalid_input_empty_message() {
        let err = DydxAdapterError::invalid_input("");
        assert_eq!(format!("{}", err), "Invalid input: ");
    }

    // NotImplemented error tests
    #[test]
    fn test_error_not_implemented_display() {
        let err = DydxAdapterError::not_implemented("vote extensions");
        assert_eq!(format!("{}", err), "Not implemented: vote extensions");
    }

    #[test]
    fn test_error_not_implemented_empty_message() {
        let err = DydxAdapterError::not_implemented("");
        assert_eq!(format!("{}", err), "Not implemented: ");
    }

    // Message error tests
    #[test]
    fn test_error_message_display() {
        let err = DydxAdapterError::msg("generic error");
        assert_eq!(format!("{}", err), "generic error");
    }

    #[test]
    fn test_error_message_empty() {
        let err = DydxAdapterError::msg("");
        assert_eq!(format!("{}", err), "");
    }

    #[test]
    fn test_error_message_with_prefix() {
        let err = DydxAdapterError::msg("failed to connect");
        assert_eq!(format!("{}", err), "failed to connect");
    }

    // Error construction tests
    #[test]
    fn test_error_construction_methods() {
        let grpc_err = DydxAdapterError::grpc("test");
        assert!(matches!(grpc_err, DydxAdapterError::Grpc(_)));

        let exec_err = DydxAdapterError::execution_failed(5, "test");
        assert!(matches!(exec_err, DydxAdapterError::ExecutionFailed { round: 5, .. }));

        let invalid_err = DydxAdapterError::invalid_input("test");
        assert!(matches!(invalid_err, DydxAdapterError::InvalidInput(_)));

        let not_impl_err = DydxAdapterError::not_implemented("test");
        assert!(matches!(not_impl_err, DydxAdapterError::NotImplemented(_)));

        let msg_err = DydxAdapterError::msg("test");
        assert!(matches!(msg_err, DydxAdapterError::Message(_)));
    }

    // Standard trait tests
    #[test]
    fn test_error_std_trait() {
        // Verify that our error implements std::error::Error
        fn assert_std_error<E: std::error::Error + Send + Sync>() {}
        assert_std_error::<DydxAdapterError>();
    }

    #[test]
    fn test_error_send_sync() {
        // Verify Send + Sync bounds for async usage
        fn is_send<T: Send>() {}
        fn is_sync<T: Sync>() {}

        is_send::<DydxAdapterError>();
        is_sync::<DydxAdapterError>();
    }

    #[test]
    fn test_error_debug_format() {
        let err = DydxAdapterError::grpc("test");
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Grpc") || debug_str.contains("test"));
    }

    // Error variant coverage tests
    #[test]
    fn test_all_error_variants_constructible() {
        // Verify all error variants can be constructed
        let _ = DydxAdapterError::grpc("test");
        let _ = DydxAdapterError::ExecutionFailed {
            round: 1,
            message: "test".to_string(),
        };
        let _ = DydxAdapterError::InvalidStateRoot("test".to_string());
        let _ = DydxAdapterError::Connection("test".to_string());
        let _ = DydxAdapterError::InvalidInput("test".to_string());
        let _ = DydxAdapterError::NotImplemented("test".to_string());
        let _ = DydxAdapterError::Message("test".to_string());
    }

    #[test]
    fn test_error_size() {
        // Check that error variants have reasonable size
        let size = std::mem::size_of::<DydxAdapterError>();
        // Error type uses String which has heap allocation
        // Size may vary but should be reasonable (under 100 bytes)
        assert!(size < 100, "Error size should be reasonable, got {}", size);
        assert!(size > 0, "Error size should be non-zero");
    }

    #[test]
    fn test_error_clone_not_required() {
        // Note: Error types typically don't implement Clone
        // If Clone is needed, it can be added via derive(Clone)
        let err = DydxAdapterError::msg("test");
        let _ = err; // Use the error
    }
}
