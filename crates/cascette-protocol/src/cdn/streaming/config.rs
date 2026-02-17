//! Configuration structures for CDN streaming operations
//!
//! Provides comprehensive configuration options for tuning streaming behavior,
//! connection management, retry policies, and buffer sizing.

use std::time::Duration;

use super::{bootstrap::CdnBootstrap, http::CdnServer};

/// Configuration for streaming CDN operations
///
/// Contains all settings needed to optimize streaming performance including
/// network timeouts, buffer sizes, connection pooling, and retry behavior.
/// Default values are tuned for typical CDN usage patterns.
///
/// # Known limitations vs Agent.exe
///
/// The following Agent.exe connection parameters are not configurable
/// through reqwest and are documented here for reference:
///
/// - **Low speed limit** (Agent: 100 bps / 60s) — reqwest does not expose
///   a stall detection equivalent. Application-layer stall detection would
///   need to track throughput during stream consumption.
/// - **Receive buffer** (Agent: 256KB `SO_RCVBUF`) — reqwest does not
///   expose socket options. The OS default applies.
/// - **DNS cache TTL** (Agent: 300s) — reqwest uses the system resolver.
///   A custom TTL would require a custom DNS resolver implementation.
/// - **Total connection pool cap** (Agent: 12) — reqwest only exposes
///   per-host idle connection limits, not a total active connection cap.
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Timeout for individual HTTP requests
    ///
    /// How long to wait for a complete HTTP request/response cycle.
    /// Should be long enough for large range requests but short enough
    /// to detect hanging connections.
    pub request_timeout: Duration,

    /// Timeout for establishing connections
    ///
    /// Maximum time to wait when establishing a new TCP connection.
    /// Lower values provide faster failure detection but may cause
    /// issues on slow networks.
    pub connect_timeout: Duration,

    /// How long idle connections remain in the pool
    ///
    /// Connections unused for this duration are closed to free resources.
    /// Longer values reduce connection overhead but consume more resources.
    pub connection_idle_timeout: Duration,

    /// Maximum concurrent connections per host
    ///
    /// Limits simultaneous connections to prevent overwhelming the CDN.
    /// Higher values improve throughput but may trigger rate limiting.
    pub max_connections_per_host: usize,

    /// Buffer size for streaming data
    ///
    /// Size of internal buffers used for streaming operations.
    /// Larger buffers improve throughput but use more memory.
    pub stream_buffer_size: usize,

    /// Maximum size for a single range request
    ///
    /// Large range requests are split into chunks of this size.
    /// Balances memory usage against request overhead.
    pub max_range_size: u64,

    /// Minimum gap size for range coalescing
    ///
    /// Adjacent ranges separated by less than this amount are combined
    /// into a single request to reduce HTTP overhead.
    pub range_coalesce_threshold: u64,

    /// Maximum number of ranges in a single request
    ///
    /// Limits complexity of multipart range requests.
    /// Most CDNs support 5-10 ranges efficiently.
    pub max_ranges_per_request: usize,

    /// Maximum number of HTTP redirects to follow.
    /// Default: 5 (matches Agent.exe and the non-streaming client).
    pub max_redirects: usize,

    /// Retry configuration
    pub retry: RetryConfig,

    /// Connection pool configuration
    pub connection_pool: ConnectionPoolConfig,

    /// CDN server configuration
    pub cdn: CdnConfig,
}

/// Retry policy configuration
///
/// Controls how failed requests are retried, including backoff strategy
/// and maximum retry attempts.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts for failed requests
    pub max_attempts: u32,

    /// Base delay for exponential backoff
    ///
    /// Initial delay between retry attempts. Subsequent delays are calculated
    /// using exponential backoff with jitter.
    pub base_delay: Duration,

    /// Maximum delay between retry attempts
    ///
    /// Caps the exponential backoff to prevent excessively long waits.
    pub max_delay: Duration,

    /// Jitter factor for retry delays (0.0 to 1.0)
    ///
    /// Adds randomness to retry delays to prevent thundering herd problems.
    /// 0.0 = no jitter, 1.0 = full jitter (up to 100% variation).
    pub jitter_factor: f64,

    /// Whether to retry on specific HTTP status codes
    pub retry_on_status: Vec<u16>,
}

/// Connection pool configuration
///
/// Controls connection reuse and resource management for HTTP clients.
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum total connections across all hosts
    pub max_total_connections: usize,

    /// Maximum connections per individual host
    pub max_connections_per_host: usize,

    /// Keep-alive timeout for connections
    ///
    /// How long to keep connections open for potential reuse.
    /// Longer values improve efficiency but consume more resources.
    pub keep_alive_timeout: Duration,

    /// Enable TCP keep-alive probes
    ///
    /// Sends periodic probes to detect dead connections.
    /// Helps prevent hanging on broken connections.
    pub tcp_keepalive: bool,

    /// TCP keep-alive probe interval
    ///
    /// How often to send keep-alive probes when enabled.
    pub tcp_keepalive_interval: Duration,

    /// Enable connection pooling
    ///
    /// When disabled, creates a new connection for each request.
    /// Disabling may be useful for debugging or special network conditions.
    pub enable_pooling: bool,
}

/// CDN configuration for server management and failover
#[derive(Debug, Clone)]
pub struct CdnConfig {
    /// List of CDN servers for failover
    ///
    /// Servers are tried in priority order (lower priority values first).
    /// Should include both primary Blizzard CDNs and community mirrors.
    pub servers: Vec<CdnServer>,

    /// Prefer HTTPS when available
    ///
    /// When true, uses HTTPS for servers that support it.
    /// Falls back to HTTP for servers that don't.
    pub prefer_https: bool,

    /// Enable automatic server rotation
    ///
    /// When true, cycles through servers to distribute load.
    /// When false, always uses highest priority available server.
    pub enable_rotation: bool,

    /// Maximum number of servers to try before giving up
    ///
    /// Limits failover attempts to prevent excessive retry storms.
    /// Should be <= number of configured servers.
    pub max_failover_attempts: u32,

    /// Timeout for CDN server health checks
    ///
    /// How long to wait when testing if a server is responsive.
    /// Shorter values provide faster failover but may cause false negatives.
    pub health_check_timeout: Duration,

    /// Interval between health checks for failed servers
    ///
    /// How often to retry servers that have previously failed.
    /// Longer intervals reduce load but delay recovery detection.
    pub health_check_interval: Duration,

    /// Path cache TTL (time to live)
    ///
    /// How long to cache CDN paths before re-querying.
    /// Should be long enough to avoid excessive Ribbit queries.
    pub path_cache_ttl: Duration,

    /// Enable path validation
    ///
    /// When true, validates that cached paths still work.
    /// Helps detect CDN path changes but adds overhead.
    pub validate_paths: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            connection_idle_timeout: Duration::from_secs(90),
            max_connections_per_host: 8,
            stream_buffer_size: 64 * 1024,       // 64KB
            max_range_size: 10 * 1024 * 1024,    // 10MB
            range_coalesce_threshold: 64 * 1024, // 64KB
            max_ranges_per_request: 6,
            max_redirects: 5,
            retry: RetryConfig::default(),
            connection_pool: ConnectionPoolConfig::default(),
            cdn: CdnConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.1,
            retry_on_status: vec![429, 500, 502, 503, 504],
        }
    }
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(60),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        }
    }
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            servers: Self::default_cdn_servers(),
            prefer_https: true,
            enable_rotation: false,
            max_failover_attempts: 3,
            health_check_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(300), // 5 minutes
            path_cache_ttl: Duration::from_secs(3600),       // 1 hour
            validate_paths: false,
        }
    }
}

impl CdnConfig {
    /// Get community mirror CDN servers only
    ///
    /// These mirrors host historic NGDP content that may no longer be
    /// available on official Blizzard CDNs. Use for archived builds.
    /// All mirrors support HTTPS connections.
    ///
    /// Mirrors (in priority order):
    /// - casc.wago.tools: Full NGDP mirror, HTTPS
    /// - cdn.arctium.tools: Full NGDP mirror, HTTPS, most complete
    /// - archive.wow.tools: Historical content archive, HTTPS
    pub fn community_mirrors() -> Vec<CdnServer> {
        vec![
            CdnServer::new("casc.wago.tools".to_string(), true, 10),
            CdnServer::new("cdn.arctium.tools".to_string(), true, 20),
            CdnServer::new("archive.wow.tools".to_string(), true, 30),
        ]
    }

    /// Get official Blizzard CDN servers only
    ///
    /// Use for current/live builds. These CDNs only host recent content.
    pub fn blizzard_cdns() -> Vec<CdnServer> {
        vec![
            CdnServer::new("level3.blizzard.com".to_string(), true, 10),
            CdnServer::new("blzddist1-a.akamaihd.net".to_string(), true, 20),
            CdnServer::new("us.cdn.blizzard.com".to_string(), true, 30),
            CdnServer::new("eu.cdn.blizzard.com".to_string(), true, 40),
        ]
    }

    /// Get default CDN servers (Blizzard official + community mirrors)
    ///
    /// Includes both primary Blizzard CDNs and reliable community mirrors
    /// for fallback when official servers are unavailable.
    pub fn default_cdn_servers() -> Vec<CdnServer> {
        let mut servers = Self::blizzard_cdns();
        // Add community mirrors with lower priority (100+)
        for mut mirror in Self::community_mirrors() {
            mirror.priority += 100; // Shift priority to be after Blizzard CDNs
            servers.push(mirror);
        }
        servers
    }

    /// Create configuration optimized for official Blizzard CDNs only
    ///
    /// Use for current/live builds where content is available on official CDNs.
    pub fn blizzard_only() -> Self {
        Self {
            servers: Self::blizzard_cdns(),
            prefer_https: true,
            enable_rotation: true, // Distribute load across official servers
            max_failover_attempts: 4,
            health_check_timeout: Duration::from_secs(10),
            health_check_interval: Duration::from_secs(60),
            path_cache_ttl: Duration::from_secs(1800), // 30 minutes
            validate_paths: true,
        }
    }

    /// Create configuration for community mirrors only
    ///
    /// Use for historic/archived builds that are no longer on official CDNs.
    pub fn community_only() -> Self {
        Self {
            servers: Self::community_mirrors(),
            prefer_https: true, // Prefer HTTPS but allow HTTP fallback
            enable_rotation: true,
            max_failover_attempts: 3,
            health_check_timeout: Duration::from_secs(15), // Mirrors may be slower
            health_check_interval: Duration::from_secs(300),
            path_cache_ttl: Duration::from_secs(7200), // 2 hours - historic content stable
            validate_paths: false,
        }
    }

    /// Create configuration with community mirrors for high availability
    pub fn high_availability() -> Self {
        Self {
            servers: Self::default_cdn_servers(),
            prefer_https: true,
            enable_rotation: false,   // Prefer official servers first
            max_failover_attempts: 5, // Try more servers
            health_check_timeout: Duration::from_secs(3),
            health_check_interval: Duration::from_secs(120),
            path_cache_ttl: Duration::from_secs(7200), // 2 hours
            validate_paths: false,                     // Less validation overhead
        }
    }

    /// Create configuration for development/testing with community mirrors
    pub fn development() -> Self {
        Self {
            servers: Self::community_mirrors(),
            prefer_https: false, // Allow HTTP for simpler testing
            enable_rotation: true,
            max_failover_attempts: 3,
            health_check_timeout: Duration::from_secs(10),
            health_check_interval: Duration::from_secs(30),
            path_cache_ttl: Duration::from_secs(300), // 5 minutes
            validate_paths: true,                     // More validation for development
        }
    }

    /// Update CDN configuration from bootstrap
    ///
    /// Merges bootstrap configuration with existing settings,
    /// preserving current configuration values while updating servers.
    ///
    /// # Arguments
    /// * `bootstrap` - CDN bootstrap configuration from Ribbit
    pub fn update_from_bootstrap(&mut self, bootstrap: &CdnBootstrap) {
        // Replace servers with bootstrap servers
        self.servers.clone_from(&bootstrap.servers);

        // Update max_failover_attempts to match server count if needed
        if self.max_failover_attempts > self.servers.len() as u32 {
            self.max_failover_attempts = self.servers.len().max(1) as u32;
        }
    }

    /// Merge additional servers while preserving existing configuration
    ///
    /// # Arguments
    /// * `additional_servers` - New servers to add
    pub fn merge_servers(&mut self, additional_servers: Vec<CdnServer>) {
        for server in additional_servers {
            // Skip duplicates
            if !self.servers.iter().any(|s| s.host == server.host) {
                self.servers.push(server);
            }
        }

        // Re-sort by priority
        self.servers.sort_by_key(|s| s.priority);
    }

    /// Remove servers that are no longer available
    ///
    /// # Arguments
    /// * `hosts_to_remove` - Hostnames to remove from server list
    pub fn remove_servers(&mut self, hosts_to_remove: &[String]) {
        self.servers
            .retain(|server| !hosts_to_remove.contains(&server.host));

        // Adjust max_failover_attempts if we removed too many servers
        if self.max_failover_attempts > self.servers.len() as u32 {
            self.max_failover_attempts = self.servers.len().max(1) as u32;
        }
    }

    /// Update server priorities
    ///
    /// # Arguments
    /// * `priority_updates` - Map of hostname to new priority
    pub fn update_server_priorities(
        &mut self,
        priority_updates: &std::collections::HashMap<String, u32>,
    ) {
        for server in &mut self.servers {
            if let Some(new_priority) = priority_updates.get(&server.host) {
                server.priority = *new_priority;
            }
        }

        // Re-sort by updated priorities
        self.servers.sort_by_key(|s| s.priority);
    }

    /// Create runtime-updatable configuration from bootstrap
    ///
    /// Creates a new CDN configuration using bootstrap data while
    /// preserving sensible defaults for other settings.
    ///
    /// # Arguments
    /// * `bootstrap` - Bootstrap configuration from Ribbit
    /// * `base_config` - Optional base configuration to extend
    pub fn from_bootstrap(bootstrap: &CdnBootstrap, base_config: Option<&Self>) -> Self {
        let default_config = Self::default();
        let base = base_config.unwrap_or(&default_config);

        Self {
            servers: bootstrap.servers.clone(),
            prefer_https: base.prefer_https,
            enable_rotation: base.enable_rotation,
            max_failover_attempts: (bootstrap.servers.len().max(1) as u32)
                .min(base.max_failover_attempts),
            health_check_timeout: base.health_check_timeout,
            health_check_interval: base.health_check_interval,
            path_cache_ttl: base.path_cache_ttl,
            validate_paths: base.validate_paths,
        }
    }

    /// Check if configuration allows runtime updates
    ///
    /// Some configurations may be locked to prevent runtime changes
    /// for security or consistency reasons.
    pub fn allows_runtime_updates(&self) -> bool {
        // For now, all configurations allow updates
        // Future versions might add configuration locks
        true
    }

    /// Validate CDN configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.servers.is_empty() {
            return Err("At least one CDN server must be configured".to_string());
        }

        if self.max_failover_attempts == 0 {
            return Err("max_failover_attempts must be greater than 0".to_string());
        }

        if self.max_failover_attempts > self.servers.len() as u32 {
            return Err("max_failover_attempts should not exceed number of servers".to_string());
        }

        // Validate server configurations
        for (i, server) in self.servers.iter().enumerate() {
            if server.host.is_empty() {
                return Err(format!("Server {i} has empty host"));
            }
        }

        Ok(())
    }
}

impl StreamingConfig {
    /// Create a new configuration optimized for high throughput
    ///
    /// Increases connection limits, buffer sizes, and range request sizes
    /// for scenarios where maximum throughput is more important than
    /// resource conservation.
    pub fn high_throughput() -> Self {
        Self {
            max_connections_per_host: 16,
            stream_buffer_size: 256 * 1024,       // 256KB
            max_range_size: 50 * 1024 * 1024,     // 50MB
            range_coalesce_threshold: 256 * 1024, // 256KB
            max_ranges_per_request: 10,
            connection_pool: ConnectionPoolConfig {
                max_total_connections: 200,
                keep_alive_timeout: Duration::from_secs(120),
                ..Default::default()
            },
            cdn: CdnConfig::high_availability(),
            ..Default::default()
        }
    }

    /// Create a new configuration optimized for low memory usage
    ///
    /// Reduces buffer sizes and connection limits for memory-constrained
    /// environments such as embedded systems or mobile applications.
    pub fn low_memory() -> Self {
        Self {
            max_connections_per_host: 2,
            stream_buffer_size: 16 * 1024,       // 16KB
            max_range_size: 1024 * 1024,         // 1MB
            range_coalesce_threshold: 16 * 1024, // 16KB
            max_ranges_per_request: 2,
            connection_pool: ConnectionPoolConfig {
                max_total_connections: 20,
                keep_alive_timeout: Duration::from_secs(30),
                ..Default::default()
            },
            cdn: CdnConfig {
                servers: vec![
                    // Fewer servers for low memory
                    CdnServer::new("level3.blizzard.com".to_string(), true, 10),
                    CdnServer::new("cdn.arctium.tools".to_string(), true, 20),
                ],
                max_failover_attempts: 2,
                path_cache_ttl: Duration::from_secs(1800),
                ..CdnConfig::default()
            },
            ..Default::default()
        }
    }

    /// Create a new configuration optimized for unreliable networks
    ///
    /// Increases retry attempts, reduces timeouts, and adjusts buffer sizes
    /// for networks with high latency or frequent connection issues.
    pub fn unreliable_network() -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(20),
            max_range_size: 1024 * 1024, // 1MB chunks for faster recovery
            retry: RetryConfig {
                max_attempts: 5,
                base_delay: Duration::from_millis(500),
                max_delay: Duration::from_secs(60),
                jitter_factor: 0.2,
                ..Default::default()
            },
            cdn: CdnConfig {
                max_failover_attempts: 5, // More attempts for unreliable networks
                health_check_timeout: Duration::from_secs(15),
                health_check_interval: Duration::from_secs(60),
                enable_rotation: true, // Distribute load to find working servers
                ..CdnConfig::default()
            },
            ..Default::default()
        }
    }

    /// Validate the configuration for consistency and reasonable values
    ///
    /// Checks for common configuration errors and returns detailed error
    /// messages for invalid settings.
    ///
    /// # Returns
    /// `Ok(())` if the configuration is valid, or an error string describing
    /// the first validation failure found.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_connections_per_host == 0 {
            return Err("max_connections_per_host must be greater than 0".to_string());
        }

        if self.stream_buffer_size == 0 {
            return Err("stream_buffer_size must be greater than 0".to_string());
        }

        if self.max_range_size == 0 {
            return Err("max_range_size must be greater than 0".to_string());
        }

        if self.max_ranges_per_request == 0 {
            return Err("max_ranges_per_request must be greater than 0".to_string());
        }

        if self.range_coalesce_threshold > self.max_range_size {
            return Err("range_coalesce_threshold should not exceed max_range_size".to_string());
        }

        if self.retry.max_attempts == 0 {
            return Err("retry.max_attempts must be greater than 0".to_string());
        }

        if self.retry.jitter_factor < 0.0 || self.retry.jitter_factor > 1.0 {
            return Err("retry.jitter_factor must be between 0.0 and 1.0".to_string());
        }

        if self.connection_pool.max_total_connections == 0 {
            return Err("connection_pool.max_total_connections must be greater than 0".to_string());
        }

        if self.connection_pool.max_total_connections < self.max_connections_per_host {
            return Err(
                "connection_pool.max_total_connections should be >= max_connections_per_host"
                    .to_string(),
            );
        }

        // Validate CDN configuration
        self.cdn.validate()?;

        Ok(())
    }

    /// Update configuration with new CDN settings from Ribbit
    ///
    /// Merges new CDN configuration while preserving performance settings.
    ///
    /// # Arguments
    /// * `bootstrap` - New bootstrap configuration from Ribbit
    pub fn update_cdn_from_bootstrap(&mut self, bootstrap: &CdnBootstrap) {
        self.cdn.update_from_bootstrap(bootstrap);
    }

    /// Create configuration for runtime updates
    ///
    /// Creates a configuration that can be safely updated at runtime
    /// without disrupting existing connections.
    ///
    /// # Arguments
    /// * `bootstrap` - Initial bootstrap configuration
    pub fn for_runtime_updates(bootstrap: &CdnBootstrap) -> Result<Self, String> {
        bootstrap.validate().map_err(|e| e.to_string())?;

        Ok(Self {
            cdn: CdnConfig::from_bootstrap(bootstrap, None),
            // Use conservative settings for runtime-updatable configs
            retry: RetryConfig {
                max_attempts: 5, // More retries for changing environments
                base_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(45),
                jitter_factor: 0.2, // More jitter for distributed load
                ..RetryConfig::default()
            },
            ..Self::default()
        })
    }

    /// Calculate the total memory usage estimate for this configuration
    ///
    /// Returns an estimate of memory usage in bytes based on buffer sizes
    /// and connection limits. Useful for capacity planning.
    pub fn estimated_memory_usage(&self) -> usize {
        let buffer_memory = self.stream_buffer_size * self.max_connections_per_host;
        let connection_overhead = self.connection_pool.max_total_connections * 8192; // Estimated per-connection overhead
        let path_cache_overhead = self.cdn.servers.len() * 256; // Estimated per-server path cache
        buffer_memory + connection_overhead + path_cache_overhead
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = StreamingConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_high_throughput_config() {
        let config = StreamingConfig::high_throughput();
        assert!(config.validate().is_ok());
        assert!(
            config.max_connections_per_host > StreamingConfig::default().max_connections_per_host
        );
        assert!(config.stream_buffer_size > StreamingConfig::default().stream_buffer_size);
    }

    #[test]
    fn test_low_memory_config() {
        let config = StreamingConfig::low_memory();
        assert!(config.validate().is_ok());
        assert!(config.stream_buffer_size < StreamingConfig::default().stream_buffer_size);
        assert!(
            config.max_connections_per_host < StreamingConfig::default().max_connections_per_host
        );
    }

    #[test]
    fn test_unreliable_network_config() {
        let config = StreamingConfig::unreliable_network();
        assert!(config.validate().is_ok());
        assert!(config.retry.max_attempts > StreamingConfig::default().retry.max_attempts);
        assert!(config.request_timeout > StreamingConfig::default().request_timeout);
    }

    #[test]
    fn test_invalid_config_validation() {
        let config = StreamingConfig {
            max_connections_per_host: 0,
            ..StreamingConfig::default()
        };
        assert!(config.validate().is_err());

        let config = StreamingConfig {
            stream_buffer_size: 0,
            ..StreamingConfig::default()
        };
        assert!(config.validate().is_err());

        let config = StreamingConfig {
            retry: RetryConfig {
                jitter_factor: 2.0,
                ..RetryConfig::default()
            },
            ..StreamingConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_memory_usage_calculation() {
        let config = StreamingConfig::default();
        let usage = config.estimated_memory_usage();
        assert!(usage > 0);
        assert!(usage > config.stream_buffer_size); // Should include connection overhead
    }

    #[test]
    fn test_config_cloning() {
        let config = StreamingConfig::default();
        let cloned = config;
        let other = StreamingConfig::default();
        assert_eq!(
            other.max_connections_per_host,
            cloned.max_connections_per_host
        );
        assert_eq!(other.stream_buffer_size, cloned.stream_buffer_size);
    }

    #[test]
    fn test_cdn_config_default() {
        let config = CdnConfig::default();
        assert!(!config.servers.is_empty());
        assert!(config.prefer_https);
        assert!(!config.enable_rotation);
        assert_eq!(config.max_failover_attempts, 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cdn_config_blizzard_only() {
        let config = CdnConfig::blizzard_only();
        assert_eq!(config.servers.len(), 4);
        assert!(config.enable_rotation);
        assert!(config.validate_paths);
        assert!(config.validate().is_ok());

        // All servers should be Blizzard-operated (includes Akamai CDN)
        for server in &config.servers {
            assert!(
                server.host.contains("blizzard.com") || server.host.contains("akamaihd.net"),
                "Unexpected server host: {}",
                server.host
            );
            assert!(server.supports_https);
        }
    }

    #[test]
    fn test_cdn_config_high_availability() {
        let config = CdnConfig::high_availability();
        assert!(config.servers.len() >= 3); // Should include mirrors
        assert_eq!(config.max_failover_attempts, 5);
        assert!(!config.validate_paths); // Less overhead
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cdn_config_development() {
        let config = CdnConfig::development();
        assert_eq!(config.servers.len(), 3);
        assert!(!config.prefer_https); // Allow HTTP for testing
        assert!(config.enable_rotation);
        assert!(config.validate_paths); // More validation
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cdn_config_validation() {
        // Valid config should pass
        let config = CdnConfig::default();
        assert!(config.validate().is_ok());

        // Empty servers should fail
        let config = CdnConfig {
            servers: vec![],
            ..CdnConfig::default()
        };
        assert!(config.validate().is_err());

        // Zero max_failover_attempts should fail
        let config = CdnConfig {
            max_failover_attempts: 0,
            ..CdnConfig::default()
        };
        assert!(config.validate().is_err());

        // Too many failover attempts should fail
        let config = CdnConfig {
            servers: vec![CdnServer::https("example.com".to_string())],
            max_failover_attempts: 5, // More than 1 server
            ..CdnConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_streaming_config_with_cdn() {
        let config = StreamingConfig::default();
        assert!(config.validate().is_ok());

        // CDN config should be included in validation
        let mut config = StreamingConfig::default();
        config.cdn.servers.clear(); // Invalid CDN config
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cdn_config_runtime_updates() {
        let mut config = CdnConfig::default();
        let initial_server_count = config.servers.len();

        // Test server merging
        let additional_servers = vec![
            CdnServer::new("new.example.com".to_string(), true, 150),
            CdnServer::new("level3.blizzard.com".to_string(), true, 10), // Duplicate
        ];
        config.merge_servers(additional_servers);

        // Should add new server but not duplicate
        assert_eq!(config.servers.len(), initial_server_count + 1);
        assert!(config.servers.iter().any(|s| s.host == "new.example.com"));

        // Test server removal
        config.remove_servers(&["new.example.com".to_string()]);
        assert_eq!(config.servers.len(), initial_server_count);
        assert!(!config.servers.iter().any(|s| s.host == "new.example.com"));
    }

    #[test]
    fn test_cdn_config_priority_updates() {
        let mut config = CdnConfig::default();
        let first_server_host = config.servers[0].host.clone();
        let original_priority = config.servers[0].priority;

        let mut priority_updates = std::collections::HashMap::new();
        priority_updates.insert(first_server_host.clone(), 999);

        config.update_server_priorities(&priority_updates);

        // Should update priority and re-sort
        assert_eq!(
            config
                .servers
                .last()
                .expect("Operation should succeed")
                .host,
            first_server_host
        );
        assert_eq!(
            config
                .servers
                .last()
                .expect("Operation should succeed")
                .priority,
            999
        );
        assert_ne!(
            config
                .servers
                .last()
                .expect("Operation should succeed")
                .priority,
            original_priority
        );
    }

    #[test]
    fn test_streaming_config_runtime_creation() {
        let mut bootstrap = CdnBootstrap::new();
        bootstrap.servers = vec![
            CdnServer::https("level3.blizzard.com".to_string()),
            CdnServer::http("backup.example.com".to_string()),
        ];
        bootstrap
            .paths
            .insert("wow".to_string(), "tpr/wow".to_string());

        let config = StreamingConfig::for_runtime_updates(&bootstrap);
        assert!(config.is_ok());

        let config = config.expect("Operation should succeed");
        assert_eq!(config.cdn.servers.len(), 2);
        assert!(config.retry.max_attempts > StreamingConfig::default().retry.max_attempts);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cdn_config_from_bootstrap() {
        let mut bootstrap = CdnBootstrap::new();
        bootstrap.servers = vec![
            CdnServer::https("test1.example.com".to_string()),
            CdnServer::http("test2.example.com".to_string()),
        ];

        let config = CdnConfig::from_bootstrap(&bootstrap, None);
        assert_eq!(config.servers.len(), 2);
        assert_eq!(config.max_failover_attempts, 2);
        assert!(config.validate().is_ok());

        // Test with base config
        let base = CdnConfig::development();
        let config = CdnConfig::from_bootstrap(&bootstrap, Some(&base));
        assert_eq!(config.servers.len(), 2); // From bootstrap
        assert_eq!(config.prefer_https, base.prefer_https); // From base
    }

    #[test]
    fn test_memory_usage_includes_cdn() {
        let config = StreamingConfig::default();
        let base_usage = config.stream_buffer_size * config.max_connections_per_host
            + config.connection_pool.max_total_connections * 8192;
        let total_usage = config.estimated_memory_usage();

        // Should include CDN path cache overhead
        assert!(total_usage > base_usage);
        assert!(total_usage >= base_usage + config.cdn.servers.len() * 256);
    }
}
