#![allow(clippy::literal_string_with_formatting_args)]
//! High-performance multi-layer caching for NGDP and CASC content distribution systems
//!
//! This crate provides a comprehensive caching solution designed specifically for
//! NGDP (Next Generation Distribution Pipeline) and CASC (Content Addressable Storage Container)
//! systems. It supports multiple cache layers, different eviction policies, and
//! provides detailed metrics for performance monitoring.
//!
//! # Features
//!
//! - **Multi-Layer Caching**: Support for hierarchical cache layers (L1, L2, L3, etc.)
//! - **Type-Safe Keys**: Strongly-typed cache keys for different data types
//! - **Async Operations**: Full async/await support for non-blocking cache operations
//! - **Flexible Eviction**: Multiple eviction policies (LRU, LFU, TTL, size-based)
//! - **Comprehensive Metrics**: Detailed performance and usage statistics
//! - **Memory Pooling**: Optimized memory allocation for NGDP file patterns
//! - **Thread-Safe**: Designed for high-concurrency NGDP server environments
//! - **Configurable**: Extensive configuration options with sensible defaults
//! - **Game-Optimized**: Cache access pattern analysis for game downloads
//!
//! # Architecture
//!
//! The caching system follows a layered architecture:
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │            Application              │
//! └─────────────────────────────────────┘
//!                  │
//! ┌─────────────────────────────────────┐
//! │         Cache Interface             │
//! │     (AsyncCache trait)              │
//! └─────────────────────────────────────┘
//!                  │
//! ┌─────────────────────────────────────┐
//! │        Cache Implementations       │
//! │  ┌─────┐ ┌──────┐ ┌─────────────┐   │
//! │  │ L1  │ │  L2  │ │ Multi-Layer │   │
//! │  │Mem  │ │ Disk │ │   Cache     │   │
//! │  └─────┘ └──────┘ └─────────────┘   │
//! └─────────────────────────────────────┘
//!                  │
//! ┌─────────────────────────────────────┐
//! │        Memory Pool System           │
//! │  ┌─────────────────────────────┐    │
//! │  │    NGDP-Optimized Pools     │    │
//! │  │  Small | Med | Large | Huge │    │
//! │  │  16KB  |256KB| 8MB  | 32MB │    │
//! │  └─────────────────────────────┘    │
//! └─────────────────────────────────────┘
//! ```
//!
//! # Memory Pool Optimization
//!
//! The cache system includes a sophisticated memory pooling system optimized
//! for NGDP file access patterns:
//!
//! - **Size Classes**: Separate pools for different NGDP file sizes
//!   - Small: Ribbit responses, config files (≤16KB)
//!   - Medium: Archive indices, patch manifests (16KB-256KB)
//!   - Large: Root files, install manifests (256KB-8MB)
//!   - Huge: Encoding files, large archives (8MB-32MB)
//!
//! - **Thread-Local Pools**: Lock-free allocation for hot paths
//! - **Cache-Aligned Storage**: Reduces false sharing between CPU cores
//! - **Burst Optimization**: Handles game patch traffic spikes efficiently
//!
//! # Usage Examples
//!
//! ## Basic Memory Cache with Pooling
//!
//! ```rust
//! use cascette_cache::{
//!     config::CacheConfig,
//!     key::RibbitKey,
//!     pool::{NgdpMemoryPool, NgdpSizeClass},
//!     traits::AsyncCache,
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create memory pool for optimal NGDP performance
//! let pool = NgdpMemoryPool::new();
//! pool.warm_up(); // Pre-allocate buffers for common sizes
//!
//! // Create a simple in-memory cache configuration
//! let config = CacheConfig::memory_only();
//!
//! // Cache key for Ribbit service discovery
//! let key = RibbitKey::new("summary", "us");
//! let data = bytes::Bytes::from("cached response data");
//!
//! // Use optimized allocation for NGDP file sizes
//! let buffer = pool.allocate(16 * 1024 * 1024); // Large file allocation
//! assert_eq!(NgdpSizeClass::from_size(buffer.capacity()), NgdpSizeClass::Huge);
//!
//! // Note: Actual cache implementation would be created here
//! // This is just showing the interface
//! # Ok(())
//! # }
//! ```
//!
//! ## Multi-Layer Cache Configuration with Memory Optimization
//!
//! ```rust
//! use cascette_cache::config::{
//!     MultiLayerCacheConfig,
//!     MemoryCacheConfig,
//!     DiskCacheConfig,
//!     CacheConfig,
//! };
//! use cascette_cache::pool::{allocate_thread_local, deallocate_thread_local};
//! use std::time::Duration;
//!
//! // Configure a multi-layer cache with memory (L1) and disk (L2) layers
//! let multi_layer = MultiLayerCacheConfig::new()
//!     .add_memory_layer(
//!         MemoryCacheConfig::new()
//!             .with_max_entries(10_000)
//!             .with_default_ttl(Duration::from_secs(300)) // 5 minutes
//!     )
//!     .add_disk_layer(
//!         DiskCacheConfig::new("/tmp/cache")
//!             .with_max_files(100_000)
//!             .with_default_ttl(Duration::from_secs(24 * 3600)) // 24 hours
//!     );
//!
//! let config = CacheConfig::multi_layer(multi_layer);
//!
//! // Use thread-local allocation for maximum performance
//! let fast_buffer = allocate_thread_local(16 * 1024); // 16KB config file
//! // ... use buffer ...
//! deallocate_thread_local(fast_buffer); // Return to thread pool
//! ```
//!
//! ## Cache Access Pattern Analysis
//!
//! ```rust
//! use cascette_cache::{
//!     game_optimized::{CacheAccessAnalyzer, AnalyzerConfig},
//! };
//!
//! // Create access pattern analyzer
//! let analyzer = CacheAccessAnalyzer::new(AnalyzerConfig::default());
//!
//! // Record cache access patterns
//! analyzer.record_access("manifest", true, 1024);
//! analyzer.record_access("content", false, 4096);
//!
//! // Get access pattern statistics
//! let patterns = analyzer.get_access_patterns();
//! for (key_type, stats) in patterns {
//!     println!("Key type: {}, Hit rate: {:.2}, Access count: {}",
//!              key_type, stats.hit_rate, stats.access_count);
//! }
//! ```
//!
//! ## Cache Key Types with Fast Hashing
//!
//! The crate provides specialized key types for different NGDP/CASC data
//! with optimized hashing and caching:
//!
//! ```rust
//! use cascette_cache::key::*;
//! use cascette_crypto::{ContentKey, EncodingKey};
//!
//! // Ribbit service discovery cache with pre-computed hashes
//! let ribbit_key = RibbitKey::with_product("builds", "eu", "wow");
//! let fast_hash = ribbit_key.fast_hash(); // 32-bit + 64-bit hashes
//! assert!(fast_hash.fast_eq(&ribbit_key.fast_hash()));
//!
//! // Configuration file cache with thread-local string formatting
//! let config_key = ConfigKey::new("buildconfig", "abc123def456");
//! let cached_string = config_key.as_cache_key(); // Cached after first call
//!
//! // BLTE content cache optimized for large files
//! let encoding_key = EncodingKey::from_data(b"encoded data");
//! let blte_key = BlteKey::new(encoding_key);
//!
//! // Content data cache with fast lookups
//! let content_key = ContentKey::from_data(b"content data");
//! let content_cache_key = ContentCacheKey::new(content_key);
//! ```
//!
//! # Performance Optimizations
//!
//! ## Cache Key Optimizations
//!
//! - **Pre-computed Hashing**: Jenkins96 hashes cached on first use
//! - **Thread-Local String Buffers**: Avoid allocations in key formatting
//! - **Fast Equality**: 32-bit hash comparison before full comparison
//! - **OnceLock Caching**: String representations cached after first use
//!
//! ## Metrics Collection Optimizations
//!
//! - **Cache-Aligned Atomics**: Prevent false sharing between CPU cores
//! - **Batched Updates**: Reduce atomic operation overhead
//! - **Fast Snapshots**: Lightweight metrics for hot paths
//! - **Saturating Arithmetic**: Prevent overflow in high-load scenarios
//!
//! ## Memory Pool Optimizations
//!
//! - **Size-Class Segregation**: Minimize fragmentation
//! - **NGDP-Specific Sizing**: Optimized for real-world file patterns
//! - **Thread-Local Caching**: Zero-contention for small allocations
//! - **Burst Handling**: Efficient allocation during patch downloads
//!
//! ## Game Download Optimizations
//!
//! - **Access Pattern Analysis**: Track cache usage patterns for different key types
//! - **Hit Rate Monitoring**: Monitor cache effectiveness for different content types
//! - **Usage Statistics**: Collect detailed statistics on cache access patterns
//! - **Performance Insights**: Understand cache behavior for NGDP workloads
//!
//! # Design Principles
//!
//! ## Zero-Copy When Possible
//!
//! Cache operations minimize data copying by using `bytes::Bytes` for stored data,
//! which provides efficient reference counting and zero-copy cloning. Memory pools
//! reuse buffers to avoid repeated allocation/deallocation cycles.
//!
//! ## Type Safety
//!
//! Cache keys are strongly typed to prevent mixing different types of cached data
//! and provide compile-time guarantees about cache key format and hashing.
//!
//! ## Async by Default
//!
//! All cache operations are async to support non-blocking I/O, which is essential
//! for high-throughput NGDP servers handling thousands of concurrent requests.
//!
//! ## Performance First
//!
//! Every component is optimized for NGDP workload patterns:
//! - Large files with bursty access
//! - High read/write ratios (90% reads)
//! - Sequential access patterns during patches
//! - Network-bound operations with CDN bottlenecks

#![warn(missing_docs)]
#![allow(clippy::return_self_not_must_use)] // Builder patterns
#![allow(clippy::float_cmp)] // Statistics need exact float comparisons
#![allow(clippy::mixed_attributes_style)] // Inner and outer doc attributes
#![allow(clippy::doc_markdown)] // Cache-specific terms don't need backticks
#![allow(clippy::use_self)] // Sometimes explicit types are clearer
#![allow(clippy::redundant_closure_for_method_calls)] // Sometimes clearer
#![allow(clippy::manual_instant_elapsed)] // Direct subtraction can be clearer

// ============================================================================
// Platform-independent modules (available on all platforms)
// ============================================================================
pub mod config;
pub mod error;
pub mod game_optimized;
pub mod key;
pub mod pool;
pub mod simd;
pub mod stats;
pub mod traits;

// ============================================================================
// Native-only modules (require tokio::time, filesystem, or other native features)
// ============================================================================
#[cfg(not(target_arch = "wasm32"))]
pub mod cdn;
#[cfg(not(target_arch = "wasm32"))]
pub mod disk_cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod integration;
#[cfg(not(target_arch = "wasm32"))]
pub mod memory;
#[cfg(not(target_arch = "wasm32"))]
pub mod memory_cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod multi_layer;
#[cfg(not(target_arch = "wasm32"))]
pub mod ngdp;
#[cfg(not(target_arch = "wasm32"))]
pub mod streaming;
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;
#[cfg(not(target_arch = "wasm32"))]
pub mod zerocopy;

// ============================================================================
// WASM-only modules (browser storage backends)
// ============================================================================
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub mod indexed_db_cache;
#[cfg(target_arch = "wasm32")]
pub mod local_storage_cache;

// ============================================================================
// Platform-independent re-exports (available on all platforms)
// ============================================================================
pub use error::{CacheError, CacheResult, NgdpCacheError, NgdpCacheResult, to_ngdp_result};
pub use game_optimized::{AccessPatternStats, AnalyzerConfig, CacheAccessAnalyzer};
pub use pool::{NgdpMemoryPool, NgdpSizeClass};
pub use simd::{
    CpuFeatures, SimdHashOperations, SimdStats, detect_cpu_features, global_simd_stats,
};
pub use stats::{CacheStats, FastCacheMetrics};
// Native-only stats exports
#[cfg(not(target_arch = "wasm32"))]
pub use stats::{AtomicCacheMetrics, MultiLayerStats, PerformanceMetrics};
// Platform-independent trait exports
pub use traits::{AsyncCache, EvictionPolicy, InvalidationStrategy};

// Native-only trait exports
#[cfg(not(target_arch = "wasm32"))]
pub use traits::{
    CacheEntry, CacheListener, CacheMetrics, CachePersistence, CacheWarming, MultiLayerCache,
};

// ============================================================================
// Native-only re-exports
// ============================================================================
#[cfg(not(target_arch = "wasm32"))]
pub use memory::{AccessPattern, ContentTypeHint, MemoryPool, MemoryPoolStats, SizedMemoryPool};
#[cfg(not(target_arch = "wasm32"))]
pub use streaming::{
    ContentStream, StreamingCache, StreamingConfig, StreamingProcessor, StreamingStats,
};
#[cfg(not(target_arch = "wasm32"))]
pub use validation::{
    Md5ValidationHooks, NgdpBytes, NgdpValidationHooks, NoOpValidationHooks, ValidationHooks,
    ValidationMetrics, ValidationResult,
};

// Re-export native cache implementations
#[cfg(not(target_arch = "wasm32"))]
pub use disk_cache::DiskCache;
#[cfg(not(target_arch = "wasm32"))]
pub use integration::{ArchiveOps, BlteBlockOps, EncodingFileOps, FormatConfig, RootFileOps};
#[cfg(not(target_arch = "wasm32"))]
pub use memory_cache::MemoryCache;
#[cfg(not(target_arch = "wasm32"))]
pub use multi_layer::{LayerStats, MultiLayerCacheImpl, MultiLayerStats as MultiLayerStatsV2};

// Re-export NGDP-specific cache implementations (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use ngdp::{
    ArchiveCache, ArchiveMetadata, BlockMetadata, BlteBlockCache, ContentAddressedCache,
    ContentValidationMetrics, NgdpResolutionCache, NgdpResolutionConfig, ResolutionMetrics,
};

// Re-export CDN integration components (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use cdn::{
    CdnArchiveCache, CdnBackedCache, CdnCacheBuilder, CdnCacheStack, CdnClient, CdnConfig,
    CdnContentCache, CdnMetrics, CdnNgdpResolutionCache,
};

// ============================================================================
// WASM-only re-exports
// ============================================================================
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub use indexed_db_cache::{IndexedDbCache, IndexedDbCacheConfig};
#[cfg(target_arch = "wasm32")]
pub use local_storage_cache::{LocalStorageCache, LocalStorageCacheConfig};

// ============================================================================
// Prelude module
// ============================================================================
pub mod prelude {
    //! Convenient re-exports of commonly used types and traits

    // Platform-independent exports
    pub use crate::{
        AccessPatternStats, AnalyzerConfig, CacheAccessAnalyzer,
        config::CacheConfig,
        error::{CacheError, CacheResult, NgdpCacheError, NgdpCacheResult, to_ngdp_result},
        key::{
            ArchiveIndexKey, ArchiveRangeKey, BlteBlockKey, BlteKey, CacheKey, ConfigKey,
            ContentCacheKey, EncodingFileKey, FastHash, ManifestKey, RibbitKey, RootFileKey,
        },
        pool::{NgdpMemoryPool, NgdpSizeClass, allocate_thread_local, deallocate_thread_local},
        stats::{CacheStats, FastCacheMetrics},
        traits::{AsyncCache, EvictionPolicy, InvalidationStrategy},
    };

    // Native-only exports
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::{
        ArchiveCache, BlteBlockCache, CacheEntry, ContentAddressedCache, DiskCache, MemoryCache,
        MultiLayerCacheImpl, NgdpResolutionCache,
        config::{DiskCacheConfig, MemoryCacheConfig, MultiLayerCacheConfig},
        integration::{ArchiveOps, BlteBlockOps, EncodingFileOps, FormatConfig, RootFileOps},
        memory::{AccessPattern, ContentTypeHint, MemoryPool, MemoryPoolStats, SizedMemoryPool},
        streaming::{
            ContentStream, StreamingCache, StreamingConfig, StreamingProcessor, StreamingStats,
        },
        traits::MultiLayerCache,
        validation::{
            Md5ValidationHooks, NgdpBytes, NgdpValidationHooks, NoOpValidationHooks,
            ValidationHooks, ValidationMetrics, ValidationResult,
        },
    };

    // WASM-only exports
    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    pub use crate::{
        IndexedDbCache, IndexedDbCacheConfig, LocalStorageCache, LocalStorageCacheConfig,
    };
}
