// Vote extension generation trait
//
// This module defines the trait for generating vote extensions,
// which allow applications to inject arbitrary data into votes
// (e.g., oracle prices for dYdX v4).

use crate::core::Error;
use async_trait::async_trait;

/// Trait for generating vote extensions.
///
/// Vote extensions allow applications to inject arbitrary data into votes
/// during the consensus process. For example, dYdX v4 uses vote extensions
/// to include oracle price data from Slinky.
///
/// # Example
///
/// ```text
/// use consensus_traits::vote_extension::VoteExtensionGenerator;
/// use async_trait::async_trait;
///
/// struct SlinkyOracleExtension;
///
/// #[async_trait]
/// impl VoteExtensionGenerator for SlinkyOracleExtension {
///     async fn generate_vote_extension(
///         &self,
///         epoch: u64,
///         round: u64,
///     ) -> Result<Vec<u8>, Error> {
///         // Fetch current prices from oracle
///         let prices = fetch_prices().await?;
///         Ok(prices.encode())
///     }
/// }
/// ```
#[async_trait]
pub trait VoteExtensionGenerator: Send + Sync {
    /// Generate a vote extension for the given epoch and round.
    ///
    /// This method is called by the consensus engine before creating a vote.
    /// The returned extension data will be included in the vote and signed
    /// along with the vote data.
    ///
    /// # Parameters
    ///
    /// * `epoch` - The current epoch number
    /// * `round` - The current round number within the epoch
    ///
    /// # Returns
    ///
    /// A byte vector containing the extension data. Returns an empty vector
    /// if no extension is needed for this round.
    ///
    /// # Errors
    ///
    /// Returns an error if extension generation fails (e.g., oracle unavailable).
    async fn generate_vote_extension(&self, epoch: u64, round: u64) -> Result<Vec<u8>, Error>;
}

/// A no-op vote extension generator that returns empty extensions.
///
/// This is useful for blockchains that don't use vote extensions.
#[derive(Debug, Clone, Copy)]
pub struct NoOpVoteExtensionGenerator;

#[async_trait]
impl VoteExtensionGenerator for NoOpVoteExtensionGenerator {
    async fn generate_vote_extension(&self, _epoch: u64, _round: u64) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestExtensionGenerator;

    #[async_trait]
    impl VoteExtensionGenerator for TestExtensionGenerator {
        async fn generate_vote_extension(&self, epoch: u64, round: u64) -> Result<Vec<u8>, Error> {
            Ok(format!("epoch={},round={}", epoch, round).into_bytes())
        }
    }

    #[tokio::test]
    async fn test_vote_extension_generator() {
        let generator = TestExtensionGenerator;
        let extension = generator.generate_vote_extension(1, 5).await.unwrap();
        assert_eq!(extension, b"epoch=1,round=5");
    }

    #[tokio::test]
    async fn test_noop_extension_generator() {
        let generator = NoOpVoteExtensionGenerator;
        let extension = generator.generate_vote_extension(1, 5).await.unwrap();
        assert!(extension.is_empty());
    }
}
