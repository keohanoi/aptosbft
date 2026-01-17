// Vote Extensions for dYdX AptosBFT Integration
//
// This module implements vote extensions for the dYdX v4 AptosBFT integration.
// Vote extensions allow validators to include additional data (such as Slinky
// oracle price data) in their votes, which is then aggregated and passed to
// the application during block execution.
//
// In dYdX, vote extensions are critical for the Slinky price oracle integration,
// as they allow validators to collectively agree on the price data used for each
// block.

use consensus_traits::core::Error;
use serde::{Deserialize, Serialize};

/// dYdX vote extension containing Slinky oracle price data
///
/// This is the data that validators include in their vote extensions during
/// the Precommit phase. The extensions are aggregated in the Quorum Certificate
/// and passed to the application during ExecuteBlock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DydxVoteExtension {
    /// Price data from Slinky oracle (protobuf encoded)
    pub price_data: Vec<u8>,

    /// Timestamp of the price data (unix milliseconds)
    pub timestamp_ms: u64,

    /// Extension version (for future compatibility)
    pub version: u8,
}

impl DydxVoteExtension {
    /// Create a new vote extension
    pub fn new(price_data: Vec<u8>, timestamp_ms: u64) -> Self {
        Self {
            price_data,
            timestamp_ms,
            version: 1,
        }
    }

    /// Create an empty vote extension (for testing/fallback)
    pub fn empty() -> Self {
        Self {
            price_data: vec![],
            timestamp_ms: 0,
            version: 1,
        }
    }

    /// Serialize the extension for signing/inclusion in vote
    pub fn to_bytes(&self) -> Vec<u8> {
        // In production, use proper protobuf serialization
        // For now, serialize via serde
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Parse extension from bytes (during vote processing)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::msg(format!("Failed to parse vote extension: {}", e)))
    }

    /// Verify the extension format and basic validity
    pub fn verify(&self) -> Result<(), Error> {
        // Verify version is supported
        if self.version > 1 {
            return Err(Error::msg(format!("Unsupported extension version: {}", self.version)));
        }

        // Verify timestamp is not too far in the future (allow 1 second clock skew)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::msg(format!("System time error: {}", e)))?
            .as_millis() as u64;

        if self.timestamp_ms > now_ms + 1000 {
            return Err(Error::msg("Extension timestamp is too far in the future"));
        }

        // In production, verify:
        // - Price data format is valid protobuf
        // - Prices are within reasonable bounds
        // - Required markets are present
        // - Signatures are valid (if using signed price data)

        Ok(())
    }
}

/// Vote Extension Generator for dYdX
///
/// This trait is implemented by validators to generate vote extensions
/// containing Slinky oracle price data.
pub trait VoteExtensionGenerator: Send + Sync {
    /// Generate a vote extension for the current round
    ///
    /// This method should:
    /// 1. Fetch the latest price data from Slinky
    /// 2. Encode the price data in the vote extension format
    /// 3. Return the extension for inclusion in the vote
    fn generate_vote_extension(&self) -> Result<DydxVoteExtension, Error>;

    /// Verify a vote extension received from another validator
    ///
    /// This validates the extension format and checks that:
    /// - The data is properly structured
    /// - The timestamp is reasonable
    /// - The price data is within acceptable bounds
    fn verify_vote_extension(&self, extension: &DydxVoteExtension) -> Result<(), Error>;

    /// Aggregate multiple vote extensions into one
    ///
    /// During QC formation, vote extensions from all validators are
    /// aggregated. For price data, this typically means:
    /// - Taking the median price for each market
    /// - Using the most recent timestamp
    /// - Detecting and filtering outliers
    fn aggregate_extensions(&self, extensions: &[DydxVoteExtension]) -> Result<DydxVoteExtension, Error>;
}

/// dYdX Vote Extension Generator implementation
///
/// This implementation fetches price data from the Slinky sidecar service
/// and generates vote extensions for inclusion in validator votes.
pub struct DydxVoteExtensionGenerator {
    /// Address of the Slinky sidecar service
    pub slinky_address: String,

    /// Maximum age of price data to accept (milliseconds)
    pub max_price_age_ms: u64,
}

impl DydxVoteExtensionGenerator {
    /// Create a new vote extension generator
    pub fn new(slinky_address: String) -> Self {
        Self {
            slinky_address,
            max_price_age_ms: 1000, // 1 second default
        }
    }

    /// Create with custom max price age
    pub fn with_max_price_age(mut self, max_price_age_ms: u64) -> Self {
        self.max_price_age_ms = max_price_age_ms;
        self
    }
}

impl VoteExtensionGenerator for DydxVoteExtensionGenerator {
    fn generate_vote_extension(&self) -> Result<DydxVoteExtension, Error> {
        // In production, this would:
        // 1. Make an HTTP request to the Slinky sidecar
        // 2. Fetch the latest price data for all required markets
        // 3. Encode the prices in the vote extension format
        //
        // Example Slinky API call:
        // GET http://slinky:8080/s/latest_prices
        // Response: JSON with market prices
        //
        // For now, return an empty extension as a placeholder
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::msg(format!("System time error: {}", e)))?
            .as_millis() as u64;

        Ok(DydxVoteExtension {
            price_data: vec![],
            timestamp_ms: now_ms,
            version: 1,
        })
    }

    fn verify_vote_extension(&self, extension: &DydxVoteExtension) -> Result<(), Error> {
        // Verify the extension format and validity
        extension.verify()?;

        // In production, also verify:
        // - Price data format matches expected structure
        // - All required markets are present
        // - Prices are within reasonable bounds (e.g., not too far from previous block)
        // - Signature is valid (if using signed price data from Slinky)

        Ok(())
    }

    fn aggregate_extensions(&self, extensions: &[DydxVoteExtension]) -> Result<DydxVoteExtension, Error> {
        if extensions.is_empty() {
            return Err(Error::msg("Cannot aggregate empty extensions list"));
        }

        // In production, aggregation would:
        // 1. Parse price data from each extension
        // 2. For each market, calculate the median price across all validators
        // 3. Use the maximum timestamp (most recent data)
        // 4. Filter out any outliers (e.g., prices > 2 standard deviations from median)
        // 5. Return aggregated price data

        // Use the first non-empty extension
        for ext in extensions {
            if !ext.price_data.is_empty() {
                return Ok(ext.clone());
            }
        }

        // If all extensions are empty, return an error instead of empty extension
        Err(Error::msg("Cannot aggregate extensions: all extensions are empty"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_extension_empty() {
        let ext = DydxVoteExtension::empty();

        // Verify empty extension properties
        assert!(ext.price_data.is_empty());
        assert_eq!(ext.price_data.len(), 0);
        assert_eq!(ext.timestamp_ms, 0);
        assert_eq!(ext.version, 1);

        // Verify empty extension can be serialized
        let bytes = ext.to_bytes();
        assert!(!bytes.is_empty());

        // Verify empty extension can be deserialized
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert!(parsed.price_data.is_empty());
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn test_vote_extension_new() {
        let price_data = vec![1, 2, 3, 4];
        let timestamp = 12345;
        let ext = DydxVoteExtension::new(price_data.clone(), timestamp);

        // Verify all fields are set correctly
        assert_eq!(ext.price_data, price_data);
        assert_eq!(ext.price_data.len(), 4);
        assert_eq!(ext.timestamp_ms, timestamp);
        assert_eq!(ext.version, 1);

        // Verify extension can be round-tripped
        let bytes = ext.to_bytes();
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.price_data, price_data);
        assert_eq!(parsed.timestamp_ms, timestamp);
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn test_vote_extension_large_price_data() {
        // Test with large price data (simulating multiple price entries)
        let large_price_data = vec![7u8; 10_000]; // 10KB of price data
        let ext = DydxVoteExtension::new(large_price_data.clone(), 99999);

        assert_eq!(ext.price_data, large_price_data);
        assert_eq!(ext.price_data.len(), 10_000);

        // Verify serialization works with large data
        let bytes = ext.to_bytes();
        assert!(!bytes.is_empty());

        // Verify deserialization works with large data
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.price_data, large_price_data);
    }

    #[test]
    fn test_vote_extension_verify_valid_current_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let ext = DydxVoteExtension::new(vec![1, 2, 3], now_ms);

        // Should verify successfully with current timestamp
        assert!(ext.verify().is_ok(), "Vote extension with current timestamp should verify");
    }

    #[test]
    fn test_vote_extension_verify_slightly_old_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 500ms ago should still be valid
        let old_ms = now_ms.saturating_sub(500);
        let ext = DydxVoteExtension::new(vec![1, 2, 3], old_ms);

        assert!(ext.verify().is_ok(), "Vote extension from 500ms ago should verify");
    }

    #[test]
    fn test_vote_extension_verify_future_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 10 seconds in the future should fail
        let future_ms = now_ms + 10000;
        let ext = DydxVoteExtension::new(vec![1, 2, 3], future_ms);

        let result = ext.verify();
        assert!(result.is_err(), "Vote extension with future timestamp should fail verification");

        // Verify error message is informative
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("future") || err_msg.contains("timestamp"));
    }

    #[test]
    fn test_vote_extension_verify_very_future_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 1 hour in the future
        let future_ms = now_ms + (60 * 60 * 1000);
        let ext = DydxVoteExtension::new(vec![1, 2, 3], future_ms);

        let result = ext.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_vote_extension_verify_unsupported_version() {
        // Test with unsupported version
        let versions_to_test = [2, 10, 100, 255];

        for version in versions_to_test {
            let ext = DydxVoteExtension {
                price_data: vec![],
                timestamp_ms: 0,
                version,
            };

            let result = ext.verify();
            assert!(result.is_err(), "Version {} should be unsupported", version);

            let err_msg = format!("{}", result.unwrap_err());
            assert!(err_msg.contains("version") || err_msg.contains("supported"));
        }
    }

    #[test]
    fn test_vote_extension_verify_version_1_only_supported() {
        let ext = DydxVoteExtension {
            price_data: vec![1, 2, 3],
            timestamp_ms: 0,
            version: 1,
        };
        assert!(ext.verify().is_ok(), "Version 1 should be supported");
    }

    #[test]
    fn test_vote_extension_to_from_bytes_roundtrip() {
        let original = DydxVoteExtension::new(vec![1, 2, 3, 4, 5], 12345);

        // Serialize
        let bytes = original.to_bytes();
        assert!(!bytes.is_empty(), "Serialized bytes should not be empty");

        // Deserialize
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();

        // Verify all fields round-trip correctly
        assert_eq!(parsed.price_data, original.price_data);
        assert_eq!(parsed.timestamp_ms, original.timestamp_ms);
        assert_eq!(parsed.version, original.version);
    }

    #[test]
    fn test_vote_extension_serialization_deterministic() {
        let ext = DydxVoteExtension::new(vec![10, 20, 30], 99999);

        let bytes1 = ext.to_bytes();
        let bytes2 = ext.to_bytes();

        // Verify serialization is deterministic
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_vote_extension_empty_serialization() {
        let ext = DydxVoteExtension::empty();
        let bytes = ext.to_bytes();

        // Empty extension should still serialize
        assert!(!bytes.is_empty());

        // And deserialize back correctly
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert!(parsed.price_data.is_empty());
    }

    #[test]
    fn test_generator_default_address() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());

        assert_eq!(gen.slinky_address, "localhost:8080");
        assert_eq!(gen.max_price_age_ms, 1000); // Default max age
    }

    #[test]
    fn test_generator_with_custom_address() {
        let custom_address = "slinky.example.com:9090";
        let gen = DydxVoteExtensionGenerator::new(custom_address.to_string());

        assert_eq!(gen.slinky_address, custom_address);
        assert_eq!(gen.max_price_age_ms, 1000);
    }

    #[test]
    fn test_generator_with_max_price_age() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_max_price_age(5000);

        assert_eq!(gen.slinky_address, "localhost:8080");
        assert_eq!(gen.max_price_age_ms, 5000);
    }

    #[test]
    fn test_generator_with_zero_max_price_age() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_max_price_age(0);

        assert_eq!(gen.max_price_age_ms, 0);
    }

    #[test]
    fn test_aggregate_empty_returns_error() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let extensions: &[DydxVoteExtension] = &[];

        let result = gen.aggregate_extensions(extensions);

        // Empty list should return error
        assert!(result.is_err(), "Aggregating empty extensions should fail");

        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("empty") || err_msg.contains("No extensions"));
    }

    #[test]
    fn test_aggregate_single() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext = DydxVoteExtension::new(vec![1, 2, 3], 12345);

        let result = gen.aggregate_extensions(&[ext.clone()]).unwrap();

        // Should return the same extension when there's only one
        assert_eq!(result.price_data, ext.price_data);
        assert_eq!(result.timestamp_ms, ext.timestamp_ms);
        assert_eq!(result.version, ext.version);
    }

    #[test]
    fn test_aggregate_two_extensions() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::new(vec![1, 2, 3], 12345);
        let ext2 = DydxVoteExtension::new(vec![4, 5, 6], 67890);

        let result = gen.aggregate_extensions(&[ext1, ext2]).unwrap();

        // Should return the first extension (prioritize first)
        assert_eq!(result.price_data, vec![1, 2, 3]);
        assert_eq!(result.timestamp_ms, 12345);
    }

    #[test]
    fn test_aggregate_multiple_uses_first_non_empty() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::new(vec![1, 2, 3], 12345);
        let ext2 = DydxVoteExtension::new(vec![4, 5, 6], 67890);
        let ext3 = DydxVoteExtension::empty();

        // Order: empty, non-empty, non-empty
        let result = gen.aggregate_extensions(&[ext3.clone(), ext1.clone(), ext2]).unwrap();

        // Should use first non-empty extension (ext1)
        assert_eq!(result.price_data, ext1.price_data);
        assert_eq!(result.timestamp_ms, 12345);
    }

    #[test]
    fn test_aggregate_all_empty_returns_error() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::empty();
        let ext2 = DydxVoteExtension::empty();
        let ext3 = DydxVoteExtension::empty();

        let result = gen.aggregate_extensions(&[ext1, ext2, ext3]);

        // All empty should return error
        assert!(result.is_err());
    }

    #[test]
    fn test_aggregate_preserves_first_extension() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());

        // Create extensions with different data
        let extensions: Vec<DydxVoteExtension> = vec![
            DydxVoteExtension::new(vec![10, 20, 30], 11111),
            DydxVoteExtension::new(vec![40, 50, 60], 22222),
            DydxVoteExtension::new(vec![70, 80, 90], 33333),
        ];

        // Test that first extension in the slice is always used
        // Test 1: Only ext1
        let result = gen.aggregate_extensions(&[extensions[0].clone()]).unwrap();
        assert_eq!(result.price_data, vec![10, 20, 30]);
        assert_eq!(result.timestamp_ms, 11111);

        // Test 2: ext1 and ext2 - should use ext1 (first)
        let result = gen.aggregate_extensions(&[extensions[0].clone(), extensions[1].clone()]).unwrap();
        assert_eq!(result.price_data, vec![10, 20, 30]);
        assert_eq!(result.timestamp_ms, 11111);

        // Test 3: ext2 and ext3 - should use ext2 (first in that slice)
        let result = gen.aggregate_extensions(&[extensions[1].clone(), extensions[2].clone()]).unwrap();
        assert_eq!(result.price_data, vec![40, 50, 60]);
        assert_eq!(result.timestamp_ms, 22222);

        // Test 4: All three - should use ext1 (first overall)
        let result = gen.aggregate_extensions(&[extensions[0].clone(), extensions[1].clone(), extensions[2].clone()]).unwrap();
        assert_eq!(result.price_data, vec![10, 20, 30]);
        assert_eq!(result.timestamp_ms, 11111);
    }
}
