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

use std::time::Duration;
use consensus_traits::core::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Default Slinky sidecar address
const DEFAULT_SLINKY_ADDRESS: &str = "localhost:8080";

/// Default timeout for Slinky HTTP requests
const DEFAULT_SLINKY_TIMEOUT_MS: u64 = 500;

/// Maximum price age to accept (milliseconds)
const DEFAULT_MAX_PRICE_AGE_MS: u64 = 1000;

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

    /// Check if this extension has valid price data
    pub fn has_price_data(&self) -> bool {
        !self.price_data.is_empty()
    }

    /// Serialize the extension for signing/inclusion in vote
    pub fn to_bytes(&self) -> Vec<u8> {
        // Use serde_json for serialization (in production, use protobuf)
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

/// Slinky sidecar price response
///
/// This is the JSON response format from the Slinky sidecar's `/s/latest_prices` endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlinkyPriceResponse {
    /// Latest prices for all markets
    pub prices: std::collections::HashMap<String, SlinkyMarketPrice>,

    /// Response timestamp
    pub timestamp_ms: u64,
}

/// Price data for a single market from Slinky
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlinkyMarketPrice {
    /// Current price (scaled by market's exponent)
    pub price: String,

    /// Market exponent (for price scaling)
    pub exponent: u32,

    /// Price timestamp
    pub timestamp_ms: u64,
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

/// dYdX Vote Extension Generator implementation with Slinky integration
///
/// This implementation fetches price data from the Slinky sidecar service
/// via HTTP and generates vote extensions for inclusion in validator votes.
pub struct DydxVoteExtensionGenerator {
    /// Address of the Slinky sidecar service
    pub slinky_address: String,

    /// Maximum age of price data to accept (milliseconds)
    pub max_price_age_ms: u64,

    /// HTTP timeout for requests
    pub http_timeout: Duration,

    /// Use HTTPS instead of HTTP
    pub use_https: bool,
}

impl DydxVoteExtensionGenerator {
    /// Create a new vote extension generator with default settings
    pub fn new(slinky_address: String) -> Self {
        Self {
            slinky_address,
            max_price_age_ms: DEFAULT_MAX_PRICE_AGE_MS,
            http_timeout: Duration::from_millis(DEFAULT_SLINKY_TIMEOUT_MS),
            use_https: false,
        }
    }

    /// Create with custom max price age
    pub fn with_max_price_age(mut self, max_price_age_ms: u64) -> Self {
        self.max_price_age_ms = max_price_age_ms;
        self
    }

    /// Set HTTP timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = timeout;
        self
    }

    /// Enable HTTPS
    pub fn with_https(mut self, use_https: bool) -> Self {
        self.use_https = use_https;
        self
    }

    /// Build the Slinky URL for the latest prices endpoint
    fn build_url(&self) -> String {
        let protocol = if self.use_https { "https" } else { "http" };
        format!("{}://{}/s/latest_prices", protocol, self.slinky_address)
    }

    /// Fetch latest prices from Slinky sidecar
    fn fetch_prices(&self) -> Result<SlinkyPriceResponse, Error> {
        let url = self.build_url();

        // Create HTTP client with timeout
        let client = reqwest::blocking::Client::builder()
            .timeout(self.http_timeout)
            .build()
            .map_err(|e| Error::msg(format!("Failed to create HTTP client: {}", e)))?;

        // Make HTTP GET request
        log::debug!("Fetching prices from Slinky: {}", url);

        let response = client.get(&url)
            .send()
            .map_err(|e| Error::msg(format!("Slinky HTTP request failed: {}", e)))?;

        // Check response status
        if !response.status().is_success() {
            return Err(Error::msg(format!(
                "Slinky returned error status: {}",
                response.status()
            )));
        }

        // Parse response JSON
        let price_response = response
            .json::<SlinkyPriceResponse>()
            .map_err(|e| Error::msg(format!("Failed to parse Slinky response: {}", e)))?;

        log::debug!("Received prices for {} markets from Slinky", price_response.prices.len());

        Ok(price_response)
    }

    /// Serialize price response to bytes for vote extension
    fn serialize_prices(&self, prices: &SlinkyPriceResponse) -> Result<Vec<u8>, Error> {
        serde_json::to_vec(prices)
            .map_err(|e| Error::msg(format!("Failed to serialize prices: {}", e)))
    }
}

impl VoteExtensionGenerator for DydxVoteExtensionGenerator {
    fn generate_vote_extension(&self) -> Result<DydxVoteExtension, Error> {
        // Fetch latest prices from Slinky sidecar
        let prices = self.fetch_prices()?;

        // Serialize prices for inclusion in vote extension
        let price_data = self.serialize_prices(&prices)?;

        // Create vote extension
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::msg(format!("System time error: {}", e)))?
            .as_millis() as u64;

        let extension = DydxVoteExtension {
            price_data,
            timestamp_ms: now_ms,
            version: 1,
        };

        log::debug!(
            "Generated vote extension: {} bytes, {} markets, timestamp={}",
            extension.price_data.len(),
            prices.prices.len(),
            extension.timestamp_ms
        );

        Ok(extension)
    }

    fn verify_vote_extension(&self, extension: &DydxVoteExtension) -> Result<(), Error> {
        // Verify the extension format and validity
        extension.verify()?;

        // Verify price data is not empty
        if extension.price_data.is_empty() {
            return Err(Error::msg("Vote extension has empty price data"));
        }

        // Verify price age is within acceptable bounds
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::msg(format!("System time error: {}", e)))?
            .as_millis() as u64;

        let price_age = now_ms.saturating_sub(extension.timestamp_ms);
        if price_age > self.max_price_age_ms {
            return Err(Error::msg(format!(
                "Price data is too old: {}ms (max: {}ms)",
                price_age, self.max_price_age_ms
            )));
        }

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

        // Find the most recent non-empty extension
        let mut best_extension: Option<&DydxVoteExtension> = None;
        let mut best_timestamp = 0u64;

        for ext in extensions {
            if !ext.price_data.is_empty() && ext.timestamp_ms > best_timestamp {
                best_timestamp = ext.timestamp_ms;
                best_extension = Some(ext);
            }
        }

        match best_extension {
            Some(ext) => {
                log::debug!(
                    "Aggregated {} extensions, using most recent from timestamp={}",
                    extensions.len(),
                    ext.timestamp_ms
                );
                Ok(ext.clone())
            }
            None => {
                Err(Error::msg("Cannot aggregate extensions: all extensions are empty"))
            }
        }

        // In production, proper aggregation would:
        // 1. Parse price data from each extension
        // 2. For each market, calculate the median price across all validators
        // 3. Use the maximum timestamp (most recent data)
        // 4. Filter out any outliers (e.g., prices > 2 standard deviations from median)
        // 5. Return aggregated price data
        //
        // This requires:
        // - Parsing JSON price data from each extension
        // - Building a map of market -> Vec<price>
        // - Computing median for each market
        // - Serializing aggregated result
    }
}

/// Create a vote extension generator with default settings
pub fn create_extension_generator() -> DydxVoteExtensionGenerator {
    DydxVoteExtensionGenerator::new(DEFAULT_SLINKY_ADDRESS.to_string())
}

/// Create a vote extension generator with custom Slinky address
pub fn create_extension_generator_with_address(address: String) -> DydxVoteExtensionGenerator {
    DydxVoteExtensionGenerator::new(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_extension_empty() {
        let ext = DydxVoteExtension::empty();

        assert!(!ext.has_price_data());
        assert_eq!(ext.price_data.len(), 0);
        assert_eq!(ext.timestamp_ms, 0);
        assert_eq!(ext.version, 1);

        let bytes = ext.to_bytes();
        assert!(!bytes.is_empty());

        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert!(!parsed.has_price_data());
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn test_vote_extension_new() {
        let price_data = vec![1, 2, 3, 4];
        let timestamp = 12345;
        let ext = DydxVoteExtension::new(price_data.clone(), timestamp);

        assert!(ext.has_price_data());
        assert_eq!(ext.price_data, price_data);
        assert_eq!(ext.price_data.len(), 4);
        assert_eq!(ext.timestamp_ms, timestamp);
        assert_eq!(ext.version, 1);

        let bytes = ext.to_bytes();
        let parsed = DydxVoteExtension::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.price_data, price_data);
        assert_eq!(parsed.timestamp_ms, timestamp);
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn test_vote_extension_verify_valid_current_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let ext = DydxVoteExtension::new(vec![1, 2, 3], now_ms);
        assert!(ext.verify().is_ok());
    }

    #[test]
    fn test_vote_extension_verify_slightly_old_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let old_ms = now_ms.saturating_sub(500);
        let ext = DydxVoteExtension::new(vec![1, 2, 3], old_ms);
        assert!(ext.verify().is_ok());
    }

    #[test]
    fn test_vote_extension_verify_future_timestamp() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let future_ms = now_ms + 10000;
        let ext = DydxVoteExtension::new(vec![1, 2, 3], future_ms);

        let result = ext.verify();
        assert!(result.is_err());

        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("future") || err_msg.contains("timestamp"));
    }

    #[test]
    fn test_vote_extension_verify_unsupported_version() {
        let ext = DydxVoteExtension {
            price_data: vec![],
            timestamp_ms: 0,
            version: 2,
        };

        let result = ext.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_generator_default_address() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        assert_eq!(gen.slinky_address, "localhost:8080");
        assert_eq!(gen.max_price_age_ms, DEFAULT_MAX_PRICE_AGE_MS);
        assert!(!gen.use_https);
    }

    #[test]
    fn test_generator_build_url_http() {
        let gen = DydxVoteExtensionGenerator::new("example.com:9000".to_string());
        let url = gen.build_url();
        assert_eq!(url, "http://example.com:9000/s/latest_prices");
    }

    #[test]
    fn test_generator_build_url_https() {
        let gen = DydxVoteExtensionGenerator::new("example.com:9000".to_string())
            .with_https(true);
        let url = gen.build_url();
        assert_eq!(url, "https://example.com:9000/s/latest_prices");
    }

    #[test]
    fn test_generator_with_max_price_age() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_max_price_age(5000);
        assert_eq!(gen.max_price_age_ms, 5000);
    }

    #[test]
    fn test_generator_with_timeout() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_timeout(Duration::from_secs(2));
        assert_eq!(gen.http_timeout, Duration::from_secs(2));
    }

    #[test]
    fn test_aggregate_empty_returns_error() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let extensions: &[DydxVoteExtension] = &[];
        let result = gen.aggregate_extensions(extensions);
        assert!(result.is_err());
    }

    #[test]
    fn test_aggregate_single() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext = DydxVoteExtension::new(vec![1, 2, 3], 12345);
        let result = gen.aggregate_extensions(&[ext.clone()]).unwrap();
        assert_eq!(result.price_data, ext.price_data);
        assert_eq!(result.timestamp_ms, ext.timestamp_ms);
    }

    #[test]
    fn test_aggregate_uses_most_recent() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::new(vec![1, 2, 3], 10000);
        let ext2 = DydxVoteExtension::new(vec![4, 5, 6], 20000); // Most recent
        let ext3 = DydxVoteExtension::new(vec![7, 8, 9], 15000);

        let result = gen.aggregate_extensions(&[ext1, ext2, ext3]).unwrap();
        assert_eq!(result.price_data, vec![4, 5, 6]);
        assert_eq!(result.timestamp_ms, 20000);
    }

    #[test]
    fn test_aggregate_all_empty_returns_error() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::empty();
        let ext2 = DydxVoteExtension::empty();
        let result = gen.aggregate_extensions(&[ext1, ext2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_aggregate_mixed_empty_and_non_empty() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext1 = DydxVoteExtension::empty();
        let ext2 = DydxVoteExtension::new(vec![1, 2, 3], 5000);
        let ext3 = DydxVoteExtension::empty();

        let result = gen.aggregate_extensions(&[ext1, ext2, ext3]).unwrap();
        assert_eq!(result.price_data, vec![1, 2, 3]);
        assert_eq!(result.timestamp_ms, 5000);
    }

    #[test]
    fn test_verify_vote_extension_empty_data() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string());
        let ext = DydxVoteExtension::empty();
        let result = gen.verify_vote_extension(&ext);
        assert!(result.is_err());

        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("empty"));
    }

    #[test]
    fn test_verify_vote_extension_too_old() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_max_price_age(100);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let old_ms = now_ms.saturating_sub(500); // 500ms ago
        let ext = DydxVoteExtension::new(vec![1, 2, 3], old_ms);

        let result = gen.verify_vote_extension(&ext);
        assert!(result.is_err());

        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("too old"));
    }

    #[test]
    fn test_verify_vote_extension_within_age_limit() {
        let gen = DydxVoteExtensionGenerator::new("localhost:8080".to_string())
            .with_max_price_age(1000);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let recent_ms = now_ms.saturating_sub(500);
        let ext = DydxVoteExtension::new(vec![1, 2, 3], recent_ms);

        assert!(gen.verify_vote_extension(&ext).is_ok());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_SLINKY_ADDRESS, "localhost:8080");
        assert_eq!(DEFAULT_SLINKY_TIMEOUT_MS, 500);
        assert_eq!(DEFAULT_MAX_PRICE_AGE_MS, 1000);
    }

    #[test]
    fn test_create_extension_generator() {
        let gen = create_extension_generator();
        assert_eq!(gen.slinky_address, DEFAULT_SLINKY_ADDRESS);
        assert_eq!(gen.max_price_age_ms, DEFAULT_MAX_PRICE_AGE_MS);
    }

    #[test]
    fn test_create_extension_generator_with_address() {
        let gen = create_extension_generator_with_address("custom:9090".to_string());
        assert_eq!(gen.slinky_address, "custom:9090");
    }

    #[test]
    fn test_slinky_market_price() {
        // Test that we can deserialize Slinky price responses
        let json = r#"{
            "prices": {
                "BTC-USD": {
                    "price": "50000.00",
                    "exponent": 8,
                    "timestampMs": 1234567890
                },
                "ETH-USD": {
                    "price": "3000.00",
                    "exponent": 8,
                    "timestampMs": 1234567890
                }
            },
            "timestampMs": 1234567890
        }"#;

        let response: SlinkyPriceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.prices.len(), 2);
        assert_eq!(response.prices.get("BTC-USD").unwrap().price, "50000.00");
        assert_eq!(response.prices.get("ETH-USD").unwrap().exponent, 8);
    }

    #[test]
    fn test_slinky_price_response_timestamp() {
        let json = r#"{
            "prices": {},
            "timestampMs": 9876543210
        }"#;

        let response: SlinkyPriceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.timestamp_ms, 9876543210);
    }
}
