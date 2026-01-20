//! # cascette-protocol - High-Performance NGDP/CASC Network Protocol Implementation
//!
//! This crate provides a production-ready networking layer for NGDP (Next Generation Distribution
//! Pipeline) and CASC (Content Addressable Storage Container) operations used by Blizzard Entertainment
//! games like World of Warcraft.
//!
//! ## Architecture Overview
//!
//! The crate implements a three-tier architecture:
//!
//! 1. **Unified Protocol Client** ([`RibbitTactClient`]): Automatic fallback between TACT HTTPS/HTTP and Ribbit TCP
//! 2. **CDN Content Delivery** ([`CdnClient`]): Efficient content downloads with range requests and progress tracking
//! 3. **Performance Layer**: High-performance caching, memory management, and network optimizations
//!
//! ## Performance Features
//!
//! ### Network Optimizations
//! - **Global HTTP Client**: Shared client instance with optimized connection pooling
//! - **HTTP/2 Support**: Multiplexed connections with automatic fallback to HTTP/1.1
//! - **Connection Reuse**: Persistent connections with configurable keepalive
//! - **Request Retry**: Exponential backoff with jitter for resilient operations
//!
//! ### Memory Management
//! - **Thread-Local Buffers**: Reduce allocations for frequently used operations
//! - **String Interning**: Zero-copy sharing of common protocol strings
//! - **Memory Pools**: Automatic buffer pooling for different size classes
//! - **LRU Cache Eviction**: Intelligent memory management for cache entries
//!
//! ### Protocol Optimizations
//! - **Cache-First**: All protocol responses cached with appropriate TTLs
//! - **Pre-computed Hashes**: Fast lookups for common NGDP endpoints
//! - **Zero-Copy Parsing**: Minimal allocations during protocol parsing
//! - **Batch Operations**: Efficient handling of concurrent requests
//!
//! ## Protocol Support
//!
//! ### TACT (Tooling And Content Technology)
//! - **HTTPS Primary**: Secure connections to `us.version.battle.net`
//! - **HTTP Fallback**: Fallback protocol if HTTPS fails
//! - **Response Caching**: Automatic caching of BPSV responses
//!
//! ### Ribbit TCP Protocol
//! - **Direct TCP**: Low-level TCP connections to `*.version.battle.net:1119`
//! - **Final Fallback**: Used when HTTP protocols fail
//! - **Multiple Hosts**: Automatic failover between regional hosts
//!
//! ### CDN Content Delivery
//! - **Range Requests**: Partial content downloads for efficiency
//! - **Progress Tracking**: Real-time download progress callbacks
//! - **Concurrent Downloads**: Multiple simultaneous content streams
//! - **Dependency Injection**: CDN endpoints from external configuration
//!
//! ## Usage Examples
//!
//! ### Basic Protocol Query
//!
//! Query version information with automatic fallback:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, ClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::default();
//!     let client = RibbitTactClient::new(config)?;
//!
//!     // Query with automatic protocol fallback and caching
//!     let versions = client.query("v1/products/wow/versions").await?;
//!
//!     println!("Found {} WoW versions", versions.rows().len());
//!     for row in versions.rows() {
//!         if let Some(version_name) = row.get_by_name("VersionsName", versions.schema()) {
//!             if let Some(build_id) = row.get_by_name("BuildId", versions.schema()) {
//!                 println!("Version: {} (Build: {})",
//!                     version_name.as_string().unwrap_or("unknown"),
//!                     build_id.as_string().unwrap_or("unknown"));
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Production Configuration
//!
//! Configure for high-throughput production workloads:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, ClientConfig, CacheConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig {
//!         cache_config: CacheConfig::production(), // 1GB memory, 32GB disk
//!         connect_timeout: Duration::from_secs(5),  // Fast failure detection
//!         request_timeout: Duration::from_secs(120), // Long requests for large responses
//!         ..Default::default()
//!     };
//!     let client = RibbitTactClient::new(config)?;
//!
//!     // High-throughput operations with optimized caching
//!     let versions = client.query("v1/products/wow/versions").await?;
//!
//!     // Monitor cache performance
//!     let stats = client.cache().stats()?;
//!     println!("Cache hit rate: {:.1}%", stats.hit_rate());
//!     println!("Memory usage: {} MB", stats.memory_usage / 1024 / 1024);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### CDN Content Download
//!
//! Download content with progress tracking and range requests:
//!
//! ```rust,no_run
//! use cascette_protocol::{CdnClient, CdnEndpoint, ContentType};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = Arc::new(cascette_protocol::cache::ProtocolCache::new(
//!         &cascette_protocol::config::CacheConfig::default()
//!     )?);
//!
//!     let cdn_client = CdnClient::new(cache, Default::default())?;
//!
//!     // CDN endpoint typically obtained from Ribbit query results
//!     let endpoint = CdnEndpoint {
//!         host: "level3.blizzard.com".to_string(),
//!         path: "tpr/wow".to_string(),
//!         product_path: None, // Optional for newer products
//!         scheme: None, // Optional, defaults to HTTPS
//!     };
//!
//!     // Download content by key with progress tracking
//!     let key = hex::decode("abcdef1234567890abcdef1234567890")?;
//!     let data = cdn_client.download_with_progress(
//!         &endpoint,
//!         ContentType::Data,
//!         &key,
//!         |downloaded, total| {
//!             if total > 0 {
//!                 let progress = (downloaded as f64 / total as f64) * 100.0;
//!                 println!("Download progress: {:.1}%", progress);
//!             }
//!         }
//!     ).await?;
//!
//!     println!("Downloaded {} bytes", data.len());
//!     Ok(())
//! }
//! ```
//!
//! ### Complete NGDP Workflow
//!
//! Example of a complete NGDP operation chain:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, CdnClient, ClientConfig, ContentType};
//! use cascette_formats::CascFormat;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize with environment-based configuration
//!     let config = ClientConfig::from_env()?;
//!     let client = RibbitTactClient::new(config)?;
//!
//!     // 1. Query version information
//!     let versions = client.query("v1/products/wow/versions").await?;
//!     println!("Available versions: {}", versions.rows().len());
//!
//!     // 2. Query CDN configuration
//!     let cdns = client.query("v1/products/wow/cdns").await?;
//!
//!     // 3. Extract CDN endpoint from response
//!     let cdn_endpoint = CdnClient::endpoint_from_bpsv_row(
//!         cdns.rows().first().ok_or("No CDN configurations found")?,
//!         cdns.schema()
//!     )?;
//!
//!     // 4. Initialize CDN client
//!     let cdn_client = CdnClient::new(client.cache().clone(), Default::default())?;
//!
//!     // 5. Download build configuration
//!     let latest_version = versions.rows().first().ok_or("No versions found")?;
//!     let build_config_hash = latest_version
//!         .get_by_name("BuildConfig", versions.schema())
//!         .and_then(|v| v.as_string())
//!         .ok_or("Missing BuildConfig field")?;
//!
//!     let build_config_key = hex::decode(build_config_hash)?;
//!     let config_data = cdn_client.download(
//!         &cdn_endpoint,
//!         ContentType::Config,
//!         &build_config_key
//!     ).await?;
//!
//!     println!("Downloaded build config: {} bytes", config_data.len());
//!
//!     // Config data can now be parsed with cascette-formats
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! ### Environment Variables
//!
//! The client can be configured via environment variables:
//!
//! ```bash
//! # Protocol endpoints
//! export CASCETTE_TACT_HTTPS_URL="https://us.version.battle.net"
//! export CASCETTE_TACT_HTTP_URL="http://us.version.battle.net"
//! export CASCETTE_RIBBIT_HOSTS="us.version.battle.net:1119,eu.version.battle.net:1119"
//!
//! # Cache settings
//! export CASCETTE_CACHE_DIR="/var/cache/cascette"
//! export CASCETTE_MEMORY_MAX_SIZE="268435456"  # 256MB
//! export CASCETTE_DISK_MAX_SIZE="8589934592"   # 8GB
//!
//! # Network timeouts
//! export CASCETTE_CONNECT_TIMEOUT="10"
//! export CASCETTE_REQUEST_TIMEOUT="30"
//! ```
//!
//! ### Configuration Profiles
//!
//! Predefined configuration profiles for common scenarios:
//!
//! ```rust,no_run
//! use cascette_protocol::{ClientConfig, CacheConfig};
//!
//! // High-performance production settings
//! let prod_config = ClientConfig {
//!     cache_config: CacheConfig::production(),
//!     ..Default::default()
//! };
//!
//! // Memory-constrained environments
//! let memory_config = ClientConfig {
//!     cache_config: CacheConfig::memory_optimized(),
//!     ..Default::default()
//! };
//!
//! // Environment-based configuration
//! let env_config = ClientConfig::from_env()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error handling with automatic retries:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, ProtocolError, ClientConfig};
//!
//! async fn handle_errors(client: &RibbitTactClient) -> Result<(), ProtocolError> {
//!     match client.query("v1/products/wow/versions").await {
//!         Ok(versions) => {
//!             println!("Successfully retrieved {} versions", versions.rows().len());
//!         }
//!         Err(ProtocolError::AllHostsFailed) => {
//!             // All protocol attempts failed - check network connectivity
//!             eprintln!("Could not reach any Blizzard servers");
//!         }
//!         Err(ProtocolError::RateLimited) => {
//!             // Rate limited - implement backoff
//!             tokio::time::sleep(std::time::Duration::from_secs(60)).await;
//!         }
//!         Err(ProtocolError::Parse(msg)) => {
//!             // Invalid response format
//!             eprintln!("Response parsing failed: {}", msg);
//!         }
//!         Err(e) if e.should_retry() => {
//!             // Retryable error - the client will automatically retry
//!             eprintln!("Transient error (will retry): {}", e);
//!         }
//!         Err(e) => {
//!             eprintln!("Permanent error: {}", e);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Performance Monitoring
//!
//! Monitor performance and cache efficiency:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, ClientConfig};
//!
//! async fn monitor_performance(client: &RibbitTactClient) -> Result<(), Box<dyn std::error::Error>> {
//!     // Get cache statistics
//!     let stats = client.cache().stats()?;
//!     println!("Cache Statistics:");
//!     println!("  Hit rate: {:.1}%", stats.hit_rate());
//!     println!("  Memory usage: {} MB", stats.memory_usage / 1024 / 1024);
//!     println!("  Total entries: {}", stats.entries);
//!
//!     // Cleanup expired entries
//!     let cleanup_count = client.cache().cleanup_expired()?;
//!     println!("Cleaned up {} expired entries", cleanup_count);
//!
//!     // Monitor hit rate for optimization
//!     if stats.hit_rate() < 50.0 {
//!         println!("Warning: Low cache hit rate - consider increasing TTL values");
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Integration with Other Crates
//!
//! This crate is designed to work seamlessly with other cascette-rs crates:
//!
//! - [`cascette-formats`]: Parse BPSV responses and CASC data formats
//! - [`cascette-cache`]: Advanced caching with persistent storage
//! - [`cascette-crypto`]: Cryptographic operations for CASC content
//!
//! For complete examples and integration patterns, see the crate documentation.
//!
//! ## Thread Safety
//!
//! All client types are thread-safe and can be shared across async tasks:
//!
//! ```rust,no_run
//! use cascette_protocol::{RibbitTactClient, ClientConfig};
//! use std::sync::Arc;
//! use tokio::task::JoinSet;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Arc::new(RibbitTactClient::new(ClientConfig::default())?);
//!     let mut tasks = JoinSet::new();
//!
//!     // Spawn concurrent queries
//!     for product in ["wow", "wowt", "wowdev"] {
//!         let client = Arc::clone(&client);
//!         let product = product.to_string();
//!
//!         tasks.spawn(async move {
//!             let endpoint = format!("v1/products/{}/versions", product);
//!             client.query(&endpoint).await
//!         });
//!     }
//!
//!     // Collect results
//!     while let Some(result) = tasks.join_next().await {
//!         match result? {
//!             Ok(versions) => println!("Retrieved {} versions", versions.rows().len()),
//!             Err(e) => eprintln!("Query failed: {}", e),
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod cache;
pub mod cdn;
pub mod client;
pub mod config;
pub mod error;
pub mod mime_parser;
pub mod optimized;
pub mod retry;
pub mod transport;
pub mod v1_mime;

// Re-export main types
pub use cdn::{CdnClient, CdnEndpoint, ContentType};
pub use client::RibbitTactClient;
pub use config::{CacheConfig, CdnConfig, ClientConfig};
pub use error::{ProtocolError, Result};
pub use retry::RetryPolicy;
pub use transport::{HttpClient, HttpConfig};

// Re-export internal client types for advanced usage
pub use client::{RibbitClient, TactClient};

// Re-export optimization utilities for power users
pub use optimized::{PooledBuffer, format_cache_key, get_buffer, intern_string, return_buffer};
