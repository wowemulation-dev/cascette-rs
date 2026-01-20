//! High-performance cache wrapper for protocol operations
//!
//! This module provides optimized caching for NGDP/CASC protocol operations.
//!
//! On native platforms, this integrates with the cascette-cache multi-layer caching system.
//! On WASM, a no-op cache is provided since persistent storage is not available.
//!
//! ## Performance Note
//!
//! On native platforms, this module bridges sync and async code. When called from async
//! contexts, it uses a shared background runtime to avoid blocking the current runtime.

use std::time::Duration;

use crate::config::CacheConfig;
use crate::error::Result;

// ============================================================================
// Native platform implementation (full caching support)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::{CacheConfig, CacheError, CacheStats, Duration, Result};
    use bytes::Bytes;
    use cascette_cache::{
        config::{DiskCacheConfig, MemoryCacheConfig},
        disk_cache::DiskCache,
        memory_cache::MemoryCache,
        traits::AsyncCache,
    };
    use std::sync::{Arc, OnceLock};
    use tokio::runtime::{Handle, Runtime};

    /// Shared runtime for executing cache operations when called from sync contexts.
    static SHARED_RUNTIME: OnceLock<Runtime> = OnceLock::new();

    /// Get or create the shared runtime for cache operations
    fn get_shared_runtime() -> &'static Runtime {
        #[allow(clippy::expect_used)]
        SHARED_RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("cascette-cache-runtime")
                .build()
                .expect("Failed to create shared cache runtime")
        })
    }

    /// Simple string-based cache key compatible with cascette-cache
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProtocolCacheKey {
        key: String,
        cached_key: OnceLock<String>,
    }

    impl std::hash::Hash for ProtocolCacheKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.key.hash(state);
        }
    }

    impl cascette_cache::key::CacheKey for ProtocolCacheKey {
        fn as_cache_key(&self) -> &str {
            self.cached_key.get_or_init(|| self.key.clone())
        }
    }

    impl ProtocolCacheKey {
        pub fn new(key: String) -> Self {
            Self {
                key,
                cached_key: OnceLock::new(),
            }
        }
    }

    /// High-performance protocol cache backed by cascette-cache
    pub struct ProtocolCache {
        cache: Arc<dyn AsyncCache<ProtocolCacheKey> + Send + Sync>,
        config: CacheConfig,
    }

    impl ProtocolCache {
        /// Create a new high-performance protocol cache
        pub fn new(config: &CacheConfig) -> Result<Self> {
            let cache: Arc<dyn AsyncCache<ProtocolCacheKey> + Send + Sync> =
                if let Some(ref cache_dir) = config.cache_dir {
                    let disk_config = DiskCacheConfig::new(cache_dir)
                        .with_max_disk_usage(config.disk_max_size_bytes)
                        .with_max_files(100_000)
                        .with_default_ttl(config.cdn_ttl)
                        .with_subdirectories(false, 0);

                    let disk_cache = DiskCache::new(disk_config).map_err(|e| {
                        crate::error::ProtocolError::Cache(CacheError::Backend(e.to_string()))
                    })?;

                    Arc::new(disk_cache)
                } else {
                    let memory_config = MemoryCacheConfig::new()
                        .with_max_entries(config.memory_max_items)
                        .with_max_memory(config.memory_max_size_bytes)
                        .with_default_ttl(config.ribbit_ttl);

                    let memory_cache = MemoryCache::new(memory_config).map_err(|e| {
                        crate::error::ProtocolError::Cache(CacheError::Backend(e.to_string()))
                    })?;

                    Arc::new(memory_cache)
                };

            Ok(Self {
                cache,
                config: config.clone(),
            })
        }

        fn execute_async<F, T>(operation: F) -> Result<T>
        where
            F: std::future::Future<Output = cascette_cache::error::CacheResult<T>> + Send + 'static,
            T: Send + 'static,
        {
            if Handle::try_current().is_ok() {
                let result = std::thread::spawn(move || {
                    let rt = get_shared_runtime();
                    rt.block_on(operation)
                })
                .join()
                .map_err(|_| {
                    crate::error::ProtocolError::Cache(CacheError::Runtime(
                        "Failed to execute async operation in thread".to_string(),
                    ))
                })?;
                result.map_err(|e| {
                    crate::error::ProtocolError::Cache(CacheError::Backend(e.to_string()))
                })
            } else {
                let rt = get_shared_runtime();
                rt.block_on(operation).map_err(|e| {
                    crate::error::ProtocolError::Cache(CacheError::Backend(e.to_string()))
                })
            }
        }

        fn parse_legacy_key(key: &str) -> ProtocolCacheKey {
            ProtocolCacheKey::new(key.to_string())
        }

        fn get_ttl_for_key(&self, key: &ProtocolCacheKey) -> Duration {
            if key.key.starts_with("ribbit:") {
                self.config.ribbit_ttl
            } else if key.key.starts_with("cdn:") {
                self.config.cdn_ttl
            } else {
                self.config.config_ttl
            }
        }

        pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            let cache_key = Self::parse_legacy_key(key);
            let cache = self.cache.clone();
            let result = Self::execute_async(async move { cache.get(&cache_key).await })?;
            Ok(result.map(|bytes| bytes.to_vec()))
        }

        pub fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
            self.get(key)
        }

        pub fn store_with_ttl(&self, key: &str, data: &[u8], ttl: Duration) -> Result<()> {
            let cache_key = Self::parse_legacy_key(key);
            let bytes = Bytes::copy_from_slice(data);
            let cache = self.cache.clone();
            Self::execute_async(async move { cache.put_with_ttl(cache_key, bytes, ttl).await })
        }

        pub fn store_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
            let cache_key = Self::parse_legacy_key(key);
            let ttl = self.get_ttl_for_key(&cache_key);
            self.store_with_ttl(key, data, ttl)
        }

        pub fn cleanup_expired(&self) -> Result<usize> {
            Ok(0)
        }

        pub fn stats(&self) -> Result<CacheStats> {
            let cache = self.cache.clone();
            let stats = Self::execute_async(async move { cache.stats().await })?;
            Ok(CacheStats {
                hits: stats.hit_count,
                misses: stats.miss_count,
                entries: stats.entry_count as u64,
                memory_usage: stats.memory_usage_bytes as u64,
                disk_usage: 0,
            })
        }

        #[allow(clippy::unused_async)]
        pub async fn warm_cache(&self, keys: Vec<String>) -> Result<usize> {
            let mut warmed = 0;
            for key in keys {
                let cache_key = Self::parse_legacy_key(&key);
                if matches!(self.cache.contains(&cache_key).await, Ok(true)) {
                    warmed += 1;
                }
            }
            Ok(warmed)
        }

        pub fn hit_rate(&self) -> Result<f64> {
            let stats = self.stats()?;
            Ok(stats.hit_rate())
        }

        pub fn clear(&self) -> Result<()> {
            let cache = self.cache.clone();
            Self::execute_async(async move { cache.clear().await })
        }

        pub fn len(&self) -> Result<usize> {
            let cache = self.cache.clone();
            Self::execute_async(async move { cache.size().await })
        }

        pub fn is_empty(&self) -> Result<bool> {
            let cache = self.cache.clone();
            Self::execute_async(async move { cache.is_empty().await })
        }
    }
}

// ============================================================================
// WASM platform implementation (no-op cache - no persistent storage)
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;

    /// No-op protocol cache for WASM platform
    ///
    /// On WASM, persistent storage is not available, so this implementation
    /// simply passes through without caching. Future implementations could
    /// use localStorage or IndexedDB for browser-based caching.
    pub struct ProtocolCache {
        _config: CacheConfig,
    }

    impl ProtocolCache {
        /// Create a new no-op cache for WASM
        pub fn new(config: &CacheConfig) -> Result<Self> {
            Ok(Self {
                _config: config.clone(),
            })
        }

        /// Get data from cache - always returns None on WASM
        pub fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
            Ok(None)
        }

        /// Get bytes from cache - always returns None on WASM
        pub fn get_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
            Ok(None)
        }

        /// Store data with TTL - no-op on WASM
        pub fn store_with_ttl(&self, _key: &str, _data: &[u8], _ttl: Duration) -> Result<()> {
            Ok(())
        }

        /// Store bytes - no-op on WASM
        pub fn store_bytes(&self, _key: &str, _data: &[u8]) -> Result<()> {
            Ok(())
        }

        /// Clean up expired entries - no-op on WASM
        pub fn cleanup_expired(&self) -> Result<usize> {
            Ok(0)
        }

        /// Get cache statistics
        pub fn stats(&self) -> Result<CacheStats> {
            Ok(CacheStats {
                hits: 0,
                misses: 0,
                entries: 0,
                memory_usage: 0,
                disk_usage: 0,
            })
        }

        /// Pre-warm cache - no-op on WASM
        #[allow(clippy::unused_async)]
        pub async fn warm_cache(&self, _keys: Vec<String>) -> Result<usize> {
            Ok(0)
        }

        /// Get cache hit rate - always 0 on WASM
        pub fn hit_rate(&self) -> Result<f64> {
            Ok(0.0)
        }

        /// Clear the cache - no-op on WASM
        pub fn clear(&self) -> Result<()> {
            Ok(())
        }

        /// Get current cache size - always 0 on WASM
        pub fn len(&self) -> Result<usize> {
            Ok(0)
        }

        /// Check if cache is empty - always true on WASM
        pub fn is_empty(&self) -> Result<bool> {
            Ok(true)
        }
    }
}

// ============================================================================
// Public exports
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use native::ProtocolCache;

#[cfg(target_arch = "wasm32")]
pub use wasm::ProtocolCache;

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: u64,
    pub memory_usage: u64,
    pub disk_usage: u64,
}

impl CacheStats {
    /// Calculate cache hit rate as percentage
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            (self.hits as f64) / ((self.hits + self.misses) as f64) * 100.0
        }
    }

    /// Get total cache usage in bytes
    pub fn total_usage(&self) -> u64 {
        self.memory_usage + self.disk_usage
    }
}

/// Enhanced error type for cache operations
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Backend cache error: {0}")]
    Backend(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Cache error: {0}")]
    Other(String),
}
