// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Consensus pipeline module.
//!
//! This module provides a processing pipeline for consensus operations,
//! organizing the flow from receiving messages to committing blocks.

use consensus_traits::{
    block::{Block, Vote, BlockMetadata, QuorumCertificate},
    core::Hash as HashTrait,
};
use std::{
    fmt::{self, Debug, Display},
    sync::Arc,
    time::Duration,
};
use anyhow::Result;

/// Pipeline stage identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// Receiving stage - accepting incoming messages
    Receiving,
    /// Validation stage - checking message validity
    Validation,
    /// Processing stage - executing consensus logic
    Processing,
    /// Commit stage - committing blocks to chain
    Commit,
}

impl Stage {
    /// Get the next stage in the pipeline.
    pub fn next(&self) -> Option<Stage> {
        match self {
            Stage::Receiving => Some(Stage::Validation),
            Stage::Validation => Some(Stage::Processing),
            Stage::Processing => Some(Stage::Commit),
            Stage::Commit => None,
        }
    }

    /// Get the stage name as a string.
    pub fn name(&self) -> &str {
        match self {
            Stage::Receiving => "receiving",
            Stage::Validation => "validation",
            Stage::Processing => "processing",
            Stage::Commit => "commit",
        }
    }
}

impl Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Configuration for the consensus pipeline.
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    /// Timeout for each stage
    pub stage_timeout_ms: u64,

    /// Maximum number of concurrent processing operations
    pub max_concurrent: usize,

    /// Whether to validate messages immediately
    pub validate_immediately: bool,

    /// Whether to process messages in parallel
    pub parallel_processing: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            stage_timeout_ms: 5000,
            max_concurrent: 10,
            validate_immediately: true,
            parallel_processing: false,
        }
    }
}

impl PipelineConfig {
    /// Get the stage timeout as a Duration.
    pub fn stage_timeout(&self) -> Duration {
        Duration::from_millis(self.stage_timeout_ms)
    }
}

/// Result of processing a message through a stage.
#[derive(Clone, Debug)]
pub enum StageResult {
    /// Message should continue to the next stage
    Continue,

    /// Message should be stopped here
    Stop {
        /// Reason for stopping
        reason: String,
    },

    /// Message should be skipped
    Skip {
        /// Reason for skipping
        reason: String,
    },
}

impl StageResult {
    /// Create a continue result.
    pub fn continue_to_next() -> Self {
        StageResult::Continue
    }

    /// Create a stop result.
    pub fn stop(reason: impl Into<String>) -> Self {
        StageResult::Stop {
            reason: reason.into(),
        }
    }

    /// Create a skip result.
    pub fn skip(reason: impl Into<String>) -> Self {
        StageResult::Skip {
            reason: reason.into(),
        }
    }

    /// Check if the result indicates continuation.
    pub fn should_continue(&self) -> bool {
        matches!(self, StageResult::Continue)
    }
}

/// Processing pipeline for consensus operations.
///
/// The pipeline organizes the flow of messages and blocks through
/// different stages of consensus processing.
pub struct Pipeline<B, V>
where
    B: Block,
    V: Vote,
{
    /// Pipeline configuration
    config: PipelineConfig,

    /// Current stage
    current_stage: Stage,

    /// Statistics
    stats: PipelineStats,

    /// Phantom data for generic types
    _phantom: std::marker::PhantomData<(B, V)>,
}

/// Statistics for pipeline processing.
#[derive(Clone, Debug, Default)]
pub struct PipelineStats {
    /// Messages processed at each stage
    pub stage_counts: [u64; 4],

    /// Total messages processed
    pub total_processed: u64,

    /// Messages that failed
    pub failed: u64,

    /// Messages that were skipped
    pub skipped: u64,
}

impl<B, V> Pipeline<B, V>
where
    B: Block,
    V: Vote,
{
    /// Create a new pipeline.
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            current_stage: Stage::Receiving,
            stats: PipelineStats::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the current stage.
    pub fn current_stage(&self) -> Stage {
        self.current_stage
    }

    /// Get the pipeline statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Get the pipeline configuration.
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Reset the pipeline to the receiving stage.
    pub fn reset(&mut self) {
        self.current_stage = Stage::Receiving;
    }

    /// Process a block through the pipeline.
    ///
    /// The block will progress through each stage until it completes
    /// or is stopped/skipped at some stage.
    pub fn process_block(&mut self, block: &Arc<B>) -> Result<PipelineResult> {
        let mut current_stage = Stage::Receiving;

        loop {
            // Record processing at this stage
            self.stats.stage_counts[current_stage as usize] += 1;
            self.stats.total_processed += 1;

            // Process at current stage
            let result = self.process_at_stage(current_stage, block)?;

            match result {
                StageResult::Continue => {
                    // Move to next stage
                    if let Some(next) = current_stage.next() {
                        current_stage = next;
                    } else {
                        // Pipeline complete
                        break;
                    }
                }
                StageResult::Stop { reason } => {
                    self.stats.failed += 1;
                    return Ok(PipelineResult::Stopped {
                        stage: current_stage,
                        reason,
                    });
                }
                StageResult::Skip { reason } => {
                    self.stats.skipped += 1;
                    return Ok(PipelineResult::Skipped {
                        stage: current_stage,
                        reason,
                    });
                }
            }
        }

        Ok(PipelineResult::Completed)
    }

    /// Process a vote through the pipeline.
    ///
    /// Votes typically follow a simpler path than blocks.
    pub fn process_vote(&mut self, vote: &V) -> Result<PipelineResult> {
        // Simplified: votes skip most stages
        // In production, this would be more sophisticated

        self.stats.stage_counts[Stage::Receiving as usize] += 1;
        self.stats.total_processed += 1;

        // Votes typically go directly to processing stage
        let result = self.process_vote_at_stage(vote)?;

        match result {
            StageResult::Continue => Ok(PipelineResult::Completed),
            StageResult::Stop { reason } => Ok(PipelineResult::Stopped {
                stage: Stage::Processing,
                reason,
            }),
            StageResult::Skip { reason } => Ok(PipelineResult::Skipped {
                stage: Stage::Processing,
                reason,
            }),
        }
    }

    /// Process at a specific stage.
    fn process_at_stage(&self, stage: Stage, block: &Arc<B>) -> Result<StageResult> {
        match stage {
            Stage::Receiving => {
                // Check if we should accept this block
                // In production, this would check for duplicates, valid round, etc.
                Ok(StageResult::continue_to_next())
            }
            Stage::Validation => {
                // Validate the block
                // In production, this would check signatures, parent hash, etc.
                Ok(StageResult::continue_to_next())
            }
            Stage::Processing => {
                // Process the block through consensus logic
                // In production, this would update state, broadcast votes, etc.
                Ok(StageResult::continue_to_next())
            }
            Stage::Commit => {
                // Commit the block to the chain
                // In production, this would update storage, notify listeners, etc.
                Ok(StageResult::continue_to_next())
            }
        }
    }

    /// Process a vote at the processing stage.
    fn process_vote_at_stage(&self, _vote: &V) -> Result<StageResult> {
        // Simplified: always continue
        // In production, this would validate the vote, update pending votes, etc.
        Ok(StageResult::continue_to_next())
    }

    /// Process a quorum certificate.
    ///
    /// QCs typically trigger immediate advancement.
    ///
    /// NOTE: This is a simplified placeholder. In production, you'd need to
    /// specify the concrete QC type as a generic parameter or use a different approach.
    pub fn process_qc(&mut self) -> Result<PipelineResult> {
        // Simplified: QCs trigger fast processing
        // In production, this would validate the QC and advance rounds
        Ok(PipelineResult::Completed)
    }
}

/// Result of pipeline processing.
#[derive(Clone, Debug)]
pub enum PipelineResult {
    /// Processing completed successfully
    Completed,

    /// Processing was stopped at a stage
    Stopped {
        /// Stage where processing stopped
        stage: Stage,
        /// Reason for stopping
        reason: String,
    },

    /// Processing was skipped at a stage
    Skipped {
        /// Stage where processing was skipped
        stage: Stage,
        /// Reason for skipping
        reason: String,
    },
}

impl PipelineResult {
    /// Check if the result indicates completion.
    pub fn is_completed(&self) -> bool {
        matches!(self, PipelineResult::Completed)
    }

    /// Get the stage where processing ended (if any).
    pub fn final_stage(&self) -> Option<Stage> {
        match self {
            PipelineResult::Completed => None,
            PipelineResult::Stopped { stage, .. } => Some(*stage),
            PipelineResult::Skipped { stage, .. } => Some(*stage),
        }
    }
}

/// Builder for creating a pipeline.
pub struct PipelineBuilder {
    config: PipelineConfig,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineBuilder {
    /// Create a new pipeline builder.
    pub fn new() -> Self {
        Self {
            config: PipelineConfig::default(),
        }
    }

    /// Set the stage timeout.
    pub fn stage_timeout_ms(mut self, timeout: u64) -> Self {
        self.config.stage_timeout_ms = timeout;
        self
    }

    /// Set the maximum concurrent operations.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.config.max_concurrent = max;
        self
    }

    /// Enable or disable immediate validation.
    pub fn validate_immediately(mut self, validate: bool) -> Self {
        self.config.validate_immediately = validate;
        self
    }

    /// Enable or disable parallel processing.
    pub fn parallel_processing(mut self, parallel: bool) -> Self {
        self.config.parallel_processing = parallel;
        self
    }

    /// Build the pipeline.
    pub fn build<B, V>(self) -> Pipeline<B, V>
    where
        B: Block,
        V: Vote,
    {
        Pipeline::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockBlock, MockVote};

    #[test]
    fn test_stage_ordering() {
        assert_eq!(Stage::Receiving.next(), Some(Stage::Validation));
        assert_eq!(Stage::Validation.next(), Some(Stage::Processing));
        assert_eq!(Stage::Processing.next(), Some(Stage::Commit));
        assert_eq!(Stage::Commit.next(), None);
    }

    #[test]
    fn test_stage_display() {
        assert_eq!(format!("{}", Stage::Validation), "validation");
    }

    #[test]
    fn test_stage_result() {
        assert!(StageResult::continue_to_next().should_continue());
        assert!(!StageResult::stop("test").should_continue());
        assert!(!StageResult::skip("test").should_continue());
    }

    #[test]
    fn test_pipeline_config() {
        let config = PipelineConfig::default();
        assert_eq!(config.stage_timeout_ms, 5000);
        assert_eq!(config.max_concurrent, 10);
        assert!(config.validate_immediately);
    }

    #[test]
    fn test_pipeline_stats() {
        let mut stats = PipelineStats::default();
        stats.stage_counts[0] = 10;
        stats.total_processed = 10;

        assert_eq!(stats.stage_counts[0], 10);
        assert_eq!(stats.total_processed, 10);
    }

    #[test]
    fn test_pipeline() {
        let config = PipelineConfig::default();
        let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        let block = Arc::new(MockBlock::genesis());
        let result = pipeline.process_block(&block).unwrap();

        // With placeholder implementations, should complete
        assert!(result.is_completed());

        // Verify stats were updated
        assert_eq!(pipeline.stats().total_processed, 4); // 4 stages
        assert_eq!(pipeline.stats().stage_counts[0], 1); // Receiving
    }

    #[test]
    fn test_pipeline_builder() {
        let pipeline = PipelineBuilder::new()
            .stage_timeout_ms(1000)
            .max_concurrent(5)
            .validate_immediately(false)
            .parallel_processing(true)
            .build::<MockBlock, MockVote>();

        assert_eq!(pipeline.config().stage_timeout_ms, 1000);
        assert_eq!(pipeline.config().max_concurrent, 5);
        assert!(!pipeline.config().validate_immediately);
        assert!(pipeline.config().parallel_processing);
    }

    #[test]
    fn test_pipeline_reset() {
        let config = PipelineConfig::default();
        let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        pipeline.current_stage = Stage::Commit;
        pipeline.reset();

        assert_eq!(pipeline.current_stage(), Stage::Receiving);
    }

    #[test]
    fn test_stage_name() {
        assert_eq!(Stage::Receiving.name(), "receiving");
        assert_eq!(Stage::Validation.name(), "validation");
        assert_eq!(Stage::Processing.name(), "processing");
        assert_eq!(Stage::Commit.name(), "commit");
    }

    #[test]
    fn test_stage_result_constructors() {
        let _ = StageResult::continue_to_next();
        let _ = StageResult::stop("invalid block");
        let _ = StageResult::skip("duplicate");
    }

    #[test]
    fn test_pipeline_config_stage_timeout() {
        let config = PipelineConfig::default();
        let duration = config.stage_timeout();
        assert_eq!(duration.as_millis(), 5000);
    }

    #[test]
    fn test_pipeline_config_new() {
        let config = PipelineConfig {
            stage_timeout_ms: 2000,
            max_concurrent: 20,
            validate_immediately: false,
            parallel_processing: true,
        };
        assert_eq!(config.stage_timeout_ms, 2000);
        assert_eq!(config.max_concurrent, 20);
        assert!(!config.validate_immediately);
        assert!(config.parallel_processing);
    }

    #[test]
    fn test_pipeline_result_constructors() {
        let completed = PipelineResult::Completed;
        assert!(completed.is_completed());
        assert_eq!(completed.final_stage(), None);

        let stopped = PipelineResult::Stopped {
            stage: Stage::Validation,
            reason: "invalid".to_string(),
        };
        assert!(!stopped.is_completed());
        assert_eq!(stopped.final_stage(), Some(Stage::Validation));

        let skipped = PipelineResult::Skipped {
            stage: Stage::Processing,
            reason: "old".to_string(),
        };
        assert!(!skipped.is_completed());
        assert_eq!(skipped.final_stage(), Some(Stage::Processing));
    }

    #[test]
    fn test_process_vote() {
        let config = PipelineConfig::default();
        let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        let vote = MockVote::new(crate::testing::MockHash(0), crate::testing::MockHash(0), 0);
        let result = pipeline.process_vote(&vote).unwrap();

        // Simplified implementation returns Completed
        assert!(result.is_completed());
    }

    #[test]
    fn test_process_qc() {
        let config = PipelineConfig::default();
        let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        let result = pipeline.process_qc().unwrap();
        assert!(result.is_completed());
    }

    #[test]
    fn test_process_all_stages() {
        // We can't directly test process_at_stage, but we can verify it works
        // by verifying the pipeline completes through all stages
        let config = PipelineConfig::default();
        let mut pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        let block = Arc::new(MockBlock::genesis());
        let result = pipeline.process_block(&block).unwrap();

        assert!(result.is_completed());
        // All 4 stages should have been processed
        assert_eq!(pipeline.stats().stage_counts[0], 1); // Receiving
        assert_eq!(pipeline.stats().stage_counts[1], 1); // Validation
        assert_eq!(pipeline.stats().stage_counts[2], 1); // Processing
        assert_eq!(pipeline.stats().stage_counts[3], 1); // Commit
    }

    #[test]
    fn test_pipeline_builder_default() {
        let builder = PipelineBuilder::default();
        let pipeline = builder.build::<MockBlock, MockVote>();

        assert_eq!(pipeline.config().stage_timeout_ms, 5000);
        assert_eq!(pipeline.config().max_concurrent, 10);
    }

    #[test]
    fn test_pipeline_builder_all_methods() {
        let pipeline = PipelineBuilder::new()
            .stage_timeout_ms(2000)
            .max_concurrent(5)
            .validate_immediately(false)
            .parallel_processing(true)
            .build::<MockBlock, MockVote>();

        assert_eq!(pipeline.config().stage_timeout_ms, 2000);
        assert_eq!(pipeline.config().max_concurrent, 5);
        assert!(!pipeline.config().validate_immediately);
        assert!(pipeline.config().parallel_processing);
    }

    #[test]
    fn test_pipeline_current_stage_accessor() {
        let config = PipelineConfig::default();
        let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        assert_eq!(pipeline.current_stage(), Stage::Receiving);
    }

    #[test]
    fn test_pipeline_stats_accessor() {
        let config = PipelineConfig::default();
        let pipeline = Pipeline::<MockBlock, MockVote>::new(config);

        let stats = pipeline.stats();
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped, 0);
    }
}
