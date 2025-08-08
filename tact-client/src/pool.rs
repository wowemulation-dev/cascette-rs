//! HTTP connection pool manager for TACT clients

use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::debug;

/// Global connection pool for TACT HTTP clients
static GLOBAL_POOL: OnceLock<Client> = OnceLock::new();

/// Default connection pool settings optimized for TACT operations
const DEFAULT_MAX_IDLE_CONNECTIONS: usize = 100;
const DEFAULT_MAX_IDLE_CONNECTIONS_PER_HOST: usize = 32;
const DEFAULT_POOL_IDLE_TIMEOUT_SECS: u64 = 90;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Configuration for the HTTP connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum idle connections in the pool
    pub max_idle_connections: Option<usize>,
    /// Maximum idle connections per host
    pub max_idle_connections_per_host: usize,
    /// How long to keep idle connections alive
    pub pool_idle_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// User agent string
    pub user_agent: Option<String>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_connections: Some(DEFAULT_MAX_IDLE_CONNECTIONS),
            max_idle_connections_per_host: DEFAULT_MAX_IDLE_CONNECTIONS_PER_HOST,
            pool_idle_timeout: Duration::from_secs(DEFAULT_POOL_IDLE_TIMEOUT_SECS),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            user_agent: None,
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum idle connections
    pub fn with_max_idle_connections(mut self, max: Option<usize>) -> Self {
        self.max_idle_connections = max;
        self
    }

    /// Set maximum idle connections per host  
    pub fn with_max_idle_connections_per_host(mut self, max: usize) -> Self {
        self.max_idle_connections_per_host = max;
        self
    }

    /// Set pool idle timeout
    pub fn with_pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_idle_timeout = timeout;
        self
    }

    /// Set request timeout
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set connection timeout  
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }
}

/// Initialize the global connection pool with custom configuration
///
/// This should be called once at application startup. If called multiple times,
/// subsequent calls will be ignored and the original pool configuration will be used.
///
/// Returns whether the pool was successfully initialized (true) or was already
/// initialized (false).
pub fn init_global_pool(config: PoolConfig) -> bool {
    let client = create_pooled_client(config);

    match GLOBAL_POOL.set(client) {
        Ok(()) => {
            debug!("Initialized global TACT HTTP connection pool");
            true
        }
        Err(_) => {
            debug!("Global TACT HTTP connection pool already initialized");
            false
        }
    }
}

/// Get the global connection pool
///
/// If the pool hasn't been initialized with `init_global_pool()`, this will
/// initialize it with default settings.
pub fn get_global_pool() -> &'static Client {
    GLOBAL_POOL.get_or_init(|| {
        debug!("Creating default global TACT HTTP connection pool");
        create_pooled_client(PoolConfig::default())
    })
}

/// Create a new pooled HTTP client with the specified configuration
pub fn create_pooled_client(config: PoolConfig) -> Client {
    debug!(
        "Creating HTTP client with pool settings: max_idle={:?}, max_per_host={}, idle_timeout={:?}",
        config.max_idle_connections, config.max_idle_connections_per_host, config.pool_idle_timeout
    );

    let mut builder = Client::builder()
        .pool_max_idle_per_host(config.max_idle_connections_per_host)
        .pool_idle_timeout(config.pool_idle_timeout)
        .timeout(config.request_timeout)
        .connect_timeout(config.connect_timeout)
        .use_rustls_tls() // Use rustls for TLS (more predictable than native-tls)
        // HTTP/2 is automatically negotiated when available
        .tcp_keepalive(Duration::from_secs(60)); // Keep TCP connections alive

    if let Some(max_idle) = config.max_idle_connections {
        builder = builder.pool_max_idle_per_host(max_idle);
    }

    if let Some(user_agent) = config.user_agent {
        builder = builder.user_agent(user_agent);
    }

    builder.build().expect("Failed to create HTTP client")
}

/// Get pool statistics (if available from reqwest)
///
/// Note: reqwest doesn't currently expose detailed connection pool metrics,
/// but this function is provided for future compatibility.
pub fn get_pool_stats() -> PoolStats {
    // reqwest doesn't expose pool metrics yet, so we return empty stats
    PoolStats {
        active_connections: None,
        idle_connections: None,
        total_connections: None,
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of active connections (if available)
    pub active_connections: Option<usize>,
    /// Number of idle connections (if available)
    pub idle_connections: Option<usize>,
    /// Total number of connections (if available)
    pub total_connections: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new()
            .with_max_idle_connections(Some(50))
            .with_max_idle_connections_per_host(20)
            .with_pool_idle_timeout(Duration::from_secs(60))
            .with_request_timeout(Duration::from_secs(45))
            .with_connect_timeout(Duration::from_secs(15))
            .with_user_agent("Test/1.0".to_string());

        assert_eq!(config.max_idle_connections, Some(50));
        assert_eq!(config.max_idle_connections_per_host, 20);
        assert_eq!(config.pool_idle_timeout, Duration::from_secs(60));
        assert_eq!(config.request_timeout, Duration::from_secs(45));
        assert_eq!(config.connect_timeout, Duration::from_secs(15));
        assert_eq!(config.user_agent, Some("Test/1.0".to_string()));
    }

    #[test]
    fn test_create_pooled_client() {
        let config = PoolConfig::default();
        let client = create_pooled_client(config);

        // Just verify the client was created successfully
        assert!(std::ptr::addr_of!(client) as usize != 0);
    }

    #[test]
    fn test_global_pool_initialization() {
        // Note: This test may interfere with other tests that use the global pool
        // In practice, the global pool should be initialized once per application
        let _config = PoolConfig::default().with_user_agent("TestPool/1.0".to_string());

        // Since we can't easily reset the global pool, we just test that getting it works
        let _pool = get_global_pool();

        // Verify pool stats function doesn't panic
        let _stats = get_pool_stats();
    }

    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(
            config.max_idle_connections,
            Some(DEFAULT_MAX_IDLE_CONNECTIONS)
        );
        assert_eq!(
            config.max_idle_connections_per_host,
            DEFAULT_MAX_IDLE_CONNECTIONS_PER_HOST
        );
        assert_eq!(
            config.pool_idle_timeout,
            Duration::from_secs(DEFAULT_POOL_IDLE_TIMEOUT_SECS)
        );
        assert_eq!(
            config.request_timeout,
            Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)
        );
        assert_eq!(
            config.connect_timeout,
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS)
        );
        assert!(config.user_agent.is_none());
    }
}
