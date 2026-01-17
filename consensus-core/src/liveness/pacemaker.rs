// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Pacemaker for round timeout and progression.
//!
//! This module implements the pacemaker component that manages round timeouts
//! and ensures the consensus system makes progress even when some validators
//! are slow or unresponsive.

use consensus_traits::core::Hash as HashTrait;
use std::{
    fmt::{Debug, Display},
    sync::Arc,
    time::Duration,
};
use anyhow::Result;

/// Configuration for the pacemaker.
#[derive(Clone, Debug)]
pub struct PacemakerConfig {
    /// Base duration for a round (when we have a recent QC)
    pub base_duration_ms: u64,
    /// Multiplier for exponential backoff
    pub exponent_base: f64,
    /// Maximum exponent for exponential backoff
    pub max_exponent: usize,
    /// Initial round number
    pub initial_round: u64,
}

impl Default for PacemakerConfig {
    fn default() -> Self {
        Self {
            base_duration_ms: 1000, // 1 second
            exponent_base: 2.0,
            max_exponent: 8,
            initial_round: 1,
        }
    }
}

impl PacemakerConfig {
    /// Create a new pacemaker configuration.
    pub fn new(
        base_duration_ms: u64,
        exponent_base: f64,
        max_exponent: usize,
        initial_round: u64,
    ) -> Self {
        Self {
            base_duration_ms,
            exponent_base,
            max_exponent,
            initial_round,
        }
    }

    /// Get the base duration as a Duration.
    pub fn base_duration(&self) -> Duration {
        Duration::from_millis(self.base_duration_ms)
    }
}

/// Reason for starting a new round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewRoundReason {
    /// Started because we received a quorum certificate for the previous round
    QuorumCertificate,
    /// Started because the previous round timed out
    Timeout { rounds_since_ordered: usize },
}

impl Display for NewRoundReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NewRoundReason::QuorumCertificate => write!(f, "QCReady"),
            NewRoundReason::Timeout { rounds_since_ordered } => {
                write!(f, "Timeout(rounds_since_ordered={})", rounds_since_ordered)
            }
        }
    }
}

/// Event produced when a new round starts.
#[derive(Clone, Debug)]
pub struct NewRoundEvent {
    /// The new round number
    pub round: u64,
    /// Reason for starting this round
    pub reason: NewRoundReason,
    /// Timeout duration for this round
    pub timeout_duration: Duration,
}

impl NewRoundEvent {
    /// Create a new round event.
    pub fn new(round: u64, reason: NewRoundReason, timeout_duration: Duration) -> Self {
        Self {
            round,
            reason,
            timeout_duration,
        }
    }
}

/// Strategy for calculating round time intervals.
///
/// This trait allows different strategies for determining how long
/// each round should last, typically increasing the duration when
/// progress is stalled.
pub trait RoundIntervalStrategy: Send + Sync {
    /// Get the duration for a round.
    ///
    /// Parameters:
    /// - `round_index_after_ordered`: Number of rounds since the last
    ///   successfully ordered round (0 means the round immediately
    ///   after an ordered round).
    fn get_round_duration(&self, round_index_after_ordered: usize) -> Duration;
}

/// Exponential backoff interval strategy.
///
/// Round durations increase exponentially based on the number of rounds
/// since the last ordered round. Formula: base * multiplier^min(index, max_exponent)
#[derive(Clone)]
pub struct ExponentialIntervalStrategy {
    /// Base duration in milliseconds
    base_ms: u64,
    /// Multiplier for each round without progress
    exponent_base: f64,
    /// Maximum exponent (caps the backoff)
    max_exponent: usize,
}

impl ExponentialIntervalStrategy {
    /// Create a new exponential interval strategy.
    pub fn new(base: Duration, exponent_base: f64, max_exponent: usize) -> Self {
        assert!(
            max_exponent < 32,
            "max_exponent should be < 32"
        );
        assert!(
            exponent_base.powf(max_exponent as f64).ceil() < f64::from(u32::MAX),
            "Maximum interval multiplier should be less than u32::MAX"
        );

        Self {
            base_ms: base.as_millis() as u64,
            exponent_base,
            max_exponent,
        }
    }

    /// Create a fixed interval strategy (useful for testing).
    #[cfg(any(test, feature = "fuzzing"))]
    pub fn fixed(duration: Duration) -> Self {
        Self::new(duration, 1.0, 0)
    }
}

impl RoundIntervalStrategy for ExponentialIntervalStrategy {
    fn get_round_duration(&self, round_index_after_ordered: usize) -> Duration {
        let pow = (round_index_after_ordered as u32).min(self.max_exponent as u32);
        let multiplier = self.exponent_base.powf(f64::from(pow));
        let duration_ms = ((self.base_ms as f64) * multiplier).ceil() as u64;
        Duration::from_millis(duration_ms)
    }
}

/// Pacemaker for managing round timeouts and progression.
///
/// The pacemaker ensures the consensus system makes progress by:
/// 1. Tracking the current round
/// 2. Setting timeouts for each round
/// 3. Starting new rounds when timeouts occur
/// 4. Calculating appropriate timeout durations
///
/// ## Type Parameters
///
/// - `R`: Round type (must be incrementable)
pub struct Pacemaker<S>
where
    S: RoundIntervalStrategy,
{
    /// Strategy for calculating round intervals
    interval_strategy: S,

    /// Highest ordered round (last round that successfully ordered a block)
    highest_ordered_round: u64,

    /// Current round
    current_round: u64,

    /// Configuration
    config: PacemakerConfig,
}

impl<S> Pacemaker<S>
where
    S: RoundIntervalStrategy,
{
    /// Create a new pacemaker.
    pub fn new(interval_strategy: S, config: PacemakerConfig) -> Self {
        let initial_round = config.initial_round;

        Self {
            interval_strategy,
            highest_ordered_round: initial_round.saturating_sub(1),
            current_round: initial_round,
            config,
        }
    }

    /// Get the current round.
    pub fn current_round(&self) -> u64 {
        self.current_round
    }

    /// Get the highest ordered round.
    pub fn highest_ordered_round(&self) -> u64 {
        self.highest_ordered_round
    }

    /// Check if we can enter a new round.
    ///
    /// Returns true if the given round is greater than the current round.
    pub fn can_enter_round(&self, round: u64) -> bool {
        round > self.current_round
    }

    /// Enter a new round.
    ///
    /// This advances the pacemaker to the specified round and returns
    /// a timeout duration for the round.
    ///
    /// ## Errors
    ///
    /// Returns an error if the round is not greater than the current round.
    pub fn enter_new_round(&mut self, round: u64) -> Result<NewRoundEvent> {
        if !self.can_enter_round(round) {
            anyhow::bail!(
                "Cannot enter round {}: current round is {}",
                round,
                self.current_round
            );
        }

        let rounds_since_ordered = (round.saturating_sub(self.highest_ordered_round + 1)) as usize;
        let reason = if rounds_since_ordered == 0 {
            NewRoundReason::QuorumCertificate
        } else {
            NewRoundReason::Timeout { rounds_since_ordered }
        };

        let timeout_duration = self
            .interval_strategy
            .get_round_duration(rounds_since_ordered);

        self.current_round = round;

        Ok(NewRoundEvent::new(round, reason, timeout_duration))
    }

    /// Record that a round was successfully ordered.
    ///
    /// This updates the highest ordered round and may affect future
    /// timeout calculations.
    pub fn record_ordered_round(&mut self, round: u64) -> Result<()> {
        if round <= self.highest_ordered_round {
            anyhow::bail!(
                "Round {} is not greater than highest ordered round {}",
                round,
                self.highest_ordered_round
            );
        }

        self.highest_ordered_round = round;
        Ok(())
    }

    /// Get the timeout duration for the current round.
    pub fn current_round_timeout(&self) -> Duration {
        let rounds_since_ordered = self
            .current_round
            .saturating_sub(self.highest_ordered_round + 1) as usize;

        self.interval_strategy
            .get_round_duration(rounds_since_ordered)
    }

    /// Check if a timeout should occur based on elapsed time.
    ///
    /// In production, this would be used with a timer service.
    /// For now, this is a placeholder.
    pub fn check_timeout(&self, _elapsed: Duration) -> bool {
        // Simplified: always return false
        // In production, this would compare elapsed time to timeout
        false
    }

    /// Reset the pacemaker to a specific round.
    ///
    /// This is useful for recovery or testing.
    pub fn reset(&mut self, round: u64) {
        self.current_round = round;
        self.highest_ordered_round = round.saturating_sub(1);
    }

    /// Get the interval strategy (useful for testing).
    #[cfg(test)]
    pub fn interval_strategy(&self) -> &S {
        &self.interval_strategy
    }
}

/// Builder for creating a pacemaker.
pub struct PacemakerBuilder {
    config: PacemakerConfig,
}

impl Default for PacemakerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PacemakerBuilder {
    /// Create a new pacemaker builder.
    pub fn new() -> Self {
        Self {
            config: PacemakerConfig::default(),
        }
    }

    /// Set the base duration.
    pub fn base_duration_ms(mut self, duration_ms: u64) -> Self {
        self.config.base_duration_ms = duration_ms;
        self
    }

    /// Set the exponent base.
    pub fn exponent_base(mut self, base: f64) -> Self {
        self.config.exponent_base = base;
        self
    }

    /// Set the max exponent.
    pub fn max_exponent(mut self, max: usize) -> Self {
        self.config.max_exponent = max;
        self
    }

    /// Set the initial round.
    pub fn initial_round(mut self, round: u64) -> Self {
        self.config.initial_round = round;
        self
    }

    /// Build the pacemaker with an exponential interval strategy.
    pub fn build_exponential(self) -> Pacemaker<ExponentialIntervalStrategy> {
        let strategy = ExponentialIntervalStrategy::new(
            self.config.base_duration(),
            self.config.exponent_base,
            self.config.max_exponent,
        );

        Pacemaker::new(strategy, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pacemaker() -> Pacemaker<ExponentialIntervalStrategy> {
        let strategy = ExponentialIntervalStrategy::fixed(Duration::from_millis(100));
        let config = PacemakerConfig {
            base_duration_ms: 100,
            exponent_base: 2.0,
            max_exponent: 4,
            initial_round: 1,
        };

        Pacemaker::new(strategy, config)
    }

    #[test]
    fn test_pacemaker_creation() {
        let pacemaker = create_test_pacemaker();

        assert_eq!(pacemaker.current_round(), 1);
        assert_eq!(pacemaker.highest_ordered_round(), 0);
    }

    #[test]
    fn test_enter_new_round() {
        let mut pacemaker = create_test_pacemaker();

        let event = pacemaker.enter_new_round(2).unwrap();

        assert_eq!(event.round, 2);
        assert_eq!(pacemaker.current_round(), 2);
        assert_eq!(event.timeout_duration, Duration::from_millis(100));
    }

    #[test]
    fn test_cannot_enter_past_round() {
        let mut pacemaker = create_test_pacemaker();

        let result = pacemaker.enter_new_round(1);

        assert!(result.is_err());
    }

    #[test]
    fn test_record_ordered_round() {
        let mut pacemaker = create_test_pacemaker();

        pacemaker.record_ordered_round(1).unwrap();

        assert_eq!(pacemaker.highest_ordered_round(), 1);
    }

    #[test]
    fn test_exponential_interval() {
        let strategy = ExponentialIntervalStrategy::new(
            Duration::from_millis(100),
            2.0,
            4,
        );

        assert_eq!(strategy.get_round_duration(0), Duration::from_millis(100));
        assert_eq!(strategy.get_round_duration(1), Duration::from_millis(200));
        assert_eq!(strategy.get_round_duration(2), Duration::from_millis(400));
        assert_eq!(strategy.get_round_duration(3), Duration::from_millis(800));
    }

    #[test]
    fn test_exponential_interval_capped() {
        let strategy = ExponentialIntervalStrategy::new(
            Duration::from_millis(100),
            2.0,
            2, // Max exponent of 2
        );

        // Should be capped at max_exponent
        assert_eq!(strategy.get_round_duration(10), Duration::from_millis(400));
    }

    #[test]
    fn test_new_round_reason_display() {
        let reason = NewRoundReason::QuorumCertificate;
        assert_eq!(format!("{}", reason), "QCReady");

        let reason = NewRoundReason::Timeout { rounds_since_ordered: 5 };
        assert_eq!(format!("{}", reason), "Timeout(rounds_since_ordered=5)");
    }

    #[test]
    fn test_pacemaker_builder() {
        let pacemaker = PacemakerBuilder::new()
            .base_duration_ms(500)
            .exponent_base(3.0)
            .max_exponent(5)
            .initial_round(2)
            .build_exponential();

        assert_eq!(pacemaker.current_round(), 2);
        assert_eq!(pacemaker.highest_ordered_round(), 1);
    }

    #[test]
    fn test_pacemaker_config_default() {
        let config = PacemakerConfig::default();
        assert_eq!(config.base_duration_ms, 1000);
        assert_eq!(config.exponent_base, 2.0);
        assert_eq!(config.max_exponent, 8);
        assert_eq!(config.initial_round, 1);
    }

    #[test]
    fn test_pacemaker_config_new() {
        let config = PacemakerConfig::new(2000, 3.0, 10, 5);
        assert_eq!(config.base_duration_ms, 2000);
        assert_eq!(config.exponent_base, 3.0);
        assert_eq!(config.max_exponent, 10);
        assert_eq!(config.initial_round, 5);
    }

    #[test]
    fn test_pacemaker_config_base_duration() {
        let config = PacemakerConfig {
            base_duration_ms: 5000,
            exponent_base: 2.0,
            max_exponent: 8,
            initial_round: 1,
        };
        assert_eq!(config.base_duration(), Duration::from_millis(5000));
    }

    #[test]
    fn test_exponential_interval_fixed() {
        let strategy = ExponentialIntervalStrategy::fixed(Duration::from_millis(200));
        assert_eq!(strategy.get_round_duration(0), Duration::from_millis(200));
        assert_eq!(strategy.get_round_duration(100), Duration::from_millis(200));
    }

    #[test]
    fn test_pacemaker_can_enter_round() {
        let pacemaker = create_test_pacemaker();

        // Can enter higher round
        assert!(pacemaker.can_enter_round(2));
        assert!(pacemaker.can_enter_round(100));

        // Cannot enter current or lower round
        assert!(!pacemaker.can_enter_round(1));
        assert!(!pacemaker.can_enter_round(0));
    }

    #[test]
    fn test_record_ordered_round_error() {
        let mut pacemaker = create_test_pacemaker();

        // First record succeeds
        pacemaker.record_ordered_round(5).unwrap();

        // Recording lower or equal round fails
        let result = pacemaker.record_ordered_round(3);
        assert!(result.is_err());

        let result2 = pacemaker.record_ordered_round(5);
        assert!(result2.is_err());
    }

    #[test]
    fn test_current_round_timeout() {
        let mut pacemaker = create_test_pacemaker();

        // Initial round
        assert_eq!(pacemaker.current_round_timeout(), Duration::from_millis(100));

        // After advancing past highest ordered round
        pacemaker.enter_new_round(10).unwrap();
        // Should get the same fixed duration (100ms) regardless of rounds since ordered
        assert_eq!(pacemaker.current_round_timeout(), Duration::from_millis(100));
    }

    #[test]
    fn test_check_timeout() {
        let pacemaker = create_test_pacemaker();

        // Simplified implementation always returns false
        assert!(!pacemaker.check_timeout(Duration::from_secs(100)));
        assert!(!pacemaker.check_timeout(Duration::from_millis(0)));
    }

    #[test]
    fn test_pacemaker_reset() {
        let mut pacemaker = create_test_pacemaker();

        // Advance to round 10
        pacemaker.enter_new_round(10).unwrap();
        pacemaker.record_ordered_round(5).unwrap();

        // Reset to round 1
        pacemaker.reset(1);
        assert_eq!(pacemaker.current_round(), 1);
        assert_eq!(pacemaker.highest_ordered_round(), 0);
    }

    #[test]
    fn test_pacemaker_reset_round_zero() {
        let mut pacemaker = create_test_pacemaker();

        pacemaker.reset(0);
        assert_eq!(pacemaker.current_round(), 0);
        assert_eq!(pacemaker.highest_ordered_round(), 0);
    }

    #[test]
    fn test_pacemaker_interval_strategy() {
        let pacemaker = create_test_pacemaker();
        let strategy = pacemaker.interval_strategy();

        assert_eq!(strategy.get_round_duration(0), Duration::from_millis(100));
    }

    #[test]
    fn test_exponential_interval_assertion_max_exponent() {
        // Should panic if max_exponent >= 32
        let result = std::panic::catch_unwind(|| {
            ExponentialIntervalStrategy::new(
                Duration::from_millis(100),
                2.0,
                32, // Too large
            );
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_exponential_interval_new_round_reason_qc() {
        let reason = NewRoundReason::QuorumCertificate;
        assert!(matches!(reason, NewRoundReason::QuorumCertificate));
    }

    #[test]
    fn test_new_round_event_new() {
        let event = NewRoundEvent::new(
            5,
            NewRoundReason::Timeout { rounds_since_ordered: 2 },
            Duration::from_millis(500),
        );

        assert_eq!(event.round, 5);
        assert_eq!(event.timeout_duration, Duration::from_millis(500));
    }

    #[test]
    fn test_enter_new_round_with_qc_reason() {
        let mut pacemaker = create_test_pacemaker();

        // Record an ordered round first
        pacemaker.record_ordered_round(1).unwrap();

        // Entering next round should have QC reason
        let event = pacemaker.enter_new_round(2).unwrap();
        assert!(matches!(event.reason, NewRoundReason::QuorumCertificate));
    }

    #[test]
    fn test_enter_new_round_with_timeout_reason() {
        let mut pacemaker = create_test_pacemaker();

        // Entering round 10 (way past highest ordered round 0) should have timeout reason
        let event = pacemaker.enter_new_round(10).unwrap();
        assert!(matches!(event.reason, NewRoundReason::Timeout { .. }));

        if let NewRoundReason::Timeout { rounds_since_ordered } = event.reason {
            assert_eq!(rounds_since_ordered, 9); // 10 - (0 + 1) = 9
        }
    }

    #[test]
    fn test_pacemaker_builder_default() {
        let builder = PacemakerBuilder::default();
        let pacemaker = builder.build_exponential();

        assert_eq!(pacemaker.current_round(), 1);
        assert_eq!(pacemaker.highest_ordered_round(), 0);
    }

    #[test]
    fn test_pacemaker_builder_chaining() {
        let pacemaker = PacemakerBuilder::default()
            .base_duration_ms(100)
            .exponent_base(1.5)
            .max_exponent(3)
            .build_exponential();

        assert_eq!(pacemaker.current_round(), 1);
    }

    #[test]
    fn test_exponential_interval_large_multiplier() {
        let strategy = ExponentialIntervalStrategy::new(
            Duration::from_millis(1),
            2.0,
            10,
        );

        // Test with large exponent
        let duration = strategy.get_round_duration(5);
        assert!(duration.as_millis() > 0);
    }

    #[test]
    fn test_pacemaker_initial_round_zero() {
        let strategy = ExponentialIntervalStrategy::fixed(Duration::from_millis(100));
        let config = PacemakerConfig {
            base_duration_ms: 100,
            exponent_base: 2.0,
            max_exponent: 4,
            initial_round: 0,
        };

        let pacemaker = Pacemaker::new(strategy, config);
        assert_eq!(pacemaker.current_round(), 0);
        assert_eq!(pacemaker.highest_ordered_round(), 0); // 0.saturating_sub(1) = 0
    }
}
