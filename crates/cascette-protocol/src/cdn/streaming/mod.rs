//! Streaming CDN architecture for efficient content delivery
//!
//! This module implements Phase 1 of the streaming CDN system, providing core
//! infrastructure for HTTP-based streaming of CASC archive content with optimal
//! bandwidth usage and memory efficiency.
//!
//! # Architecture Overview
//!
//! The streaming system is built around these core components:
//!
//! - **HTTP Client Abstraction**: Trait-based HTTP client supporting range requests
//! - **Range Request Management**: Efficient handling and coalescing of byte ranges
//! - **Streaming Configuration**: Tunable options for performance and reliability
//! - **Error Handling**: Detailed error context with recovery suggestions
//!
//! # Usage Examples
//!
//! ## Basic CDN Content Streaming
//!
//! ```rust
//! # #[cfg(feature = "streaming")]
//! # {
//! use cascette_protocol::cdn::streaming::{
//!     StreamingConfig, ReqwestHttpClient, HttpClient, HttpRange, RangeCoalescer,
//!     CdnServer, ContentType, CdnUrlBuilder, CdnBootstrap
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Example 1: Bootstrap from Ribbit response
//! let ribbit_data = b"Name!STRING:0|Path!STRING:0|Hosts!STRING:0\n\nwow|tpr/wow|level3.blizzard.com";
//! let bootstrap = CdnBootstrap::from_ribbit_response(ribbit_data, Some("wow"))?;
//!
//! let client = ReqwestHttpClient::from_bootstrap(
//!     StreamingConfig::default(),
//!     &bootstrap,
//! )?;
//!
//! // Example 2: Direct configuration with fallback
//! let client = ReqwestHttpClient::with_fallback(
//!     StreamingConfig::default()
//! )?;
//!
//! // Get content with automatic CDN failover
//! let data = client.get_cdn_content(
//!     "wow",
//!     ContentType::Data,
//!     "1234567890abcdef1234567890abcdef",
//!     Some(HttpRange::new(0, 1023)),
//!     true, // prefer HTTPS
//! ).await?;
//!
//! // Create client with updated bootstrap when new Ribbit data arrives
//! let new_bootstrap = CdnBootstrap::from_ribbit_response(ribbit_data, Some("wowt"))?;
//! let updated_client = ReqwestHttpClient::from_bootstrap(
//!     StreamingConfig::default(),
//!     &new_bootstrap,
//! )?;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Complete Content Resolution
//!
//! ```rust
//! # #[cfg(feature = "streaming")]
//! # {
//! use cascette_protocol::cdn::streaming::{
//!     StreamingCdnResolver, CdnResolutionConfig, ContentResolutionRequest,
//!     ReqwestHttpClient, StreamingConfig
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create HTTP client and resolver
//! let http_client = ReqwestHttpClient::with_fallback(StreamingConfig::default())?;
//! let mut resolver = StreamingCdnResolver::with_defaults(http_client);
//!
//! // Resolve content by encoding key
//! let encoding_key = hex::decode("1234567890abcdef1234567890abcdef")?;
//! let content = resolver.resolve_content(&encoding_key, None).await?;
//!
//! println!("Resolved {} bytes from {}", content.size, content.archive_url);
//!
//! // Batch resolution for multiple files
//! let requests = vec![
//!     ContentResolutionRequest {
//!         encoding_key: hex::decode("11111111111111111111111111111111")?,
//!         expected_size: None,
//!         decompress: true,
//!     },
//!     ContentResolutionRequest {
//!         encoding_key: hex::decode("22222222222222222222222222222222")?,
//!         expected_size: Some(2048),
//!         decompress: true,
//!     },
//! ];
//!
//! let results = resolver.resolve_multiple(requests, None).await?;
//! println!("Resolved {} files", results.len());
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Streaming BLTE Decompression
//!
//! ```rust
//! # #[cfg(feature = "streaming")]
//! # {
//! use cascette_protocol::cdn::streaming::{
//!     StreamingBlteProcessor, StreamingBlteConfig,
//!     ReqwestHttpClient, StreamingConfig
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create BLTE processor
//! let http_client = ReqwestHttpClient::with_fallback(StreamingConfig::default())?;
//! let blte_processor = StreamingBlteProcessor::with_defaults(http_client);
//!
//! // Decompress BLTE content from URL without loading entire file
//! let decompressed = blte_processor
//!     .decompress_from_url("https://test-cdn.example.com/data/12/34/1234567890abcdef.blte", None)
//!     .await?;
//!
//! // Get header information without decompression
//! let header_info = blte_processor
//!     .get_header_info("https://test-cdn.example.com/data/12/34/1234567890abcdef.blte")
//!     .await?;
//!
//! println!("BLTE file has {} chunks, {} total decompressed size",
//!     header_info.chunk_count, header_info.total_decompressed_size);
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Advanced Connection Pool Management
//!
//! ```rust
//! # #[cfg(feature = "streaming")]
//! # {
//! use cascette_protocol::cdn::streaming::{
//!     ConnectionPool, ConnectionPoolConfig, ReqwestHttpClient, StreamingConfig, CdnServer
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create connection pool with health checking
//! let pool_config = ConnectionPoolConfig {
//!     max_total_connections: 1000,
//!     max_connections_per_host: 50,
//!     keep_alive_timeout: std::time::Duration::from_secs(30),
//!     tcp_keepalive: true,
//!     tcp_keepalive_interval: std::time::Duration::from_secs(60),
//!     enable_pooling: true,
//! };
//!
//! let pool: ConnectionPool<ReqwestHttpClient> = ConnectionPool::new(pool_config);
//!
//! // Add servers to pool
//! let server = CdnServer::https("level3.blizzard.com".to_string());
//! let client = ReqwestHttpClient::new(StreamingConfig::default())?;
//! pool.add_client(&server, client);
//!
//! // Get client with connection limiting
//! let client = pool.get_client(&server).await?;
//!
//! // Record request result for health tracking
//! pool.record_result(&server, true, std::time::Duration::from_millis(100)).await;
//!
//! // Get connection statistics
//! let stats = pool.get_stats(&server).await;
//! if let Some(stats) = stats {
//!     println!("Success rate: {:.2}%", stats.success_rate() * 100.0);
//! }
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! # Performance Characteristics
//!
//! ## Phase 1 & 2 Features
//! - **Memory Usage**: O(content_size) instead of O(archive_size)
//! - **Bandwidth Efficiency**: 90%+ reduction through range coalescing
//! - **Concurrent Connections**: Configurable pool with per-host limits
//! - **Request Optimization**: Automatic range merging and splitting
//! - **BLTE Streaming**: Progressive decompression without full file buffering
//! - **Archive Extraction**: Parallel content extraction from multiple archives
//! - **CDN Failover**: Automatic retry with backup servers and mirrors
//!
//! ## Phase 3 Advanced Features
//! - **Connection Pool Management**: Health checking with automatic cleanup
//! - **Circuit Breaker Pattern**: Automatic server isolation and recovery
//! - **Bandwidth Monitoring**: Adaptive optimization based on network conditions
//! - **Priority Queuing**: Request prioritization with backpressure control
//! - **Advanced Error Recovery**: Exponential backoff with intelligent failover
//! - **Metrics**: Prometheus export for monitoring
//! - **Zero-Copy Optimizations**: Buffer pooling and memory-efficient streaming
//! - **Network Condition Adaptation**: Dynamic adjustment to network quality
//!
//! ## Production Performance Targets
//! - **Memory**: ≤ 64MB + content size for 100GB+ archives
//! - **Latency**: First byte available within 100ms
//! - **Reliability**: 99.9% success rate with proper fallback
//! - **Concurrency**: Support 1000+ concurrent connections
//! - **Efficiency**: 90%+ bandwidth reduction vs full downloads

pub mod archive;
pub mod blte;
pub mod bootstrap;
pub mod config;
pub mod error;
pub mod http;
pub mod integration;
pub mod path;
pub mod range;

// Phase 3: Advanced streaming features
pub mod metrics;
pub mod optimizer;
pub mod pool;
pub mod recovery;

// Re-export public types for convenience
pub use archive::{
    ArchiveExtractionRequest, ArchiveExtractionResult, BatchArchiveExtractor,
    StreamingArchiveConfig, StreamingArchiveReader,
};
pub use blte::{BlteHeaderInfo, StreamingBlteConfig, StreamingBlteProcessor};
pub use bootstrap::{BootstrapStats, CdnBootstrap, CdnEntry};
pub use config::{CdnConfig, ConnectionPoolConfig, RetryConfig, StreamingConfig};
pub use error::{StreamingError, StreamingResult};
pub use http::{CdnServer, HttpClient, ReqwestHttpClient};
pub use integration::{
    BatchContentResolver, CacheStats, CdnResolutionConfig, ContentResolutionRequest,
    ContentResolutionResult, StreamingCdnResolver,
};
pub use path::{CdnPathCache, CdnUrlBuilder, ContentType};
pub use range::{HttpRange, MultiRangeRequest, RangeCoalescer};

// Phase 3: Advanced streaming feature exports
pub use metrics::{
    CacheStats as MetricsCacheStats, PoolMetrics, PrometheusExporter, StreamingMetrics,
};
pub use optimizer::{
    AdvancedRangeCoalescer, BandwidthMonitor, PrioritizedRequest, PriorityRequestQueue,
    RequestPriority, ZeroCopyBuffer,
};
pub use pool::{ConnectionPool, ConnectionState, ConnectionStats};
pub use recovery::{
    ErrorRecoverySystem, FailoverManager, NetworkCondition, NetworkConditionDetector,
    RecoveryContext, RecoveryStatistics, RecoveryStrategy, RetryManager, ServerHealth,
    ServerMetrics,
};

#[cfg(test)]
#[allow(
    clippy::similar_names,
    clippy::float_cmp,
    clippy::no_effect_underscore_binding,
    clippy::used_underscore_binding,
    clippy::expect_used
)]
mod integration_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_streaming_module_imports() {
        // Verify all public types can be imported
        let _config = StreamingConfig::default();
        let _range = HttpRange::new(0, 1023);
        let _cdn_server = CdnServer::https("example.com".to_string());
        let _url_builder = CdnUrlBuilder::new();
        let _content_type = ContentType::Data;
        let _bootstrap = CdnBootstrap::new();
        // Test client creation with configuration
        let _config = StreamingConfig::default();
        let _result = ReqwestHttpClient::new(_config);
    }

    #[tokio::test]
    async fn test_basic_streaming_workflow() {
        let config = StreamingConfig::default();

        // Test HTTP client creation
        let client_result = ReqwestHttpClient::new(config.clone());
        assert!(client_result.is_ok());

        // Test HTTP client with CDN servers
        let cdn_servers = vec![
            CdnServer::https("example.com".to_string()),
            CdnServer::http("backup.com".to_string()),
        ];
        let cdn_client_result = ReqwestHttpClient::with_cdn_servers(config.clone(), cdn_servers);
        assert!(cdn_client_result.is_ok());

        // Test range coalescing
        let coalescer = RangeCoalescer::new(config.clone());
        let ranges = vec![HttpRange::new(0, 100), HttpRange::new(150, 250)];

        let optimized_ranges_result = coalescer.coalesce(ranges);
        assert!(optimized_ranges_result.is_ok());

        // Test Phase 3 components
        let bandwidth_monitor = std::sync::Arc::new(BandwidthMonitor::new(Duration::from_secs(60)));
        let advanced_coalescer = AdvancedRangeCoalescer::new(config, bandwidth_monitor);

        let test_ranges = vec![HttpRange::new(0, 1023), HttpRange::new(1030, 2047)];

        let advanced_result = advanced_coalescer.coalesce_ranges(test_ranges);
        assert!(advanced_result.is_ok());
    }

    #[test]
    fn test_error_handling_integration() {
        // Test error conversions and handling
        let error = StreamingError::InvalidRange {
            reason: "test error".to_string(),
        };

        assert!(!error.is_retryable());
        assert!(error.retry_delay_ms(0).is_none());

        let suggestion = error.recovery_suggestion();
        assert!(!suggestion.is_empty());

        // Test CDN-specific error handling
        let cdn_error = StreamingError::CdnFailover {
            server: "example.com".to_string(),
            source: Box::new(StreamingError::Timeout {
                timeout_ms: 5000,
                url: "http://example.com".to_string(),
            }),
        };
        assert!(cdn_error.is_retryable());
        assert!(cdn_error.retry_delay_ms(0).is_some());

        let failover_suggestion = cdn_error.cdn_failover_suggestion();
        assert!(failover_suggestion.is_none()); // CdnFailover itself doesn't have a suggestion
    }

    #[test]
    fn test_configuration_validation() {
        let config = StreamingConfig::default();
        assert!(config.validate().is_ok());

        let high_throughput = StreamingConfig::high_throughput();
        assert!(high_throughput.validate().is_ok());

        let low_memory = StreamingConfig::low_memory();
        assert!(low_memory.validate().is_ok());

        let unreliable = StreamingConfig::unreliable_network();
        assert!(unreliable.validate().is_ok());
    }

    #[test]
    fn test_cdn_url_construction() {
        let url_builder = CdnUrlBuilder::new();

        let url = url_builder
            .build_url(
                "level3.blizzard.com",
                "tpr/wow",
                ContentType::Data,
                "1234567890abcdef1234567890abcdef",
                true,
            )
            .expect("Operation should succeed");

        assert!(url.starts_with("https://level3.blizzard.com/tpr/wow/data/12/34/"));
        assert!(url.ends_with("1234567890abcdef1234567890abcdef"));
    }

    #[test]
    fn test_cdn_server_priority_sorting() {
        let servers = vec![
            CdnServer::new("low.com".to_string(), true, 200),
            CdnServer::new("high.com".to_string(), true, 50),
            CdnServer::new("medium.com".to_string(), true, 100),
        ];

        let config = StreamingConfig::default();
        let client =
            ReqwestHttpClient::with_cdn_servers(config, servers).expect("Operation should succeed");

        // Should be sorted by priority (lower = higher priority)
        let cdn_servers = client.cdn_servers();
        assert_eq!(cdn_servers[0].host, "high.com");
        assert_eq!(cdn_servers[1].host, "medium.com");
        assert_eq!(cdn_servers[2].host, "low.com");
    }

    #[tokio::test]
    #[allow(clippy::panic)]
    async fn test_cdn_error_handling() {
        let config = StreamingConfig::default();
        let client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        // Should fail with no CDN servers configured
        let result = client
            .get_cdn_content(
                "wow",
                ContentType::Data,
                "1234567890abcdef1234567890abcdef",
                None,
                true,
            )
            .await;

        assert!(result.is_err());
        if let Err(StreamingError::Configuration { reason }) = result {
            assert!(reason.contains("No CDN servers configured"));
        } else {
            unreachable!("Expected Configuration error");
        }
    }

    #[tokio::test]
    async fn test_phase3_metrics_integration() {
        let streaming_metrics = StreamingMetrics::new();
        let pool_metrics = PoolMetrics::new();

        // Test basic metrics recording
        streaming_metrics.record_download(1024, Duration::from_millis(100));
        streaming_metrics.record_cache_hit("test_cache");
        streaming_metrics.record_cache_miss("test_cache");

        assert_eq!(
            streaming_metrics
                .bytes_downloaded
                .load(std::sync::atomic::Ordering::Relaxed),
            1024
        );

        let cache_stats = streaming_metrics.cache_stats("test_cache");
        assert_eq!(
            cache_stats.hits.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            cache_stats
                .misses
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        // Test pool metrics
        pool_metrics
            .total_successful_requests
            .store(10, std::sync::atomic::Ordering::Relaxed);
        pool_metrics
            .total_failed_requests
            .store(2, std::sync::atomic::Ordering::Relaxed);

        assert_eq!(pool_metrics.total_requests(), 12);
        assert!((pool_metrics.success_rate() - 0.833).abs() < 0.01);
    }

    #[test]
    fn test_prometheus_exporter_creation() {
        let exporter_result = PrometheusExporter::new();
        assert!(exporter_result.is_ok());

        let exporter = exporter_result.expect("Operation should succeed");
        let output = exporter.gather();
        assert!(output.contains("cascette"));
    }

    #[test]
    fn test_phase3_type_creation() {
        // Test that all Phase 3 types can be created successfully
        let _bandwidth_monitor = BandwidthMonitor::new(Duration::from_secs(60));
        let _pool_metrics = PoolMetrics::new();
        let _streaming_metrics = StreamingMetrics::new();

        // Test recovery types
        let _health = ServerHealth::Healthy;
        let _condition = NetworkCondition::Good;
        let _connection_state = ConnectionState::Healthy;

        // Test request priority ordering
        let critical = RequestPriority::Critical;
        let normal = RequestPriority::Normal;
        assert!(critical < normal); // Lower enum value = higher priority
    }
}
