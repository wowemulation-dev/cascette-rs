//! Transport layer abstractions for protocol operations
//!
//! This module provides optimized HTTP transport with focus on:
//! - Fast client initialization
//! - Efficient connection pooling
//! - Optimized timeouts for NGDP workloads

use crate::error::Result;
use reqwest::{Client, ClientBuilder};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Global shared HTTP client for optimal performance
/// Avoids costly client creation on every protocol operation
static GLOBAL_HTTP_CLIENT: OnceLock<Arc<Client>> = OnceLock::new();

/// HTTP transport client with optimized connection pooling
#[derive(Clone)]
pub struct HttpClient {
    client: Arc<Client>,
}

impl HttpClient {
    /// Create a new HTTP client with performance-optimized configuration
    /// Uses global shared client to avoid expensive initialization
    pub fn new() -> Result<Self> {
        let client = GLOBAL_HTTP_CLIENT.get_or_init(|| {
            Arc::new(Self::create_optimized_client().unwrap_or_else(|_| {
                // Fallback to basic client if optimized creation fails
                Client::new()
            }))
        });

        Ok(Self {
            client: Arc::clone(client),
        })
    }

    /// Create optimized client with NGDP-specific settings
    #[cfg(not(target_arch = "wasm32"))]
    fn create_optimized_client() -> Result<Client> {
        ClientBuilder::new()
            // Connection pooling optimized for NGDP traffic patterns
            .pool_idle_timeout(Duration::from_secs(30)) // Shorter timeout for protocol requests
            .pool_max_idle_per_host(10) // Moderate pooling to reduce memory usage
            // Timeouts optimized for NGDP response patterns
            .timeout(Duration::from_secs(45)) // Reasonable timeout for Ribbit/CDN
            .connect_timeout(Duration::from_secs(10)) // Fast connect timeout
            // Network optimizations
            .tcp_nodelay(true) // Disable Nagle for low-latency
            .tcp_keepalive(Duration::from_secs(60)) // Keep connections alive
            // TLS - use rustls for security and WASM compatibility
            .use_rustls_tls()
            .https_only(false) // Allow HTTP for some NGDP endpoints
            // HTTP/2 optimization - don't assume prior knowledge
            .http2_adaptive_window(true) // Adaptive HTTP/2 flow control
            // Compression - enable for protocol responses
            .gzip(true)
            .brotli(true)
            .deflate(true)
            // Redirect handling for CDN
            .redirect(reqwest::redirect::Policy::limited(3))
            // User agent for NGDP traffic
            .user_agent("cascette-protocol/0.1.0")
            .build()
            .map_err(Into::into)
    }

    /// Create optimized client for WASM (browser environment)
    ///
    /// WASM uses the browser's Fetch API which doesn't support:
    /// - TCP-level options (tcp_nodelay, tcp_keepalive)
    /// - Connection pooling settings (pool_idle_timeout)
    /// - HTTP/2 specific settings (http2_adaptive_window, http2_prior_knowledge)
    /// - Custom redirect policies (handled by browser)
    /// - Timeout (handled differently in browsers)
    /// - https_only (browser manages this)
    /// - Compression settings (handled by browser)
    #[cfg(target_arch = "wasm32")]
    fn create_optimized_client() -> Result<Client> {
        ClientBuilder::new()
            // User agent for NGDP traffic
            .user_agent("cascette-protocol/0.1.0")
            .build()
            .map_err(Into::into)
    }

    /// Create a new HTTP client with custom configuration
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_config(config: &HttpConfig) -> Result<Self> {
        let mut builder = ClientBuilder::new()
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .tcp_nodelay(config.tcp_nodelay)
            .tcp_keepalive(config.tcp_keepalive);

        // TLS - always use rustls for security and WASM compatibility
        builder = builder.use_rustls_tls();

        // HTTP/2 configuration
        if config.http2_prior_knowledge {
            builder = builder.http2_prior_knowledge();
        } else {
            builder = builder.http2_adaptive_window(true);
        }

        // Compression configuration
        if config.enable_compression {
            builder = builder.gzip(true).brotli(true).deflate(true);
        }

        let client = builder.build()?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Create a new HTTP client with custom configuration for WASM
    ///
    /// Most HttpConfig options are ignored on WASM as they're not supported
    /// by the browser's Fetch API. The browser manages compression, connection
    /// pooling, and other network optimizations automatically.
    #[cfg(target_arch = "wasm32")]
    #[allow(unused_variables)] // config fields are not used on WASM
    pub fn with_config(config: &HttpConfig) -> Result<Self> {
        let builder = ClientBuilder::new();
        let client = builder.build()?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Get the underlying reqwest client
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Create a performance-optimized client for high-throughput scenarios
    pub fn high_performance() -> Result<Self> {
        let config = HttpConfig::high_performance();
        Self::with_config(&config)
    }

    /// Create a memory-optimized client for resource-constrained environments
    pub fn memory_optimized() -> Result<Self> {
        let config = HttpConfig::memory_optimized();
        Self::with_config(&config)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        // expect_used: HttpClient::new() only fails if reqwest Client::builder().build()
        // fails, which should never happen with default settings.
        Self::new().expect("HttpClient creation failed")
    }
}

/// HTTP client configuration with performance tuning options
///
/// Note: On WASM, many of these options are ignored as they're not supported
/// by the browser's Fetch API. The configuration is kept consistent across
/// platforms for API compatibility.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Connection pool idle timeout
    /// (ignored on WASM - browser manages connections)
    pub pool_idle_timeout: Duration,

    /// Maximum idle connections per host
    /// (ignored on WASM - browser manages connections)
    pub pool_max_idle_per_host: usize,

    /// Request timeout
    /// (ignored on WASM - use AbortController in application code)
    pub timeout: Duration,

    /// Connection timeout
    /// (ignored on WASM - browser manages connections)
    pub connect_timeout: Duration,

    /// Enable `TCP_NODELAY` (disable Nagle's algorithm)
    /// (ignored on WASM - no TCP access)
    pub tcp_nodelay: bool,

    /// TCP keep-alive duration
    /// (ignored on WASM - no TCP access)
    pub tcp_keepalive: Option<Duration>,

    /// Use HTTP/2 prior knowledge (faster but less compatible)
    /// (ignored on WASM - browser negotiates protocol)
    pub http2_prior_knowledge: bool,

    /// Enable compression (gzip, brotli, deflate)
    /// (supported on WASM)
    pub enable_compression: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            pool_idle_timeout: Duration::from_secs(30),
            pool_max_idle_per_host: 10,
            timeout: Duration::from_secs(45),
            connect_timeout: Duration::from_secs(10),
            tcp_nodelay: true,
            tcp_keepalive: Some(Duration::from_secs(60)),
            http2_prior_knowledge: false, // More compatible default
            enable_compression: true,     // Compress protocol responses
        }
    }
}

impl HttpConfig {
    /// Configuration optimized for high-throughput NGDP workloads
    pub fn high_performance() -> Self {
        Self {
            pool_idle_timeout: Duration::from_secs(60),
            pool_max_idle_per_host: 50, // More connections for throughput
            timeout: Duration::from_secs(120),
            connect_timeout: Duration::from_secs(5), // Fast connect for high perf
            tcp_nodelay: true,
            tcp_keepalive: Some(Duration::from_secs(30)),
            http2_prior_knowledge: true, // Assume HTTP/2 support
            enable_compression: true,
        }
    }

    /// Configuration optimized for memory-constrained environments
    pub fn memory_optimized() -> Self {
        Self {
            pool_idle_timeout: Duration::from_secs(15),
            pool_max_idle_per_host: 2, // Minimal connections
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            tcp_nodelay: false,  // Allow Nagle to batch small requests
            tcp_keepalive: None, // No keep-alive to save memory
            http2_prior_knowledge: false,
            enable_compression: false, // Disable compression to save CPU/memory
        }
    }
}
