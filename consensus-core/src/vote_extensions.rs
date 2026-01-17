// Vote extension generation support
//
// This module provides integration between AptosBFT consensus
// and vote extension generation for application-specific data.

use consensus_traits::VoteExtensionGenerator;
use std::sync::Arc;

/// Vote extension manager for consensus integration.
///
/// This wraps a VoteExtensionGenerator and provides convenience methods
/// for the consensus engine to generate vote extensions.
#[derive(Clone)]
pub struct VoteExtensionManager {
    /// The underlying vote extension generator
    generator: Arc<dyn VoteExtensionGenerator + Send + Sync>,
}

impl VoteExtensionManager {
    /// Create a new vote extension manager.
    pub fn new(generator: Arc<dyn VoteExtensionGenerator + Send + Sync>) -> Self {
        Self { generator }
    }

    /// Create a vote extension manager with a no-op generator.
    ///
    /// This is useful for blockchains that don't use vote extensions.
    pub fn noop() -> Self {
        Self {
            generator: Arc::new(consensus_traits::NoOpVoteExtensionGenerator),
        }
    }

    /// Generate a vote extension for the given epoch and round.
    pub async fn generate_extension(&self, epoch: u64, round: u64) -> Result<Vec<u8>, anyhow::Error> {
        self.generator
            .generate_vote_extension(epoch, round)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to generate vote extension: {}", e))
    }

    /// Get the underlying generator.
    pub fn generator(&self) -> &Arc<dyn VoteExtensionGenerator + Send + Sync> {
        &self.generator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestGenerator;

    #[async_trait::async_trait]
    impl VoteExtensionGenerator for TestGenerator {
        async fn generate_vote_extension(
            &self,
            epoch: u64,
            round: u64,
        ) -> Result<Vec<u8>, consensus_traits::core::Error> {
            Ok(format!("epoch={},round={}", epoch, round).into_bytes())
        }
    }

    #[tokio::test]
    async fn test_vote_extension_manager() {
        let manager = VoteExtensionManager::new(Arc::new(TestGenerator));
        let extension = manager.generate_extension(1, 5).await.unwrap();
        assert_eq!(extension, b"epoch=1,round=5");
    }

    #[tokio::test]
    async fn test_noop_manager() {
        let manager = VoteExtensionManager::noop();
        let extension = manager.generate_extension(1, 5).await.unwrap();
        assert!(extension.is_empty());
    }
}
