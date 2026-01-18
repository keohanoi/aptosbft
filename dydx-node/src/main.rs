// dYdX AptosBFT Consensus Node
//
// This is the main entry point for running the AptosBFT consensus node for dYdX v4.
// It initializes all necessary components and starts the consensus service.
//
// Usage:
//   cargo run --bin dydxaptosbft -- --config /path/to/config.toml
//
// Architecture:
//   1. Load configuration from file or environment variables
//   2. Load genesis file and parse validator set
//   3. Initialize StateComputer (gRPC client to Go application)
//   4. Create ConsensusAppService (bridges AptosBFT to Go app)
//   5. Initialize AptosBFT consensus engine
//   6. Start gRPC server for AptosBFT communication
//   7. Begin participating in consensus

mod metrics;
mod metrics_server;

use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::signal;
use tonic::transport::Server;
use tokio_util::sync::CancellationToken;

use dydx_adapter::{
    dydx_service::DydxConsensusAppService,
    genesis::CometBFTGenesis,
    network::{TcpNetwork, NetworkConfig, PeerConfig},
    types::{DydxNodeId, DydxProposerElection, DydxBlock, DydxVote},
};
use consensus_grpc_server::ConsensusServer;

use consensus_core::{
    consensus::{ConsensusConfig, ConsensusBuilder},
    consensus::runtime::{ConsensusRuntime, RuntimeConfig},
    liveness::pacemaker::PacemakerConfig,
    round_manager::RoundManagerConfig,
    pipeline::PipelineConfig,
    types::Epoch,
};

/// Default configuration file path
const DEFAULT_CONFIG_PATH: &str = "/etc/dydxaptosbft/config.toml";

/// Default gRPC server bind address
const DEFAULT_GRPC_BIND_ADDRESS: &str = "0.0.0.0:50052";

/// Default dYdX application endpoint
const DEFAULT_APP_ENDPOINT: &str = "http://localhost:50051";

/// Default log level
const DEFAULT_LOG_LEVEL: &str = "info";

/// Default P2P network listen address
const DEFAULT_P2P_LISTEN_ADDRESS: &str = "0.0.0.0:9001";

/// Validator key configuration
#[derive(Debug, Clone)]
pub struct ValidatorKey {
    /// Validator address (node ID)
    pub address: DydxNodeId,
    /// Private key bytes (for signing)
    pub private_key: Vec<u8>,
}

/// AptosBFT node configuration
#[derive(Debug, Clone)]
pub struct AptosBftConfig {
    /// Path to configuration file
    pub config_path: PathBuf,

    /// gRPC server bind address
    pub grpc_bind_address: String,

    /// dYdX application gRPC endpoint
    pub app_endpoint: String,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,

    /// Validator key file path (for signing)
    pub validator_key_path: Option<PathBuf>,

    /// Genesis file path
    pub genesis_path: Option<PathBuf>,

    /// Data directory for state
    pub data_dir: PathBuf,

    /// Enable metrics endpoint
    pub enable_metrics: bool,

    /// Metrics bind address
    pub metrics_bind_address: String,

    /// Initial epoch number
    pub initial_epoch: Epoch,

    /// Initial round number
    pub initial_round: u64,

    /// P2P network listen address (e.g., "0.0.0.0:9001")
    pub p2p_listen_address: String,

    /// Comma-separated list of peer addresses (e.g., "127.0.0.1:9002,127.0.0.1:9003")
    pub peers: String,
}

impl Default for AptosBftConfig {
    fn default() -> Self {
        Self {
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            grpc_bind_address: DEFAULT_GRPC_BIND_ADDRESS.to_string(),
            app_endpoint: DEFAULT_APP_ENDPOINT.to_string(),
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            validator_key_path: None,
            genesis_path: None,
            data_dir: PathBuf::from("/var/lib/dydxaptosbft"),
            enable_metrics: true,
            metrics_bind_address: "0.0.0.0:9090".to_string(),
            initial_epoch: 0,
            initial_round: 0,
            p2p_listen_address: "0.0.0.0:9001".to_string(),
            peers: String::new(),
        }
    }
}

impl AptosBftConfig {
    /// Load configuration from command-line arguments.
    pub fn from_args() -> Result<Self, Box<dyn std::error::Error>> {
        let args = std::env::args().collect::<Vec<_>>();
        let mut config = Self::default();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--config" | "-c" => {
                    if i + 1 < args.len() {
                        config.config_path = PathBuf::from(&args[i + 1]);
                        i += 2;
                    } else {
                        return Err("Missing value for --config".into());
                    }
                }
                "--grpc-bind-address" => {
                    if i + 1 < args.len() {
                        config.grpc_bind_address = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --grpc-bind-address".into());
                    }
                }
                "--app-endpoint" => {
                    if i + 1 < args.len() {
                        config.app_endpoint = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --app-endpoint".into());
                    }
                }
                "--validator-key" => {
                    if i + 1 < args.len() {
                        config.validator_key_path = Some(PathBuf::from(&args[i + 1]));
                        i += 2;
                    } else {
                        return Err("Missing value for --validator-key".into());
                    }
                }
                "--genesis" => {
                    if i + 1 < args.len() {
                        config.genesis_path = Some(PathBuf::from(&args[i + 1]));
                        i += 2;
                    } else {
                        return Err("Missing value for --genesis".into());
                    }
                }
                "--data-dir" => {
                    if i + 1 < args.len() {
                        config.data_dir = PathBuf::from(&args[i + 1]);
                        i += 2;
                    } else {
                        return Err("Missing value for --data-dir".into());
                    }
                }
                "--log-level" => {
                    if i + 1 < args.len() {
                        config.log_level = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --log-level".into());
                    }
                }
                "--no-metrics" => {
                    config.enable_metrics = false;
                    i += 1;
                }
                "--metrics-bind-address" => {
                    if i + 1 < args.len() {
                        config.metrics_bind_address = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --metrics-bind-address".into());
                    }
                }
                "--initial-epoch" => {
                    if i + 1 < args.len() {
                        config.initial_epoch = args[i + 1].parse()
                            .map_err(|_| format!("Invalid initial epoch: {}", args[i + 1]))?;
                        i += 2;
                    } else {
                        return Err("Missing value for --initial-epoch".into());
                    }
                }
                "--initial-round" => {
                    if i + 1 < args.len() {
                        config.initial_round = args[i + 1].parse()
                            .map_err(|_| format!("Invalid initial round: {}", args[i + 1]))?;
                        i += 2;
                    } else {
                        return Err("Missing value for --initial-round".into());
                    }
                }
                "--p2p-listen-address" => {
                    if i + 1 < args.len() {
                        config.p2p_listen_address = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --p2p-listen-address".into());
                    }
                }
                "--peers" => {
                    if i + 1 < args.len() {
                        config.peers = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Missing value for --peers".into());
                    }
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                flag => {
                    return Err(format!("Unknown flag or argument: {}", flag).into());
                }
            }
        }

        // Try to load from config file if it exists
        if config.config_path.exists() {
            config = config.merge_from_file()?;
        }

        Ok(config)
    }

    /// Merge configuration from file.
    fn merge_from_file(self) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Implement TOML config file parsing
        // For now, return the default config with CLI overrides
        log::info!("Config file exists but parsing not yet implemented, using CLI args");
        Ok(self)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate gRPC bind address format
        if self.grpc_bind_address.is_empty() {
            return Err("grpc_bind_address cannot be empty".into());
        }

        // Validate app endpoint format
        if !self.app_endpoint.starts_with("http://") && !self.app_endpoint.starts_with("https://") {
            return Err("app_endpoint must start with http:// or https://".into());
        }

        // Validate data directory
        if !self.data_dir.exists() {
            std::fs::create_dir_all(&self.data_dir)
                .map_err(|e| format!("Failed to create data directory {}: {}", self.data_dir.display(), e))?;
        }

        // Validate log level
        match self.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(format!("Invalid log level: {}", self.log_level).into()),
        }

        // Validate P2P listen address format (should be IP:port)
        if !self.p2p_listen_address.is_empty() {
            if self.p2p_listen_address.contains(':') {
                let parts: Vec<&str> = self.p2p_listen_address.split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid p2p_listen_address format: {}. Expected 'IP:port'", self.p2p_listen_address).into());
                }
                let ip = parts[0];
                let port = parts[1];
                // Basic IP validation - can be improved
                if !ip.is_empty() && (ip.parse::<u32>().is_ok() || ip == "localhost" || ip.contains('.')) {
                    // Valid IP
                } else {
                    return Err(format!("Invalid IP address: {}", ip).into());
                }
                if port.parse::<u16>().is_err() {
                    return Err(format!("Invalid port number: {}", port).into());
                }
            } else {
                return Err("p2p_listen_address must be in 'IP:port' format".into());
            }
        }

        // Validate peers format (comma-separated "IP:port" pairs)
        if !self.peers.is_empty() {
            for peer in self.peers.split(',') {
                let peer = peer.trim();
                if !peer.contains(':') {
                    return Err(format!("Invalid peer address format: {}. Expected 'IP:port'", peer).into());
                }
                let parts: Vec<&str> = peer.split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid peer address format: {}", peer).into());
                }
                let ip = parts[0];
                let port = parts[1];
                // Basic validation
                if !ip.is_empty() && (ip.parse::<u32>().is_ok() || ip == "localhost" || ip.contains('.')) {
                    // Valid IP
                } else {
                    return Err(format!("Invalid peer IP address: {}", ip).into());
                }
                if port.parse::<u16>().is_err() {
                    return Err(format!("Invalid peer port number: {}", port).into());
                }
            }
        }

        Ok(())
    }
}

/// Print usage information.
fn print_usage() {
    println!("dYdX AptosBFT Consensus Node v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("USAGE:");
    println!("    dydxaptosbft [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -c, --config <PATH>           Configuration file path [default: {}]", DEFAULT_CONFIG_PATH);
    println!("        --grpc-bind-address <ADDR> gRPC server bind address [default: {}]", DEFAULT_GRPC_BIND_ADDRESS);
    println!("        --app-endpoint <URL>       dYdX application gRPC endpoint [default: {}]", DEFAULT_APP_ENDPOINT);
    println!("        --validator-key <PATH>     Validator key file path");
    println!("        --genesis <PATH>           Genesis file path");
    println!("        --data-dir <PATH>          Data directory for state [default: /var/lib/dydxaptosbft]");
    println!("        --log-level <LEVEL>        Log level (trace|debug|info|warn|error) [default: info]");
    println!("        --no-metrics                Disable metrics endpoint");
    println!("        --metrics-bind-address <ADDR> Metrics bind address [default: 0.0.0.0:9090]");
    println!("        --initial-epoch <EPOCH>    Initial epoch number [default: 0]");
    println!("        --initial-round <ROUND>    Initial round number [default: 0]");
    println!("        --p2p-listen-address <ADDR> P2P network listen address [default: {}]", DEFAULT_P2P_LISTEN_ADDRESS);
    println!("        --peers <ADDRS>            Comma-separated peer addresses (e.g., \"127.0.0.1:9002,127.0.0.1:9003\")");
    println!("    -h, --help                    Print this help information");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    DYDX_GRPC_BIND_ADDRESS         gRPC server bind address");
    println!("    DYDX_APP_ENDPOINT              dYdX application gRPC endpoint");
    println!("    DYDX_LOG_LEVEL                 Log level");
    println!("    DYDX_DATA_DIR                  Data directory for state");
}

/// Initialize logging.
fn init_logging(config: &AptosBftConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = env_logger::Builder::new()
        .filter_level(
            match config.log_level.as_str() {
                "trace" => log::LevelFilter::Trace,
                "debug" => log::LevelFilter::Debug,
                "info" => log::LevelFilter::Info,
                "warn" => log::LevelFilter::Warn,
                "error" => log::LevelFilter::Error,
                _ => log::LevelFilter::Info,
            }
        )
        .write_style(env_logger::WriteStyle::Always)
        .build();

    // Initialize logger
    let _ = env_logger::init();
    Ok(())
}

/// Load the validator set from the genesis file.
fn load_validator_set_from_genesis(
    genesis_path: &PathBuf,
) -> Result<HashMap<DydxNodeId, u64>, Box<dyn std::error::Error>> {
    log::info!("Loading genesis from {}", genesis_path.display());

    let genesis = CometBFTGenesis::from_file(genesis_path)
        .map_err(|e| format!("Failed to load genesis: {}", e))?;

    let validator_set_info = genesis.to_validator_set_info()
        .map_err(|e| format!("Failed to parse validator set: {}", e))?;

    log::info!("Loaded {} validators from genesis", validator_set_info.validators.len());
    log::info!("Total voting power: {}", validator_set_info.total_voting_power);

    Ok(validator_set_info.to_hash_map())
}

/// Load the validator key from a file.
fn load_validator_key(
    key_path: &PathBuf,
) -> Result<ValidatorKey, Box<dyn std::error::Error>> {
    log::info!("Loading validator key from {}", key_path.display());

    // For now, use a simple JSON format for the validator key
    // Format: { "address": "...", "private_key": "..." }
    let content = std::fs::read_to_string(key_path)
        .map_err(|e| format!("Failed to read validator key file: {}", e))?;

    let key_data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse validator key JSON: {}", e))?;

    let address_str = key_data.get("address")
        .and_then(|v| v.as_str())
        .ok_or("Missing address field in validator key")?;

    let private_key_str = key_data.get("private_key")
        .and_then(|v| v.as_str())
        .ok_or("Missing private_key field in validator key")?;

    // Decode hex address
    let address_bytes = hex::decode(address_str)
        .map_err(|e| format!("Invalid hex address: {}", e))?;

    let address = DydxNodeId::from_slice(&address_bytes)?;

    // Decode hex private key
    let private_key = hex::decode(private_key_str)
        .map_err(|e| format!("Invalid hex private key: {}", e))?;

    log::info!("Loaded validator key for address: {}", address);

    Ok(ValidatorKey {
        address,
        private_key,
    })
}

/// Create the proposer election from validator set.
fn create_proposer_election(
    validators: HashMap<DydxNodeId, u64>,
) -> DydxProposerElection {
    let total_power: u64 = validators.values().sum();
    DydxProposerElection::new(validators, total_power)
}

/// Main entry point.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let mut config = AptosBftConfig::from_args()?;

    // Apply environment variable overrides
    if let Ok(addr) = std::env::var("DYDX_GRPC_BIND_ADDRESS") {
        config.grpc_bind_address = addr;
    }
    if let Ok(endpoint) = std::env::var("DYDX_APP_ENDPOINT") {
        config.app_endpoint = endpoint;
    }
    if let Ok(level) = std::env::var("DYDX_LOG_LEVEL") {
        config.log_level = level;
    }
    if let Ok(dir) = std::env::var("DYDX_DATA_DIR") {
        config.data_dir = PathBuf::from(dir);
    }

    // Initialize logging
    init_logging(&config)?;

    log::info!("dYdX AptosBFT Consensus Node v{} starting", env!("CARGO_PKG_VERSION"));
    log::info!("Configuration: {:?}", config);

    // Validate configuration
    config.validate()?;

    // Log key configuration values
    log::info!("gRPC server will bind to: {}", config.grpc_bind_address);
    log::info!("dYdX application endpoint: {}", config.app_endpoint);
    log::info!("Data directory: {}", config.data_dir.display());
    log::info!("Log level: {}", config.log_level);

    // Create the consensus application service
    log::info!("Connecting to dYdX application at {}...", config.app_endpoint);
    let service = match DydxConsensusAppService::new(config.app_endpoint.clone()).await {
        Ok(s) => {
            log::info!("Successfully connected to dYdX application");
            Arc::new(s)
        }
        Err(e) => {
            log::error!("Failed to connect to dYdX application: {}", e);
            return Err(format!("Failed to connect to dYdX application: {}", e).into());
        }
    };

    // Create the gRPC server
    let grpc_server = ConsensusServer::new(service.clone());
    let grpc_addr = config.grpc_bind_address.parse()?;

    // Start metrics server if enabled
    let metrics_updater = Arc::new(metrics::MetricsUpdater::new());
    let cancel_token = CancellationToken::new();
    #[allow(unused_variables)]
    let metrics_handle = if config.enable_metrics {
        log::info!("Metrics endpoint enabled at {}", config.metrics_bind_address);
        Some(tokio::spawn(metrics_server::start_metrics_server(
            config.metrics_bind_address.clone(),
            metrics_updater.clone(),
            cancel_token.clone(),
        )))
    } else {
        log::info!("Metrics endpoint disabled");
        None
    };

    // Start the gRPC server in the background
    log::info!("Starting gRPC server on {}...", config.grpc_bind_address);
    let grpc_server_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_server.into_server())
            .serve(grpc_addr)
            .await
    });

    log::info!("dYdX AptosBFT Consensus Node is running");
    log::info!("Ready to participate in consensus");
    log::info!("Press Ctrl+C to gracefully shut down");

    // Initialize AptosBFT consensus engine
    let (mut consensus_runtime, _self_validator_id) = if let Some(ref genesis_path) = config.genesis_path {
        // Load validator set from genesis
        let validators = load_validator_set_from_genesis(genesis_path)?;
        let proposer_election = create_proposer_election(validators.clone());

        // Load validator key if provided
        let self_validator_id = if let Some(ref key_path) = config.validator_key_path {
            let validator_key = load_validator_key(key_path)?;
            log::info!("Running as validator: {}", validator_key.address);
            Some(validator_key.address)
        } else {
            log::info!("Running in non-validating mode (no validator key provided)");
            None
        };

        // Create consensus configuration
        let consensus_config = ConsensusConfig::new(
            config.initial_round,
            config.initial_epoch,
            PacemakerConfig::default(),
            RoundManagerConfig::default(),
            PipelineConfig::default(),
        );

        // Build consensus instance with explicit types
        let consensus = ConsensusBuilder::<DydxBlock, DydxVote, DydxProposerElection>::new()
            .config(consensus_config)
            .proposer_election(proposer_election)
            .self_validator_id(self_validator_id.unwrap_or_else(|| {
                // Use a dummy address for non-validators
                DydxNodeId([0u8; 32])
            }))
            .build()
            .map_err(|e| format!("Failed to build consensus: {}", e))?;

        // Create P2P network if configured, otherwise use no network
        // Note: For now, we always use the no-network variant to avoid type complexity
        // TODO: Implement proper network integration with trait objects or type erasure

        if !config.p2p_listen_address.is_empty() {
            log::info!("P2P network configured at {} (network will be integrated in future update)", config.p2p_listen_address);
            if !config.peers.is_empty() {
                log::info!("Configured peers: {}", config.peers);
            }
        }

        // Create runtime without network (for now)
        let runtime = ConsensusRuntime::<DydxBlock, DydxVote, DydxProposerElection, ()>::new(
            consensus,
            RuntimeConfig::default(),
            None,
        );

        log::info!("AptosBFT consensus engine initialized");
        (Some(runtime), self_validator_id)
    } else {
        log::warn!("No genesis file provided - running without consensus engine");
        log::warn!("Consensus features will be disabled");
        (None, None)
    };

    // Start consensus runtime if initialized
    if let Some(ref mut runtime) = consensus_runtime {
        runtime.start()
            .map_err(|e| format!("Failed to start consensus runtime: {}", e))?;
        log::info!("Consensus runtime started");
    }

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Received Ctrl+C, initiating graceful shutdown...");
        }
        result = grpc_server_handle => {
            match result {
                Ok(Ok(())) => {
                    log::info!("gRPC server shut down normally");
                }
                Ok(Err(e)) => {
                    log::error!("gRPC server error: {}", e);
                    return Err(e.into());
                }
                Err(e) => {
                    log::error!("gRPC server task failed: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    // Shutdown consensus runtime if running
    if let Some(ref mut runtime) = consensus_runtime {
        log::info!("Shutting down consensus runtime...");
        if let Err(e) = runtime.shutdown() {
            log::error!("Error shutting down consensus runtime: {}", e);
        }
    }

    // Cancel metrics server if running
    if config.enable_metrics {
        cancel_token.cancel();
    }

    log::info!("Shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AptosBftConfig::default();
        assert_eq!(config.grpc_bind_address, DEFAULT_GRPC_BIND_ADDRESS);
        assert_eq!(config.app_endpoint, DEFAULT_APP_ENDPOINT);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(config.enable_metrics, true);
    }

    #[test]
    fn test_config_validate_valid() {
        let mut config = AptosBftConfig::default();
        // Use a temporary directory for testing
        config.data_dir = std::env::temp_dir();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_empty_grpc_address() {
        let mut config = AptosBftConfig::default();
        config.grpc_bind_address = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_invalid_app_endpoint() {
        let mut config = AptosBftConfig::default();
        config.app_endpoint = "invalid-url".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_invalid_log_level() {
        let mut config = AptosBftConfig::default();
        config.log_level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_valid_log_levels() {
        let mut config = AptosBftConfig::default();
        // Use a temporary directory for testing
        config.data_dir = std::env::temp_dir();
        for level in &["trace", "debug", "info", "warn", "error"] {
            config.log_level = level.to_string();
            assert!(config.validate().is_ok());
        }
    }
}
