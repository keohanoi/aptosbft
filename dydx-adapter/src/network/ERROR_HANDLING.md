# Network Error Handling

This document describes the error handling mechanisms implemented in the AptosBFT P2P network layer.

## Overview

The TCP network implementation includes comprehensive error handling to ensure reliable validator-to-validator communication in production environments.

## Error Types

### NetworkError Variants

| Error Type | Description | Recovery Strategy |
|------------|-------------|-------------------|
| `Io` | Low-level I/O errors | Automatic retry with exponential backoff |
| `Serialization` | Message serialization failures | Log and skip message |
| `Deserialization` | Message deserialization failures | Log and skip message |
| `Connection { peer, error }` | Failed to connect to peer | Retry with backoff |
| `ConnectionTimeout { peer, timeout }` | Connection timed out | Retry with increased timeout |
| `PeerDisconnected(peer)` | Peer connection lost | Attempt reconnection |
| `PeerNotFound(peer)` | Peer not in configuration | Configuration error - requires intervention |
| `ChannelClosed` | Internal communication channel closed | Critical error - restart network |
| `SendFailed` | Failed to send message | Retry with backoff |
| `NotRunning` | Network not started | Start network first |
| `MaxRetriesExceeded { peer }` | Exceeded retry attempts | Mark peer as unhealthy |
| `UnhealthyPeer { peer, reason }` | Peer marked unhealthy | Wait for cooldown before retry |

## Retry Configuration

The `RetryConfig` structure controls retry behavior:

```rust
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 5)
    pub max_attempts: u32,

    /// Base delay between retries in milliseconds (default: 100ms)
    pub base_delay_ms: u64,

    /// Maximum delay between retries in milliseconds (default: 10000ms)
    pub max_delay_ms: u64,

    /// Exponential backoff multiplier (default: 2.0)
    pub backoff_multiplier: f64,

    /// Number of failures before marking peer unhealthy (default: 3)
    pub unhealthy_threshold: u32,

    /// How long to wait before retrying unhealthy peer in seconds (default: 30)
    pub unhealthy_retry_delay_secs: u64,
}
```

### Retry Strategy

The retry mechanism uses **exponential backoff**:

- Attempt 1: 100ms delay
- Attempt 2: 200ms delay
- Attempt 3: 400ms delay
- Attempt 4: 800ms delay
- Attempt 5: 1600ms delay (capped at max_delay_ms)

## Peer Health Tracking

The `PeerHealth` structure tracks each peer's connection quality:

```rust
struct PeerHealth {
    /// Number of consecutive connection failures
    consecutive_failures: u32,

    /// Last successful connection timestamp
    last_success: Option<std::time::Instant>,

    /// Whether this peer is marked as unhealthy (circuit breaker open)
    is_unhealthy: bool,

    /// When this peer can be retried (if unhealthy)
    retry_after: Option<std::time::Instant>,
}
```

### Circuit Breaker Pattern

The circuit breaker prevents wasting resources on failing peers:

1. **Closed State** (Normal): All connection attempts allowed
2. **Open State** (Unhealthy): After `unhealthy_threshold` consecutive failures
3. **Half-Open State**: After `unhealthy_retry_delay_secs` cooldown

## Error Scenarios and Handling

### 1. Connection Timeout

**Scenario**: Peer doesn't respond within timeout period

**Handling**:
- Log warning with peer ID and timeout duration
- Increment consecutive failures counter
- Retry with exponential backoff
- Mark unhealthy if threshold exceeded

```rust
NetworkError::ConnectionTimeout { peer: peer_id, timeout: 5 }
```

### 2. Peer Disconnection

**Scenario**: Established connection drops

**Handling**:
- Log warning with peer ID
- Attempt immediate reconnection
- If reconnection fails, fall back to retry logic
- Reset health on successful reconnection

### 3. Message Send Failure

**Scenario**: Unable to send message due to broken connection

**Handling**:
- Close and remove the connection
- Increment peer's consecutive failures
- Attempt to reconnect with retry logic
- Queue message for retry if important

### 4. Deserialization Error

**Scenario**: Received malformed message from peer

**Handling**:
- Log error with message details
- Skip the malformed message
- **Do not** increment failure counter (message error, not connection error)
- Continue processing other messages

### 5. Channel Closed

**Scenario**: Internal communication channel closed unexpectedly

**Handling**:
- **Critical error** - indicates internal bug
- Log error and stack trace
- Network cannot continue without restart
- Return error to caller for handling

## Monitoring and Observability

### Metrics to Track

1. **Connection Failures**: Track per-peer and total connection failure rates
2. **Retry Attempts**: Monitor retry count distribution
3. **Unhealthy Peers**: Track how many peers are marked unhealthy
4. **Reconnection Success Rate**: Percentage of successful reconnections
5. **Message Send Success Rate**: Track message delivery success/failure

### Log Levels

| Level | Usage |
|-------|-------|
| `ERROR` | Critical failures, channel closed, max retries exceeded |
| `WARN` | Connection failures, peer disconnections, unhealthy peers |
| `INFO` | Successful connections, peer state changes |
| `DEBUG` | Message sending/receiving, retry attempts |
| `TRACE` | Detailed byte-level I/O operations |

## Configuration Examples

### Aggressive Retry (for testing)

```rust
let retry_config = RetryConfig {
    max_attempts: 10,
    base_delay_ms: 50,
    max_delay_ms: 5000,
    backoff_multiplier: 1.5,
    unhealthy_threshold: 10,
    unhealthy_retry_delay_secs: 60,
};
```

### Conservative Retry (for production)

```rust
let retry_config = RetryConfig {
    max_attempts: 3,
    base_delay_ms: 200,
    max_delay_ms: 30000,
    backoff_multiplier: 3.0,
    unhealthy_threshold: 2,
    unhealthy_retry_delay_secs: 300,  // 5 minutes
};
```

## Best Practices

1. **Monitor Peer Health**: Regularly check `peer_health` to identify problematic validators
2. **Configure Appropriate Timeouts**: Balance between fast failure detection and allowing for network latency
3. **Set Sensible Thresholds**: Too strict = false positives; too lenient = slow failure detection
4. **Log Everything**: Error logs are critical for diagnosing network issues in production
5. **Circuit Breaker Cooldown**: Ensure cooldown is long enough to give failing peers time to recover

## Testing Error Scenarios

### Unit Tests

```rust
#[test]
fn test_connection_error_display() {
    let peer = DydxNodeId([1u8; 32]);
    let err = NetworkError::Connection {
        peer,
        error: "connection refused".to_string()
    };
    assert!(err.to_string().contains("Connection error to peer"));
}
```

### Integration Tests

Test error scenarios by:
1. Starting a validator with incorrect peer addresses
2. Killing a peer mid-connection and observing recovery
3. Throttling network to induce timeouts
4. Sending malformed messages to test deserialization error handling

## Future Enhancements

1. **Dynamic Retry Config**: Allow adjusting retry configuration at runtime without restart
2. **Peer Reputation**: Track long-term peer reliability and adjust retry behavior
3. **Alternative Routes**: Support multiple network paths to the same peer
4. **Metrics Export**: Expose peer health metrics to Prometheus for dashboards
5. **Adaptive Timeouts**: Adjust timeout based on observed network latency
