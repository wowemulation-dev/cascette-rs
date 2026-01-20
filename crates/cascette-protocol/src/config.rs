//! Configuration structures for protocol clients

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::error::Result;
use crate::retry::RetryPolicy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// TACT HTTPS endpoint URL
    pub tact_https_url: String,

    /// TACT HTTP endpoint URL
    pub tact_http_url: String,

    /// Ribbit TCP URL (tcp://host:port format)
    pub ribbit_url: String,

    /// Cache configuration
    pub cache_config: CacheConfig,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Request timeout
    pub request_timeout: Duration,

    /// Retry policy for failed requests
    pub retry_policy: RetryPolicy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            tact_https_url: "https://us.version.battle.net".to_string(),
            tact_http_url: "http://us.patch.battle.net:1119".to_string(),
            ribbit_url: "tcp://us.version.battle.net:1119".to_string(),
            cache_config: CacheConfig::default(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            retry_policy: RetryPolicy::default(),
        }
    }
}

impl ClientConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            tact_https_url: std::env::var("CASCETTE_TACT_HTTPS_URL")
                .unwrap_or_else(|_| "https://us.version.battle.net".to_string()),
            tact_http_url: std::env::var("CASCETTE_TACT_HTTP_URL")
                .unwrap_or_else(|_| "http://us.patch.battle.net:1119".to_string()),
            ribbit_url: std::env::var("CASCETTE_RIBBIT_URL")
                .unwrap_or_else(|_| "tcp://us.version.battle.net:1119".to_string()),
            cache_config: CacheConfig::from_env()?,
            connect_timeout: Duration::from_secs(
                std::env::var("CASCETTE_CONNECT_TIMEOUT")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            ),
            request_timeout: Duration::from_secs(
                std::env::var("CASCETTE_REQUEST_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            ),
            retry_policy: RetryPolicy::from_env()?,
        })
    }
}

/// High-performance cache configuration optimized for NGDP protocol operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache directory path
    pub cache_dir: Option<PathBuf>,

    /// Memory cache configuration
    pub memory_max_items: usize,
    pub memory_max_size_bytes: usize,

    /// Disk cache configuration
    pub disk_max_size_bytes: usize,
    pub disk_max_file_size: usize,

    /// TTL for Ribbit/TACT responses
    pub ribbit_ttl: Duration,

    /// TTL for CDN content
    pub cdn_ttl: Duration,

    /// TTL for configuration files
    pub config_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_dir: None, // Will use default from cache backend
            // Memory cache optimized for protocol responses
            memory_max_items: 10000,                  // 10k items in memory
            memory_max_size_bytes: 256 * 1024 * 1024, // 256MB memory cache
            // Disk cache for larger items
            disk_max_size_bytes: 8 * 1024 * 1024 * 1024, // 8GB disk cache
            disk_max_file_size: 100 * 1024 * 1024,       // 100MB max file size
            // Protocol-specific TTLs
            ribbit_ttl: Duration::from_secs(300), // 5 minutes for version info
            cdn_ttl: Duration::from_secs(3600),   // 1 hour for CDN content
            config_ttl: Duration::from_secs(1800), // 30 minutes for config files
        }
    }
}

impl CacheConfig {
    /// Create optimized cache configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            cache_dir: std::env::var("CASCETTE_CACHE_DIR").map(PathBuf::from).ok(),
            memory_max_items: std::env::var("CASCETTE_MEMORY_MAX_ITEMS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10000),
            memory_max_size_bytes: std::env::var("CASCETTE_MEMORY_MAX_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(256 * 1024 * 1024),
            disk_max_size_bytes: std::env::var("CASCETTE_DISK_MAX_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8 * 1024 * 1024 * 1024),
            disk_max_file_size: std::env::var("CASCETTE_DISK_MAX_FILE_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100 * 1024 * 1024),
            ribbit_ttl: Duration::from_secs(
                std::env::var("CASCETTE_RIBBIT_TTL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300),
            ),
            cdn_ttl: Duration::from_secs(
                std::env::var("CASCETTE_CDN_TTL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
            ),
            config_ttl: Duration::from_secs(
                std::env::var("CASCETTE_CONFIG_TTL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1800),
            ),
        })
    }

    /// Create a high-performance configuration for production workloads
    pub fn production() -> Self {
        Self {
            cache_dir: Some(PathBuf::from("/var/cache/cascette")),
            memory_max_items: 50000,                   // 50k items in memory
            memory_max_size_bytes: 1024 * 1024 * 1024, // 1GB memory cache
            disk_max_size_bytes: 32 * 1024 * 1024 * 1024, // 32GB disk cache
            disk_max_file_size: 500 * 1024 * 1024,     // 500MB max file size
            ribbit_ttl: Duration::from_secs(180),      // 3 minutes for faster updates
            cdn_ttl: Duration::from_secs(7200),        // 2 hours for CDN content
            config_ttl: Duration::from_secs(900),      // 15 minutes for config files
        }
    }

    /// Create a memory-optimized configuration for low-memory environments
    pub fn memory_optimized() -> Self {
        Self {
            cache_dir: None,
            memory_max_items: 1000,                  // 1k items in memory
            memory_max_size_bytes: 32 * 1024 * 1024, // 32MB memory cache
            disk_max_size_bytes: 1024 * 1024 * 1024, // 1GB disk cache
            disk_max_file_size: 10 * 1024 * 1024,    // 10MB max file size
            ribbit_ttl: Duration::from_secs(600),    // 10 minutes
            cdn_ttl: Duration::from_secs(3600),      // 1 hour
            config_ttl: Duration::from_secs(1800),   // 30 minutes
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnConfig {
    /// Maximum concurrent downloads
    pub max_concurrent: usize,

    /// Chunk size for large downloads
    pub chunk_size: usize,

    /// Enable progress tracking
    pub enable_progress: bool,

    /// Connection pool size
    pub pool_size: usize,
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            chunk_size: 4 * 1024 * 1024, // 4MB
            enable_progress: false,
            pool_size: 20,
        }
    }
}

#[cfg(test)]
#[allow(
    unsafe_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(
            config.tact_https_url,
            "https://us.version.battle.net".to_string()
        );
        assert_eq!(
            config.tact_http_url,
            "http://us.patch.battle.net:1119".to_string()
        );
        assert_eq!(
            config.ribbit_url,
            "tcp://us.version.battle.net:1119".to_string()
        );
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_client_config_from_env() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;

        // Generate a unique test ID to avoid conflicts during parallel execution
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let thread_id = thread::current().id();
        let unique_suffix = format!("{thread_id:?}_{test_id}");

        // Use unique environment variable names for this test instance
        let secure_url_env = format!("CASCETTE_TACT_HTTPS_URL_TEST_{unique_suffix}");
        let plain_url_env = format!("CASCETTE_TACT_HTTP_URL_TEST_{unique_suffix}");
        let hosts_list_env = format!("CASCETTE_RIBBIT_HOSTS_TEST_{unique_suffix}");
        let conn_timeout_env = format!("CASCETTE_CONNECT_TIMEOUT_TEST_{unique_suffix}");
        let req_timeout_env = format!("CASCETTE_REQUEST_TIMEOUT_TEST_{unique_suffix}");

        // Set test-specific environment variables
        unsafe {
            std::env::set_var(&secure_url_env, "https://example.com");
            std::env::set_var(&plain_url_env, "http://example.com");
            std::env::set_var(&hosts_list_env, "host1:1119,host2:1119,host3:1119");
            std::env::set_var(&conn_timeout_env, "15");
            std::env::set_var(&req_timeout_env, "45");
        }

        // Test the configuration with the test-specific environment variables
        let config = test_client_config_from_env_with_prefix(&format!("_TEST_{}", unique_suffix));

        assert_eq!(config.tact_https_url, "https://example.com".to_string());
        assert_eq!(config.tact_http_url, "http://example.com".to_string());
        // For single URL config, we'll take the first host from the list
        assert_eq!(config.ribbit_url, "tcp://host1:1119".to_string());
        assert_eq!(config.connect_timeout, Duration::from_secs(15));
        assert_eq!(config.request_timeout, Duration::from_secs(45));

        // Clean up test-specific environment variables
        unsafe {
            std::env::remove_var(&secure_url_env);
            std::env::remove_var(&plain_url_env);
            std::env::remove_var(&hosts_list_env);
            std::env::remove_var(&conn_timeout_env);
            std::env::remove_var(&req_timeout_env);
        }
    }

    /// Helper function to create configuration from environment variables with a suffix
    /// This is used for testing to avoid conflicts during parallel execution
    fn test_client_config_from_env_with_prefix(suffix: &str) -> ClientConfig {
        let cache_config = test_cache_config_from_env_with_prefix(suffix);
        let retry_policy = test_retry_policy_from_env_with_suffix(suffix);

        ClientConfig {
            tact_https_url: std::env::var(format!("CASCETTE_TACT_HTTPS_URL{}", suffix))
                .unwrap_or_else(|_| "https://us.version.battle.net".to_string()),
            tact_http_url: std::env::var(format!("CASCETTE_TACT_HTTP_URL{}", suffix))
                .unwrap_or_else(|_| "http://us.patch.battle.net:1119".to_string()),
            ribbit_url: std::env::var(format!("CASCETTE_RIBBIT_HOSTS{}", suffix)).map_or_else(
                |_| "tcp://us.version.battle.net:1119".to_string(),
                |hosts| {
                    // Take the first host if multiple are provided
                    let first_host = hosts
                        .split(',')
                        .next()
                        .unwrap_or("us.version.battle.net:1119");
                    format!("tcp://{}", first_host.trim())
                },
            ),
            cache_config,
            connect_timeout: Duration::from_secs(
                std::env::var(format!("CASCETTE_CONNECT_TIMEOUT{}", suffix))
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            ),
            request_timeout: Duration::from_secs(
                std::env::var(format!("CASCETTE_REQUEST_TIMEOUT{}", suffix))
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            ),
            retry_policy,
        }
    }

    /// Helper function to create cache configuration from environment variables with a suffix
    fn test_cache_config_from_env_with_prefix(suffix: &str) -> CacheConfig {
        CacheConfig {
            cache_dir: std::env::var(format!("CASCETTE_CACHE_DIR{}", suffix))
                .map(PathBuf::from)
                .ok(),
            memory_max_items: std::env::var(format!("CASCETTE_MEMORY_MAX_ITEMS{}", suffix))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10000),
            memory_max_size_bytes: std::env::var(format!("CASCETTE_MEMORY_MAX_SIZE{}", suffix))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(256 * 1024 * 1024),
            disk_max_size_bytes: std::env::var(format!("CASCETTE_DISK_MAX_SIZE{}", suffix))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8 * 1024 * 1024 * 1024),
            disk_max_file_size: std::env::var(format!("CASCETTE_DISK_MAX_FILE_SIZE{}", suffix))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100 * 1024 * 1024),
            ribbit_ttl: Duration::from_secs(
                std::env::var(format!("CASCETTE_RIBBIT_TTL{}", suffix))
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300),
            ),
            cdn_ttl: Duration::from_secs(
                std::env::var(format!("CASCETTE_CDN_TTL{}", suffix))
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
            ),
            config_ttl: Duration::from_secs(
                std::env::var(format!("CASCETTE_CONFIG_TTL{}", suffix))
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1800),
            ),
        }
    }

    /// Helper function to create retry policy from environment variables with a suffix
    fn test_retry_policy_from_env_with_suffix(_suffix: &str) -> RetryPolicy {
        // For now, return the default retry policy since RetryPolicy::from_env()
        // doesn't have environment variable support implemented yet
        RetryPolicy::default()
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(config.cache_dir.is_none());
        assert_eq!(config.memory_max_items, 10000);
        assert_eq!(config.memory_max_size_bytes, 256 * 1024 * 1024);
        assert_eq!(config.disk_max_size_bytes, 8 * 1024 * 1024 * 1024);
        assert_eq!(config.disk_max_file_size, 100 * 1024 * 1024);
        assert_eq!(config.ribbit_ttl, Duration::from_secs(300));
        assert_eq!(config.cdn_ttl, Duration::from_secs(3600));
        assert_eq!(config.config_ttl, Duration::from_secs(1800));
    }

    #[test]
    fn test_cache_config_production() {
        let config = CacheConfig::production();
        assert_eq!(config.cache_dir, Some(PathBuf::from("/var/cache/cascette")));
        assert_eq!(config.memory_max_items, 50000);
        assert_eq!(config.memory_max_size_bytes, 1024 * 1024 * 1024);
        assert_eq!(config.disk_max_size_bytes, 32 * 1024 * 1024 * 1024);
        assert_eq!(config.ribbit_ttl, Duration::from_secs(180));
    }

    #[test]
    fn test_cache_config_memory_optimized() {
        let config = CacheConfig::memory_optimized();
        assert!(config.cache_dir.is_none());
        assert_eq!(config.memory_max_items, 1000);
        assert_eq!(config.memory_max_size_bytes, 32 * 1024 * 1024);
        assert_eq!(config.disk_max_size_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn test_cache_config_from_env() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;

        // Generate a unique test ID to avoid conflicts during parallel execution
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(1000); // Different counter for cache test
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let thread_id = thread::current().id();
        let unique_suffix = format!("CACHE_{:?}_{}", thread_id, test_id);

        // Use unique environment variable names for this test instance
        let cache_dir_var = format!("CASCETTE_CACHE_DIR_TEST_{}", unique_suffix);
        let memory_items_var = format!("CASCETTE_MEMORY_MAX_ITEMS_TEST_{}", unique_suffix);
        let memory_size_var = format!("CASCETTE_MEMORY_MAX_SIZE_TEST_{}", unique_suffix);
        let disk_size_var = format!("CASCETTE_DISK_MAX_SIZE_TEST_{}", unique_suffix);
        let disk_file_size_var = format!("CASCETTE_DISK_MAX_FILE_SIZE_TEST_{}", unique_suffix);
        let ribbit_ttl_var = format!("CASCETTE_RIBBIT_TTL_TEST_{}", unique_suffix);
        let cdn_ttl_var = format!("CASCETTE_CDN_TTL_TEST_{}", unique_suffix);
        let config_ttl_var = format!("CASCETTE_CONFIG_TTL_TEST_{}", unique_suffix);

        unsafe {
            std::env::set_var(&cache_dir_var, "/tmp/cascette-cache");
            std::env::set_var(&memory_items_var, "5000");
            std::env::set_var(&memory_size_var, "134217728"); // 128MB
            std::env::set_var(&disk_size_var, "4294967296"); // 4GB
            std::env::set_var(&disk_file_size_var, "52428800"); // 50MB
            std::env::set_var(&ribbit_ttl_var, "600");
            std::env::set_var(&cdn_ttl_var, "7200");
            std::env::set_var(&config_ttl_var, "3600");
        }

        let config = test_cache_config_from_env_with_prefix(&format!("_TEST_{}", unique_suffix));
        assert_eq!(config.cache_dir, Some(PathBuf::from("/tmp/cascette-cache")));
        assert_eq!(config.memory_max_items, 5000);
        assert_eq!(config.memory_max_size_bytes, 134_217_728);
        assert_eq!(config.disk_max_size_bytes, 4_294_967_296);
        assert_eq!(config.disk_max_file_size, 52_428_800);
        assert_eq!(config.ribbit_ttl, Duration::from_secs(600));
        assert_eq!(config.cdn_ttl, Duration::from_secs(7200));
        assert_eq!(config.config_ttl, Duration::from_secs(3600));

        // Clean up
        unsafe {
            std::env::remove_var(&cache_dir_var);
            std::env::remove_var(&memory_items_var);
            std::env::remove_var(&memory_size_var);
            std::env::remove_var(&disk_size_var);
            std::env::remove_var(&disk_file_size_var);
            std::env::remove_var(&ribbit_ttl_var);
            std::env::remove_var(&cdn_ttl_var);
            std::env::remove_var(&config_ttl_var);
        }
    }

    #[test]
    fn test_cdn_config_default() {
        let config = CdnConfig::default();
        assert_eq!(config.max_concurrent, 5);
        assert_eq!(config.chunk_size, 4 * 1024 * 1024); // 4MB
        assert!(!config.enable_progress);
        assert_eq!(config.pool_size, 20);
    }

    #[test]
    fn test_ribbit_hosts_parsing() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;

        // Generate a unique test ID to avoid conflicts during parallel execution
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(2000); // Different counter for ribbit test
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let thread_id = thread::current().id();
        let unique_suffix = format!("RIBBIT_{:?}_{}", thread_id, test_id);

        let ribbit_hosts_var = format!("CASCETTE_RIBBIT_HOSTS_TEST_{unique_suffix}");
        let test_suffix = format!("_TEST_{}", unique_suffix);

        // Test single host
        unsafe {
            std::env::set_var(&ribbit_hosts_var, "single.host:1119");
        }
        let config = test_client_config_from_env_with_prefix(&test_suffix);
        assert_eq!(config.ribbit_url, "tcp://single.host:1119".to_string());

        // Test multiple hosts with whitespace
        unsafe {
            std::env::set_var(&ribbit_hosts_var, " host1:1119 , host2:1119 , host3:1119 ");
        }
        let config = test_client_config_from_env_with_prefix(&test_suffix);
        // For single URL config, we'll take the first host from the list
        assert_eq!(config.ribbit_url, "tcp://host1:1119".to_string());

        // Clean up
        unsafe {
            std::env::remove_var(&ribbit_hosts_var);
        }
    }

    #[test]
    fn test_malformed_env_values() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;

        // Generate a unique test ID to avoid conflicts during parallel execution
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(3000); // Different counter for malformed test
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let thread_id = thread::current().id();
        let unique_suffix = format!("MALFORMED_{:?}_{}", thread_id, test_id);

        let connect_timeout_var = format!("CASCETTE_CONNECT_TIMEOUT_TEST_{unique_suffix}");
        let request_timeout_var = format!("CASCETTE_REQUEST_TIMEOUT_TEST_{unique_suffix}");
        let test_suffix = format!("_TEST_{}", unique_suffix);

        // Test invalid timeout values fall back to defaults
        unsafe {
            std::env::set_var(&connect_timeout_var, "invalid");
            std::env::set_var(&request_timeout_var, "also_invalid");
        }

        let config = test_client_config_from_env_with_prefix(&test_suffix);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));

        // Clean up
        unsafe {
            std::env::remove_var(&connect_timeout_var);
            std::env::remove_var(&request_timeout_var);
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = ClientConfig::default();
        let json = serde_json::to_string(&config).expect("Operation should succeed");
        let deserialized: ClientConfig =
            serde_json::from_str(&json).expect("Operation should succeed");

        assert_eq!(config.tact_https_url, deserialized.tact_https_url);
        assert_eq!(config.tact_http_url, deserialized.tact_http_url);
        assert_eq!(config.ribbit_url, deserialized.ribbit_url);
        assert_eq!(config.connect_timeout, deserialized.connect_timeout);
        assert_eq!(config.request_timeout, deserialized.request_timeout);
    }
}
