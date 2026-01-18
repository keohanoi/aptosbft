// Simple HTTP metrics server implementation for dYdX AptosBFT consensus node
//
// This module provides a basic HTTP server for serving Prometheus metrics and health checks.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::metrics::{MetricsUpdater, gather_metrics};

/// Start the HTTP metrics server
pub async fn start_metrics_server(
    bind_address: String,
    metrics_updater: Arc<MetricsUpdater>,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(&bind_address).await
        .map_err(|e| format!("Failed to bind metrics server: {}", e))?;

    log::info!("Metrics HTTP server listening on {}", bind_address);

    // Accept connections loop
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                log::info!("Metrics server received shutdown signal");
                break;
            }
            result = timeout(Duration::from_secs(5), listener.accept()) => {
                match result {
                    Ok(Ok((stream, addr))) => {
                        log::debug!("Metrics connection from {}", addr);
                        let metrics = metrics_updater.clone();

                        // Handle connection in a task
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, metrics).await {
                                log::error!("Error handling metrics connection: {}", e);
                            }
                        });
                    }
                    Ok(Err(e)) => {
                        log::error!("Metrics server accept error: {}", e);
                    }
                    Err(_) => {
                        // Timeout is expected - continue loop
                        continue;
                    }
                }
            }
        }
    }

    log::info!("Metrics server shut down successfully");
    Ok(())
}

/// Handle a single HTTP connection
async fn handle_connection(
    stream: tokio::net::TcpStream,
    metrics_updater: Arc<MetricsUpdater>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, writer) = stream.into_split();

    // Wrap in BufReader for efficient line reading
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    // Read HTTP request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await
        .map_err(|e| format!("Failed to read HTTP request: {}", e))?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.is_empty() || parts.len() < 2 {
        return Err("Invalid HTTP request".into());
    }

    let method = parts[0];
    let path = parts[1];

    // Read and discard headers
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await
            .map_err(|e| format!("Failed to read header: {}", e))?;
        if line == "\r\n" || line == "\n" {
            break;
        }
    }

    // Build response
    let response = match (method, path) {
        ("GET", "/metrics") => {
            let state = metrics_updater.get_state();
            let metrics_text = gather_metrics(&state);
            format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}\n",
                metrics_text.len(),
                metrics_text
            )
        }
        ("GET", "/health") => {
            let state = metrics_updater.get_state();
            let health_json = serde_json::json!({
                "status": "healthy",
                "consensus_state": state.consensus_state.load(Ordering::Relaxed),
                "epoch": state.epoch.load(Ordering::Relaxed),
                "round": state.round.load(Ordering::Relaxed),
                "peers_connected": state.peers_connected.load(Ordering::Relaxed),
                "mempool_size": state.mempool_size.load(Ordering::Relaxed),
                "blocks_proposed": state.blocks_proposed.load(Ordering::Relaxed),
                "blocks_committed": state.blocks_committed.load(Ordering::Relaxed),
                "uptime": std::time::SystemTime::now()
            });
            let health_str = health_json.to_string();
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}\n",
                health_str.len(),
                health_str
            )
        }
        ("GET", "/") => {
            let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>dYdX AptosBFT Consensus Node</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        h1 { color: #333; }
        .status { color: #28a745; font-weight: bold; }
        .section { margin-top: 20px; }
        .metric { color: #666; }
    </style>
</head>
<body>
    <h1>dYdX AptosBFT Consensus Node</h1>
    <p class="status">Running</p>
    <div class="section">
        <p>Metrics:</p>
        <ul class="metric">
            <li><a href="/metrics">Prometheus Metrics</a></li>
            <li><a href="/health">Health Check</a></li>
        </ul>
    </div>
</body>
</html>
"#;
            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                html.len(),
                html
            )
        }
        _ => {
            format!("HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 38\r\n\r\n{{\"error\": \"Not Found\", \"paths\": [\"/metrics\", \"/health\", \"/\"]}}\n")
        }
    };

    // Send response
    writer.write_all(response.as_bytes()).await
        .map_err(|e| format!("Failed to write response: {}", e))?;
    writer.flush().await
        .map_err(|e| format!("Failed to flush response: {}", e))?;
    writer.shutdown().await
        .map_err(|e| format!("Failed to shutdown stream: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gather_metrics() {
        let updater = Arc::new(MetricsUpdater::new());
        updater.set_consensus_state(1);
        let state = updater.get_state();
        let metrics = gather_metrics(&state);
        assert!(metrics.contains("dydx_consensus_state 1"));
    }

    #[test]
    fn test_http_response_format() {
        let html = "test";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}\n", html.len(), html);
        assert!(response.contains("HTTP/1.1 200 OK"));
    }
}
