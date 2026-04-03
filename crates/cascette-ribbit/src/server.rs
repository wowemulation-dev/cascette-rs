//! Server state management and orchestration.
//!
//! Manages shared state between HTTP and TCP servers, including the build database,
//! per-product sequence number counters, and hot-reload watcher.

use crate::config::{CdnConfig, ServerConfig};
use crate::database::BuildDatabase;
use crate::error::ServerError;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Shared application state for HTTP and TCP servers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Build database (hot-reloadable)
    database: Arc<RwLock<Arc<BuildDatabase>>>,

    /// Default CDN configuration
    cdn_config: CdnConfig,

    /// Per-product monotonically increasing sequence number counters.
    ///
    /// Each product gets its own counter that increments whenever the database
    /// is reloaded with new data for that product. This matches Blizzard's
    /// per-product seqn model rather than returning a Unix timestamp.
    seqn_counters: Arc<DashMap<String, Arc<AtomicU64>>>,

    /// Server start time (for metrics)
    started_at: SystemTime,

    /// Path to the builds.json file (for hot reload)
    builds_path: PathBuf,
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

        // Seed per-product counters from current time so seqn is always
        // greater than any counter the client may have cached from a
        // previous server run.
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let seqn_counters: Arc<DashMap<String, Arc<AtomicU64>>> = Arc::new(DashMap::new());
        for product in database.products() {
            seqn_counters.insert(product.to_string(), Arc::new(AtomicU64::new(now_secs)));
        }

        let cdn_config = config.default_cdn_config();

        Ok(Self {
            database: Arc::new(RwLock::new(Arc::new(database))),
            cdn_config,
            seqn_counters,
            started_at: SystemTime::now(),
            builds_path: config.builds.clone(),
        })
    }

    /// Get a snapshot of the build database.
    ///
    /// Returns a cloned `Arc` so callers hold a stable reference even if
    /// the database is reloaded concurrently.
    pub async fn database(&self) -> Arc<BuildDatabase> {
        Arc::clone(&*self.database.read().await)
    }

    /// Get default CDN configuration.
    #[must_use]
    pub const fn cdn_config(&self) -> &CdnConfig {
        &self.cdn_config
    }

    /// Get the current sequence number for a product.
    ///
    /// Returns the per-product counter, which increments on each hot reload.
    /// Falls back to a Unix timestamp for products not yet in the counter map
    /// (e.g., a product added mid-run before the next reload).
    #[must_use]
    pub fn current_seqn(&self, product: &str) -> u64 {
        if let Some(counter) = self.seqn_counters.get(product) {
            counter.load(Ordering::Relaxed)
        } else {
            // Fallback: new product not yet in counter map
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        }
    }

    /// Get the path to the builds.json file (used by the hot-reload watcher).
    #[must_use]
    pub fn builds_path(&self) -> &std::path::Path {
        &self.builds_path
    }

    /// Get server uptime in seconds.
    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.started_at)
            .unwrap_or_default()
            .as_secs()
    }

    /// Reload the build database from disk.
    ///
    /// Increments per-product seqn counters for any product whose builds
    /// have changed. New products get a counter seeded at current time.
    ///
    /// Called by the file watcher task when `builds.json` changes.
    pub async fn reload_database(&self) {
        match BuildDatabase::from_file(&self.builds_path) {
            Ok(new_db) => {
                let now_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Bump seqn for every known product and add new ones.
                for product in new_db.products() {
                    self.seqn_counters
                        .entry(product.to_string())
                        .and_modify(|c| {
                            // Ensure the new value is strictly greater than the
                            // current one even if the clock didn't advance.
                            let prev = c.load(Ordering::Relaxed);
                            c.store(now_secs.max(prev + 1), Ordering::Relaxed);
                        })
                        .or_insert_with(|| Arc::new(AtomicU64::new(now_secs)));
                }

                let total = new_db.total_builds();
                let products = new_db.products().len();
                *self.database.write().await = Arc::new(new_db);
                tracing::info!("Database reloaded: {total} builds across {products} products");
            }
            Err(e) => {
                tracing::warn!("Database reload failed, keeping previous data: {e}");
            }
        }
    }
}

/// Server orchestration.
pub struct Server {
    /// Shared application state
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
        let state = AppState::new(&config)?;

        tracing::info!(
            "Server initialized with {} builds across {} products",
            state
                .database
                .try_read()
                .map(|db| db.total_builds())
                .unwrap_or(0),
            state
                .database
                .try_read()
                .map(|db| db.products().len())
                .unwrap_or(0),
        );

        Ok(Self {
            state: Arc::new(state),
            config,
        })
    }

    /// Run the server (start HTTP and TCP listeners).
    ///
    /// Starts HTTP (plain or TLS), TCP Ribbit, and the builds.json hot-reload
    /// watcher concurrently. Runs until interrupted.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if server binding or TLS setup fails.
    pub async fn run(self) -> Result<(), ServerError> {
        tracing::info!("Starting Cascette Ribbit Server");
        tracing::info!("HTTP server binding to: {}", self.config.http_bind);
        tracing::info!("TCP server binding to: {}", self.config.tcp_bind);

        let http_state = self.state.clone();
        let tcp_state = self.state.clone();
        let reload_state = self.state.clone();
        let http_bind = self.config.http_bind;
        let tcp_bind = self.config.tcp_bind;

        // HTTP / HTTPS server
        let http_server = if self.config.has_tls() {
            #[cfg(feature = "tls")]
            {
                let tls_cert = self
                    .config
                    .tls_cert
                    .clone()
                    .expect("tls_cert present when has_tls()");
                let tls_key = self
                    .config
                    .tls_key
                    .clone()
                    .expect("tls_key present when has_tls()");
                tracing::info!("TLS enabled with cert: {tls_cert:?}");
                tokio::spawn(async move {
                    if let Err(e) =
                        crate::http::start_tls_server(http_bind, http_state, &tls_cert, &tls_key)
                            .await
                    {
                        tracing::error!("HTTPS server failed: {e}");
                    }
                })
            }
            #[cfg(not(feature = "tls"))]
            {
                tracing::warn!(
                    "TLS cert/key configured but the 'tls' feature is not enabled; falling back to plain HTTP"
                );
                tokio::spawn(async move {
                    if let Err(e) = crate::http::start_server(http_bind, http_state).await {
                        tracing::error!("HTTP server failed: {e}");
                    }
                })
            }
        } else {
            tracing::info!("TLS disabled (HTTP only)");
            tokio::spawn(async move {
                if let Err(e) = crate::http::start_server(http_bind, http_state).await {
                    tracing::error!("HTTP server failed: {e}");
                }
            })
        };

        let tcp_server = tokio::spawn(async move {
            if let Err(e) = crate::tcp::start_server(tcp_bind, tcp_state).await {
                tracing::error!("TCP server failed: {e}");
            }
        });

        // Hot-reload watcher for builds.json
        let reload_task = tokio::spawn(async move {
            crate::watch::watch_builds(reload_state).await;
        });

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await.map_err(|e| {
            ServerError::Shutdown(format!("Failed to listen for shutdown signal: {e}"))
        })?;

        tracing::info!("Shutdown signal received, stopping server");

        http_server.abort();
        tcp_server.abort();
        reload_task.abort();

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
#[allow(clippy::unwrap_used)]
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

    fn make_config(file: &NamedTempFile) -> ServerConfig {
        ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        }
    }

    #[tokio::test]
    async fn test_app_state_creation() {
        let db_file = create_test_db_file();
        let state = AppState::new(&make_config(&db_file)).unwrap();
        let db = state.database().await;
        assert_eq!(db.total_builds(), 1);
        assert_eq!(state.cdn_config().hosts, "cdn.test.com");
        assert_eq!(state.cdn_config().path, "test/path");
    }

    #[tokio::test]
    async fn test_current_seqn_per_product() {
        let db_file = create_test_db_file();
        let state = AppState::new(&make_config(&db_file)).unwrap();

        // Known product has a real counter seeded from current time
        let seqn = state.current_seqn("test_product");
        assert!(
            seqn > 1_700_000_000,
            "seqn should be a reasonable Unix timestamp seed"
        );
        assert!(seqn < 2_000_000_000, "seqn should be before 2033");

        // Unknown product falls back to timestamp
        let seqn_unknown = state.current_seqn("nonexistent_product");
        assert!(seqn_unknown > 1_700_000_000);
    }

    #[tokio::test]
    async fn test_seqn_monotonic_across_reload() {
        let db_file = create_test_db_file();
        let state = AppState::new(&make_config(&db_file)).unwrap();

        let seqn_before = state.current_seqn("test_product");
        state.reload_database().await;
        let seqn_after = state.current_seqn("test_product");

        assert!(
            seqn_after >= seqn_before,
            "seqn must not decrease after reload: {seqn_before} -> {seqn_after}"
        );
    }

    #[test]
    fn test_uptime() {
        let db_file = create_test_db_file();
        let state = AppState::new(&make_config(&db_file)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(state.uptime_seconds() == 0); // Should be 0 or 1 second
    }
}
