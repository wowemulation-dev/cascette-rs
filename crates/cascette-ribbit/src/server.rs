//! Server state management and orchestration.
//!
//! Manages shared state between HTTP and TCP servers, including the build database
//! and configuration.

use crate::config::{CdnConfig, ServerConfig};
use crate::database::BuildDatabase;
use crate::error::ServerError;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Shared application state for HTTP and TCP servers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Build database (loaded once at startup)
    database: Arc<BuildDatabase>,

    /// Default CDN configuration
    cdn_config: CdnConfig,

    /// Server start time (for metrics)
    started_at: SystemTime,
}

impl AppState {
    /// Create new application state from configuration.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if database cannot be loaded.
    pub fn new(config: &ServerConfig) -> Result<Self, ServerError> {
        tracing::info!("Loading build database from {:?}", config.builds);

        let database = BuildDatabase::from_file(&config.builds)?;

        tracing::info!(
            "Loaded {} builds for {} products",
            database.total_builds(),
            database.products().len()
        );

        let cdn_config = config.default_cdn_config();

        Ok(Self {
            database: Arc::new(database),
            cdn_config,
            started_at: SystemTime::now(),
        })
    }

    /// Get reference to build database.
    #[must_use]
    pub const fn database(&self) -> &Arc<BuildDatabase> {
        &self.database
    }

    /// Get default CDN configuration.
    #[must_use]
    pub const fn cdn_config(&self) -> &CdnConfig {
        &self.cdn_config
    }

    /// Get current sequence number (Unix timestamp).
    ///
    /// Used for BPSV sequence numbers to enable client-side caching.
    #[must_use]
    pub fn current_seqn(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Get server uptime in seconds.
    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.started_at)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Server orchestration.
pub struct Server {
    /// Shared application state
    #[allow(dead_code)] // Used in Phase 4 for HTTP/TCP handlers
    state: Arc<AppState>,
    /// Server configuration
    config: ServerConfig,
}

impl Server {
    /// Create new server with configuration.
    ///
    /// Loads the build database and prepares shared state.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if database cannot be loaded.
    pub fn new(config: ServerConfig) -> Result<Self, ServerError> {
        // Load application state (includes database)
        let state = AppState::new(&config)?;

        tracing::info!(
            "Server initialized with {} builds across {} products",
            state.database().total_builds(),
            state.database().products().len()
        );

        Ok(Self {
            state: Arc::new(state),
            config,
        })
    }

    /// Run the server (start HTTP and TCP listeners).
    ///
    /// This starts both HTTP and TCP servers concurrently.
    /// The server runs until interrupted or an error occurs.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if server binding fails.
    pub async fn run(self) -> Result<(), ServerError> {
        tracing::info!("Starting Cascette Ribbit Server");
        tracing::info!("HTTP server binding to: {}", self.config.http_bind);
        tracing::info!("TCP server binding to: {}", self.config.tcp_bind);

        if self.config.has_tls() {
            tracing::info!("TLS enabled with cert: {:?}", self.config.tls_cert);
        } else {
            tracing::info!("TLS disabled (HTTP only)");
        }

        // Start HTTP and TCP servers concurrently
        let http_state = self.state.clone();
        let tcp_state = self.state.clone();
        let http_bind = self.config.http_bind;
        let tcp_bind = self.config.tcp_bind;

        let http_server = tokio::spawn(async move {
            if let Err(e) = crate::http::start_server(http_bind, http_state).await {
                tracing::error!("HTTP server failed: {e}");
            }
        });

        let tcp_server = tokio::spawn(async move {
            if let Err(e) = crate::tcp::start_server(tcp_bind, tcp_state).await {
                tracing::error!("TCP server failed: {e}");
            }
        });

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await.map_err(|e| {
            ServerError::Shutdown(format!("Failed to listen for shutdown signal: {e}"))
        })?;

        tracing::info!("Shutdown signal received, stopping server");

        // Wait for servers to shutdown gracefully
        http_server.abort();
        tcp_server.abort();

        Ok(())
    }

    /// Get shared application state (for testing).
    #[cfg(test)]
    #[must_use]
    pub const fn state(&self) -> &Arc<AppState> {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_db_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"[{
            "id": 1,
            "product": "test_product",
            "version": "1.0.0.1",
            "build": "1",
            "build_config": "0123456789abcdef0123456789abcdef",
            "cdn_config": "fedcba9876543210fedcba9876543210",
            "product_config": null,
            "build_time": "2024-01-01T00:00:00+00:00",
            "encoding_ekey": "aaaabbbbccccddddeeeeffffaaaaffff",
            "root_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
            "install_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
            "download_ekey": "ddddeeeeffffaaaabbbbccccddddeeee"
        }]"#;
        file.write_all(json.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_app_state_creation() {
        let db_file = create_test_db_file();
        let config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: db_file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        };

        let state = AppState::new(&config).unwrap();
        assert_eq!(state.database().total_builds(), 1);
        assert_eq!(state.cdn_config().hosts, "cdn.test.com");
        assert_eq!(state.cdn_config().path, "test/path");
    }

    #[test]
    fn test_current_seqn() {
        let db_file = create_test_db_file();
        let config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: db_file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        };

        let state = AppState::new(&config).unwrap();
        let seqn = state.current_seqn();

        // Sequence number should be a reasonable Unix timestamp
        assert!(seqn > 1_700_000_000); // After 2023
        assert!(seqn < 2_000_000_000); // Before 2033
    }

    #[test]
    fn test_uptime() {
        let db_file = create_test_db_file();
        let config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: db_file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        };

        let state = AppState::new(&config).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(state.uptime_seconds() == 0); // Should be 0 or 1 second
    }
}
