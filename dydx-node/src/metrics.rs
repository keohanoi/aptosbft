// Metrics export module for dYdX AptosBFT consensus node
//
// This module provides Prometheus metrics for monitoring the consensus node,
// including consensus state, block processing, and network metrics.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics updater that can be shared across the application
#[derive(Clone)]
pub struct MetricsUpdater {
    /// Current metrics state
    state: Arc<MetricsState>,
}

/// Current metrics state
#[derive(Debug, Default)]
pub struct MetricsState {
    /// Consensus state
    pub consensus_state: AtomicU64,
    /// Current epoch
    pub epoch: AtomicU64,
    /// Current round
    pub round: AtomicU64,
    /// Number of connected peers
    pub peers_connected: AtomicU64,
    /// Mempool size
    pub mempool_size: AtomicU64,
    /// Blocks proposed
    pub blocks_proposed: AtomicU64,
    /// Blocks voted
    pub blocks_voted: AtomicU64,
    /// Blocks committed
    pub blocks_committed: AtomicU64,
    /// Blocks executed
    pub blocks_executed: AtomicU64,
    /// Votes received
    pub votes_received: AtomicU64,
    /// Votes sent
    pub votes_sent: AtomicU64,
    /// Quorum certificates formed
    pub quorum_certificates: AtomicU64,
    /// Rounds timed out
    pub rounds_timed_out: AtomicU64,
    /// Timeout certificates
    pub timeout_certificates: AtomicU64,
    /// Proposals received
    pub proposals_received: AtomicU64,
    /// Mempool transactions proposed
    pub mempool_txns_proposed: AtomicU64,
    /// Mempool transactions committed
    pub mempool_txns_committed: AtomicU64,
    /// gRPC requests
    pub grpc_requests: AtomicU64,
    /// gRPC errors
    pub grpc_errors: AtomicU64,
}

impl MetricsUpdater {
    /// Create a new metrics updater
    pub fn new() -> Self {
        Self {
            state: Arc::new(MetricsState::default()),
        }
    }

    /// Update consensus state
    pub fn set_consensus_state(&self, state: u64) {
        self.state.consensus_state.store(state, Ordering::Relaxed);
    }

    /// Update current epoch
    pub fn set_epoch(&self, epoch: u64) {
        self.state.epoch.store(epoch, Ordering::Relaxed);
    }

    /// Update current round
    pub fn set_round(&self, round: u64) {
        self.state.round.store(round, Ordering::Relaxed);
    }

    /// Update peer count
    pub fn set_peers_connected(&self, count: u64) {
        self.state.peers_connected.store(count, Ordering::Relaxed);
    }

    /// Update mempool size
    pub fn set_mempool_size(&self, size: u64) {
        self.state.mempool_size.store(size, Ordering::Relaxed);
    }

    /// Record a block proposed
    pub fn inc_blocks_proposed(&self) {
        self.state.blocks_proposed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a block voted
    pub fn inc_blocks_voted(&self) {
        self.state.blocks_voted.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a block committed
    pub fn inc_blocks_committed(&self) {
        self.state.blocks_committed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a block executed
    pub fn inc_blocks_executed(&self) {
        self.state.blocks_executed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a vote received
    pub fn inc_votes_received(&self) {
        self.state.votes_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a vote sent
    pub fn inc_votes_sent(&self) {
        self.state.votes_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a quorum certificate formed
    pub fn inc_quorum_certificates(&self) {
        self.state.quorum_certificates.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a round timeout
    pub fn inc_rounds_timed_out(&self) {
        self.state.rounds_timed_out.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a timeout certificate
    pub fn inc_timeout_certificates(&self) {
        self.state.timeout_certificates.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a proposal received
    pub fn inc_proposals_received(&self) {
        self.state.proposals_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record transactions proposed from mempool
    pub fn inc_mempool_txns_proposed(&self, count: u64) {
        self.state.mempool_txns_proposed.fetch_add(count, Ordering::Relaxed);
    }

    /// Record transactions committed
    pub fn inc_mempool_txns_committed(&self, count: u64) {
        self.state.mempool_txns_committed.fetch_add(count, Ordering::Relaxed);
    }

    /// Record gRPC request to application
    pub fn inc_grpc_requests(&self) {
        self.state.grpc_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record gRPC error from application
    pub fn inc_grpc_errors(&self) {
        self.state.grpc_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current metrics state
    pub fn get_state(&self) -> MetricsState {
        // Clone all AtomicU64 values
        MetricsState {
            consensus_state: AtomicU64::new(self.state.consensus_state.load(Ordering::Relaxed)),
            epoch: AtomicU64::new(self.state.epoch.load(Ordering::Relaxed)),
            round: AtomicU64::new(self.state.round.load(Ordering::Relaxed)),
            peers_connected: AtomicU64::new(self.state.peers_connected.load(Ordering::Relaxed)),
            mempool_size: AtomicU64::new(self.state.mempool_size.load(Ordering::Relaxed)),
            blocks_proposed: AtomicU64::new(self.state.blocks_proposed.load(Ordering::Relaxed)),
            blocks_voted: AtomicU64::new(self.state.blocks_voted.load(Ordering::Relaxed)),
            blocks_committed: AtomicU64::new(self.state.blocks_committed.load(Ordering::Relaxed)),
            blocks_executed: AtomicU64::new(self.state.blocks_executed.load(Ordering::Relaxed)),
            votes_received: AtomicU64::new(self.state.votes_received.load(Ordering::Relaxed)),
            votes_sent: AtomicU64::new(self.state.votes_sent.load(Ordering::Relaxed)),
            quorum_certificates: AtomicU64::new(self.state.quorum_certificates.load(Ordering::Relaxed)),
            rounds_timed_out: AtomicU64::new(self.state.rounds_timed_out.load(Ordering::Relaxed)),
            timeout_certificates: AtomicU64::new(self.state.timeout_certificates.load(Ordering::Relaxed)),
            proposals_received: AtomicU64::new(self.state.proposals_received.load(Ordering::Relaxed)),
            mempool_txns_proposed: AtomicU64::new(self.state.mempool_txns_proposed.load(Ordering::Relaxed)),
            mempool_txns_committed: AtomicU64::new(self.state.mempool_txns_committed.load(Ordering::Relaxed)),
            grpc_requests: AtomicU64::new(self.state.grpc_requests.load(Ordering::Relaxed)),
            grpc_errors: AtomicU64::new(self.state.grpc_errors.load(Ordering::Relaxed)),
        }
    }
}

impl Default for MetricsUpdater {
    fn default() -> Self {
        Self::new()
    }
}

/// Gather all metrics as Prometheus format
pub fn gather_metrics(state: &MetricsState) -> String {
    let mut output = String::new();

    // Helper macro to add a metric line
    macro_rules! push_metric {
        ($name:expr, $value:expr, $help:expr) => {
            output.push_str(&format!("# HELP {} {}\n", $name, $help));
            output.push_str(&format!("# TYPE {}\n", "counter"));
            output.push_str(&format!("{} {}\n\n", $name, $value));
        };
    }

    macro_rules! push_gauge {
        ($name:expr, $value:expr, $help:expr) => {
            output.push_str(&format!("# HELP {} {}\n", $name, $help));
            output.push_str(&format!("# TYPE {}\n", "gauge"));
            output.push_str(&format!("{} {}\n\n", $name, $value));
        };
    }

    // Consensus state
    push_gauge!(
        "dydx_consensus_state",
        state.consensus_state.load(Ordering::Relaxed),
        "Current consensus state (0=bootstrapping, 1=running, 2=catching_up, 3=stopping, 4=stopped)"
    );
    push_gauge!("dydx_consensus_current_epoch", state.epoch.load(Ordering::Relaxed), "Current consensus epoch");
    push_gauge!("dydx_consensus_current_round", state.round.load(Ordering::Relaxed), "Current consensus round");

    // Block processing
    push_metric!("dydx_consensus_blocks_proposed_total", state.blocks_proposed.load(Ordering::Relaxed), "Total number of blocks proposed");
    push_metric!("dydx_consensus_blocks_voted_total", state.blocks_voted.load(Ordering::Relaxed), "Total number of blocks voted on");
    push_metric!("dydx_consensus_blocks_committed_total", state.blocks_committed.load(Ordering::Relaxed), "Total number of blocks committed");
    push_metric!("dydx_consensus_blocks_executed_total", state.blocks_executed.load(Ordering::Relaxed), "Total number of blocks executed");

    // Votes and certificates
    push_metric!("dydx_consensus_votes_received_total", state.votes_received.load(Ordering::Relaxed), "Total number of votes received from peers");
    push_metric!("dydx_consensus_votes_sent_total", state.votes_sent.load(Ordering::Relaxed), "Total number of votes sent to peers");
    push_metric!("dydx_consensus_quorum_certificates_formed_total", state.quorum_certificates.load(Ordering::Relaxed), "Total number of quorum certificates formed");
    push_metric!("dydx_consensus_rounds_timed_out_total", state.rounds_timed_out.load(Ordering::Relaxed), "Total number of rounds that timed out");
    push_metric!("dydx_consensus_timeout_certificates_total", state.timeout_certificates.load(Ordering::Relaxed), "Total number of timeout certificates formed");

    // Network
    push_metric!("dydx_consensus_proposals_received_total", state.proposals_received.load(Ordering::Relaxed), "Total number of proposals received from peers");

    // Mempool
    push_gauge!("dydx_consensus_mempool_size", state.mempool_size.load(Ordering::Relaxed), "Number of transactions in mempool");
    push_metric!("dydx_consensus_mempool_transactions_proposed_total", state.mempool_txns_proposed.load(Ordering::Relaxed), "Total number of transactions proposed from mempool");
    push_metric!("dydx_consensus_mempool_transactions_committed_total", state.mempool_txns_committed.load(Ordering::Relaxed), "Total number of transactions committed from mempool");

    // gRPC application
    push_metric!("dydx_consensus_grpc_app_requests_total", state.grpc_requests.load(Ordering::Relaxed), "Total number of gRPC requests to application");
    push_metric!("dydx_consensus_grpc_app_errors_total", state.grpc_errors.load(Ordering::Relaxed), "Total number of gRPC errors from application");

    // Peer count
    push_gauge!("dydx_consensus_peers_connected", state.peers_connected.load(Ordering::Relaxed), "Number of connected peers");

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_updater_default() {
        let updater = MetricsUpdater::new();
        let state = updater.get_state();
        assert_eq!(state.consensus_state.load(Ordering::Relaxed), 0);
        assert_eq!(state.epoch.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_updater_set_state() {
        let updater = MetricsUpdater::new();
        updater.set_consensus_state(1);
        updater.set_epoch(5);
        updater.set_round(10);

        let state = updater.get_state();
        assert_eq!(state.consensus_state.load(Ordering::Relaxed), 1);
        assert_eq!(state.epoch.load(Ordering::Relaxed), 5);
        assert_eq!(state.round.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_metrics_increment() {
        let updater = MetricsUpdater::new();
        updater.inc_blocks_proposed();
        updater.inc_blocks_voted();
        updater.inc_votes_received();

        let state = updater.get_state();
        assert!(state.blocks_proposed.load(Ordering::Relaxed) >= 1);
        assert!(state.blocks_voted.load(Ordering::Relaxed) >= 1);
        assert!(state.votes_received.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_gather_metrics() {
        let updater = MetricsUpdater::new();
        updater.set_consensus_state(1);
        updater.set_epoch(5);
        updater.inc_blocks_proposed();

        let state = updater.get_state();
        let metrics = gather_metrics(&state);
        assert!(metrics.contains("dydx_consensus"));
        assert!(metrics.contains("dydx_consensus_state 1"));
    }
}
