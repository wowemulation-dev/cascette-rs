//! Cache configuration structures
//!
//! This module defines the configuration structures used to set up and tune
//! different cache implementations. Each cache type has its own configuration
//! with sensible defaults and validation.

use crate::traits::{EvictionPolicy, InvalidationStrategy};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

/// Memory cache configuration
///
/// Configuration for in-memory cache implementations like LRU or HashMap-based caches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    /// Maximum number of entries in the cache
    pub max_entries: usize,
    /// Maximum memory usage in bytes (None for unlimited)
    pub max_memory_bytes: Option<usize>,
    /// Default TTL for entries (None for no expiration)
    pub default_ttl: Option<Duration>,
    /// Eviction policy when cache is full
    pub eviction_policy: EvictionPolicy,
    /// Invalidation strategy
    pub invalidation_strategy: InvalidationStrategy,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_memory_bytes: Some(100 * 1024 * 1024), // 100 MB
            default_ttl: Some(Duration::from_secs(3600)), // 1 hour
            eviction_policy: EvictionPolicy::Lru,
            invalidation_strategy: InvalidationStrategy::default(),
            enable_metrics: true,
            cleanup_interval: Duration::from_secs(60), // 1 minute
        }
    }
}

impl MemoryCacheConfig {
    /// Create a new memory cache configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of entries
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Set maximum memory usage
    pub fn with_max_memory(mut self, max_bytes: usize) -> Self {
        self.max_memory_bytes = Some(max_bytes);
        self
    }

    /// Set default TTL
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Set eviction policy
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_entries == 0 {
            return Err("max_entries must be greater than 0".to_string());
        }

        if let Some(max_bytes) = self.max_memory_bytes
            && max_bytes == 0
        {
            return Err("max_memory_bytes must be greater than 0".to_string());
        }

        if self.cleanup_interval.is_zero() {
            return Err("cleanup_interval must be greater than 0".to_string());
        }

        Ok(())
    }
}

/// Disk cache configuration
///
/// Configuration for persistent disk-based cache implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskCacheConfig {
    /// Base directory for cache storage
    pub cache_dir: PathBuf,
    /// Maximum number of files in the cache
    pub max_files: usize,
    /// Maximum disk usage in bytes (None for unlimited)
    pub max_disk_bytes: Option<usize>,
    /// Default TTL for entries (None for no expiration)
    pub default_ttl: Option<Duration>,
    /// Eviction policy when cache is full
    pub eviction_policy: EvictionPolicy,
    /// Invalidation strategy
    pub invalidation_strategy: InvalidationStrategy,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
    /// Sync interval for flushing data to disk
    pub sync_interval: Duration,
    /// Use subdirectories to avoid too many files in one directory
    pub use_subdirectories: bool,
    /// Number of subdirectory levels (if use_subdirectories is true)
    pub subdirectory_levels: usize,
}

impl Default for DiskCacheConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("cache"),
            max_files: 100_000,
            max_disk_bytes: Some(1024 * 1024 * 1024), // 1 GB
            default_ttl: Some(Duration::from_secs(24 * 3600)), // 24 hours
            eviction_policy: EvictionPolicy::Lru,
            invalidation_strategy: InvalidationStrategy::default(),
            enable_metrics: true,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            sync_interval: Duration::from_secs(30),     // 30 seconds
            use_subdirectories: true,
            subdirectory_levels: 2,
        }
    }
}

impl DiskCacheConfig {
    /// Create a new disk cache configuration
    pub fn new<P: Into<PathBuf>>(cache_dir: P) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            ..Self::default()
        }
    }

    /// Set maximum number of files
    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.max_files = max_files;
        self
    }

    /// Set maximum disk usage
    pub fn with_max_disk_usage(mut self, max_bytes: usize) -> Self {
        self.max_disk_bytes = Some(max_bytes);
        self
    }

    /// Set default TTL
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Enable or disable subdirectories
    pub fn with_subdirectories(mut self, enable: bool, levels: usize) -> Self {
        self.use_subdirectories = enable;
        self.subdirectory_levels = levels;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_files == 0 {
            return Err("max_files must be greater than 0".to_string());
        }

        if let Some(max_bytes) = self.max_disk_bytes
            && max_bytes == 0
        {
            return Err("max_disk_bytes must be greater than 0".to_string());
        }

        if self.cleanup_interval.is_zero() {
            return Err("cleanup_interval must be greater than 0".to_string());
        }

        if self.sync_interval.is_zero() {
            return Err("sync_interval must be greater than 0".to_string());
        }

        if self.use_subdirectories && self.subdirectory_levels == 0 {
            return Err(
                "subdirectory_levels must be greater than 0 when using subdirectories".to_string(),
            );
        }

        Ok(())
    }
}

/// Multi-layer cache configuration
///
/// Configuration for hierarchical cache implementations with multiple layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiLayerCacheConfig {
    /// Layer configurations (L1, L2, L3, etc.)
    pub layers: Vec<LayerConfig>,
    /// Promotion strategy between layers
    pub promotion_strategy: PromotionStrategy,
    /// Enable cross-layer statistics
    pub enable_cross_layer_stats: bool,
}

impl MultiLayerCacheConfig {
    /// Create a new multi-layer cache configuration
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            promotion_strategy: PromotionStrategy::OnHit,
            enable_cross_layer_stats: true,
        }
    }

    /// Add a memory cache layer
    pub fn add_memory_layer(mut self, config: MemoryCacheConfig) -> Self {
        self.layers.push(LayerConfig::Memory(config));
        self
    }

    /// Add a disk cache layer
    pub fn add_disk_layer(mut self, config: DiskCacheConfig) -> Self {
        self.layers.push(LayerConfig::Disk(config));
        self
    }

    /// Set promotion strategy
    pub fn with_promotion_strategy(mut self, strategy: PromotionStrategy) -> Self {
        self.promotion_strategy = strategy;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.layers.is_empty() {
            return Err("At least one layer must be configured".to_string());
        }

        for (i, layer) in self.layers.iter().enumerate() {
            layer.validate().map_err(|e| format!("Layer {i}: {e}"))?;
        }

        Ok(())
    }
}

impl Default for MultiLayerCacheConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual layer configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerConfig {
    /// Memory cache layer
    Memory(MemoryCacheConfig),
    /// Disk cache layer
    Disk(DiskCacheConfig),
}

impl LayerConfig {
    /// Validate the layer configuration
    pub fn validate(&self) -> Result<(), String> {
        match self {
            LayerConfig::Memory(config) => config.validate(),
            LayerConfig::Disk(config) => config.validate(),
        }
    }

    /// Get the invalidation strategy for this layer
    pub fn invalidation_strategy(&self) -> &InvalidationStrategy {
        match self {
            LayerConfig::Memory(config) => &config.invalidation_strategy,
            LayerConfig::Disk(config) => &config.invalidation_strategy,
        }
    }

    /// Check if metrics are enabled for this layer
    pub fn metrics_enabled(&self) -> bool {
        match self {
            LayerConfig::Memory(config) => config.enable_metrics,
            LayerConfig::Disk(config) => config.enable_metrics,
        }
    }
}

/// Promotion strategy for multi-layer caches
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum PromotionStrategy {
    /// Promote on any cache hit
    #[default]
    OnHit,
    /// Promote after N hits
    AfterNHits(u32),
    /// Promote based on access frequency
    FrequencyBased {
        /// Frequency threshold for promotion
        threshold: f64,
    },
    /// Promote based on entry age
    AgeBased {
        /// Minimum age before promotion
        min_age: Duration,
    },
    /// No automatic promotion
    Manual,
}

impl PartialEq for PromotionStrategy {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OnHit, Self::OnHit) | (Self::Manual, Self::Manual) => true,
            (Self::AfterNHits(a), Self::AfterNHits(b)) => a == b,
            (Self::FrequencyBased { threshold: a }, Self::FrequencyBased { threshold: b }) => {
                (a - b).abs() < f64::EPSILON
            }
            (Self::AgeBased { min_age: a }, Self::AgeBased { min_age: b }) => a == b,
            _ => false,
        }
    }
}

/// Cache warming configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheWarmingConfig {
    /// Enable cache warming on startup
    pub enabled: bool,
    /// Maximum number of entries to warm
    pub max_entries: usize,
    /// Timeout for warming operation
    pub timeout: Duration,
    /// Concurrency level for warming
    pub concurrency: usize,
    /// Predefined keys to warm
    pub predefined_keys: Vec<String>,
}

impl Default for CacheWarmingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_entries: 1000,
            timeout: Duration::from_secs(60),
            concurrency: 4,
            predefined_keys: Vec::new(),
        }
    }
}

/// Complete cache configuration
///
/// Top-level configuration that combines all cache settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Memory cache configuration
    pub memory: MemoryCacheConfig,
    /// Disk cache configuration
    pub disk: Option<DiskCacheConfig>,
    /// Multi-layer cache configuration
    pub multi_layer: Option<MultiLayerCacheConfig>,
    /// Cache warming configuration
    pub warming: CacheWarmingConfig,
    /// Global metrics configuration
    pub metrics_enabled: bool,
    /// Global tracing configuration
    pub tracing_enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            memory: MemoryCacheConfig::default(),
            disk: None,
            multi_layer: None,
            warming: CacheWarmingConfig::default(),
            metrics_enabled: true,
            tracing_enabled: false,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration with memory caching only
    pub fn memory_only() -> Self {
        Self::default()
    }

    /// Create a new cache configuration with memory and disk caching
    pub fn with_disk<P: Into<PathBuf>>(cache_dir: P) -> Self {
        Self {
            disk: Some(DiskCacheConfig::new(cache_dir)),
            ..Self::default()
        }
    }

    /// Create a multi-layer cache configuration
    pub fn multi_layer(config: MultiLayerCacheConfig) -> Self {
        Self {
            multi_layer: Some(config),
            ..Self::default()
        }
    }

    /// Enable cache warming
    pub fn with_warming(mut self, config: CacheWarmingConfig) -> Self {
        self.warming = config;
        self
    }

    /// Enable or disable metrics
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.metrics_enabled = enabled;
        self
    }

    /// Enable or disable tracing
    pub fn with_tracing(mut self, enabled: bool) -> Self {
        self.tracing_enabled = enabled;
        self
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<(), String> {
        self.memory.validate()?;

        if let Some(disk_config) = &self.disk {
            disk_config.validate()?;
        }

        if let Some(multi_layer_config) = &self.multi_layer {
            multi_layer_config.validate()?;
        }

        if self.warming.enabled && self.warming.max_entries == 0 {
            return Err("warming max_entries must be greater than 0 when enabled".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::uninlined_format_args)] // Test assertions use format strings for context
mod tests {
    use super::*;

    #[test]
    fn test_memory_cache_config_defaults() {
        let config = MemoryCacheConfig::default();
        assert_eq!(config.max_entries, 10_000);
        assert_eq!(config.max_memory_bytes, Some(100 * 1024 * 1024));
        assert_eq!(config.eviction_policy, EvictionPolicy::Lru);
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_memory_cache_config_builder() {
        let config = MemoryCacheConfig::new()
            .with_max_entries(5000)
            .with_max_memory(50 * 1024 * 1024)
            .with_eviction_policy(EvictionPolicy::Lfu);

        assert_eq!(config.max_entries, 5000);
        assert_eq!(config.max_memory_bytes, Some(50 * 1024 * 1024));
        assert_eq!(config.eviction_policy, EvictionPolicy::Lfu);
    }

    #[test]
    fn test_memory_cache_config_validation() {
        let mut config = MemoryCacheConfig::default();
        assert!(config.validate().is_ok());

        config.max_entries = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_disk_cache_config_defaults() {
        let config = DiskCacheConfig::default();
        assert_eq!(config.cache_dir, PathBuf::from("cache"));
        assert_eq!(config.max_files, 100_000);
        assert_eq!(config.max_disk_bytes, Some(1024 * 1024 * 1024));
        assert!(config.use_subdirectories);
        assert_eq!(config.subdirectory_levels, 2);
    }

    #[test]
    fn test_disk_cache_config_validation() {
        let config1 = DiskCacheConfig::default();
        assert!(config1.validate().is_ok());

        let config2 = DiskCacheConfig {
            max_files: 0,
            ..Default::default()
        };
        assert!(config2.validate().is_err());

        let config3 = DiskCacheConfig {
            max_files: 1000,
            use_subdirectories: true,
            subdirectory_levels: 0,
            ..Default::default()
        };
        assert!(config3.validate().is_err());
    }

    #[test]
    fn test_multi_layer_cache_config() {
        let config = MultiLayerCacheConfig::new()
            .add_memory_layer(MemoryCacheConfig::new())
            .add_disk_layer(DiskCacheConfig::new("/tmp/cache"));

        assert_eq!(config.layers.len(), 2);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cache_config_memory_only() {
        let config = CacheConfig::memory_only();
        assert!(config.disk.is_none());
        assert!(config.multi_layer.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cache_config_with_disk() {
        let config = CacheConfig::with_disk("/tmp/cache");
        assert!(config.disk.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_promotion_strategy_default() {
        let strategy = PromotionStrategy::default();
        assert_eq!(strategy, PromotionStrategy::OnHit);
    }

    #[test]
    fn test_memory_cache_config_edge_cases() {
        // Test with zero values
        let config1 = MemoryCacheConfig {
            max_entries: 0,
            ..Default::default()
        };
        assert!(config1.validate().is_err());

        let config2 = MemoryCacheConfig {
            max_entries: 1,
            max_memory_bytes: Some(0),
            ..Default::default()
        };
        assert!(config2.validate().is_err());

        let config3 = MemoryCacheConfig {
            max_entries: 1,
            max_memory_bytes: None, // Unlimited memory
            ..Default::default()
        };
        assert!(config3.validate().is_ok());

        // Test with zero cleanup interval
        let config4 = MemoryCacheConfig {
            cleanup_interval: Duration::ZERO,
            ..Default::default()
        };
        assert!(config4.validate().is_err());

        // Test with very small values
        let config5 = MemoryCacheConfig {
            cleanup_interval: Duration::from_nanos(1),
            ..Default::default()
        };
        assert!(config5.validate().is_ok());
    }

    #[test]
    fn test_memory_cache_config_overflow_resistance() {
        let config = MemoryCacheConfig::new()
            .with_max_entries(usize::MAX)
            .with_max_memory(usize::MAX);

        assert!(config.validate().is_ok());
        assert_eq!(config.max_entries, usize::MAX);
        assert_eq!(config.max_memory_bytes, Some(usize::MAX));
    }

    #[test]
    fn test_disk_cache_config_invalid_paths() {
        // Test with various potentially problematic paths
        let problematic_paths = vec![
            "",          // Empty path
            "\0",        // Null character
            "/dev/null", // Special device
            "con",       // Windows reserved name (on non-Windows, should be fine)
            "very/deep/path/that/might/not/exist/cache",
        ];

        for path in problematic_paths {
            let config = DiskCacheConfig::new(path);
            // Validation should pass - path existence isn't validated at config time
            assert!(
                config.validate().is_ok(),
                "Config validation failed for path: {}",
                path
            );
            assert_eq!(config.cache_dir, PathBuf::from(path));
        }
    }

    #[test]
    fn test_disk_cache_config_edge_cases() {
        // Test zero values
        let config1 = DiskCacheConfig {
            max_files: 0,
            ..Default::default()
        };
        assert!(config1.validate().is_err());

        let config2 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(0),
            ..Default::default()
        };
        assert!(config2.validate().is_err());

        // Test zero intervals
        let config3 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(1024),
            cleanup_interval: Duration::ZERO,
            ..Default::default()
        };
        assert!(config3.validate().is_err());

        let config4 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(1024),
            cleanup_interval: Duration::from_secs(1),
            sync_interval: Duration::ZERO,
            ..Default::default()
        };
        assert!(config4.validate().is_err());

        // Test subdirectory configuration edge cases
        let config5 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(1024),
            cleanup_interval: Duration::from_secs(1),
            sync_interval: Duration::from_secs(1),
            use_subdirectories: true,
            subdirectory_levels: 0,
            ..Default::default()
        };
        assert!(config5.validate().is_err());

        let config6 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(1024),
            cleanup_interval: Duration::from_secs(1),
            sync_interval: Duration::from_secs(1),
            use_subdirectories: true,
            subdirectory_levels: 1000, // Very deep nesting
            ..Default::default()
        };
        assert!(config6.validate().is_ok());

        let config7 = DiskCacheConfig {
            max_files: 1,
            max_disk_bytes: Some(1024),
            cleanup_interval: Duration::from_secs(1),
            sync_interval: Duration::from_secs(1),
            use_subdirectories: false,
            subdirectory_levels: 0, // Should be ok when subdirs disabled
            ..Default::default()
        };
        assert!(config7.validate().is_ok());
    }

    #[test]
    fn test_multi_layer_config_empty_layers() {
        let config = MultiLayerCacheConfig::new();
        assert!(config.validate().is_err()); // No layers configured
        assert_eq!(config.layers.len(), 0);
    }

    #[test]
    fn test_multi_layer_config_invalid_layers() {
        let memory_config = MemoryCacheConfig {
            max_entries: 0, // Invalid
            ..Default::default()
        };

        let config = MultiLayerCacheConfig::new().add_memory_layer(memory_config);

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.expect_err("Test operation should fail");
        assert!(error_msg.contains("Layer 0")); // Should identify which layer failed
        assert!(error_msg.contains("max_entries must be greater than 0"));
    }

    #[test]
    fn test_multi_layer_config_many_layers() {
        let mut config = MultiLayerCacheConfig::new();

        // Add many layers to test scalability
        for i in 0..100 {
            let memory_config = MemoryCacheConfig::new()
                .with_max_entries(1000 + i)
                .with_max_memory((1024 * 1024) + (i * 1024));
            config = config.add_memory_layer(memory_config);
        }

        assert_eq!(config.layers.len(), 100);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cache_config_warming_edge_cases() {
        let mut config = CacheConfig::default();

        // Test with warming enabled but zero entries
        config.warming.enabled = true;
        config.warming.max_entries = 0;
        assert!(config.validate().is_err());

        // Test with warming disabled and zero entries (should be ok)
        config.warming.enabled = false;
        assert!(config.validate().is_ok());

        // Test with very large warming configuration
        config.warming.enabled = true;
        config.warming.max_entries = usize::MAX;
        config.warming.timeout = Duration::from_secs(u64::MAX / 1000); // Avoid overflow
        config.warming.concurrency = usize::MAX;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cache_warming_config_extreme_values() {
        let config = CacheWarmingConfig {
            enabled: true,
            max_entries: 1,
            timeout: Duration::from_nanos(1), // Very short timeout
            concurrency: 1,
            predefined_keys: vec!["key".repeat(10000)], // Very long key
        };

        // Should not panic or fail validation
        assert!(!config.predefined_keys.is_empty());
        assert_eq!(config.predefined_keys[0].len(), 30000);
    }

    #[test]
    fn test_promotion_strategy_frequency_based_edge_cases() {
        let strategy1 = PromotionStrategy::FrequencyBased { threshold: 0.0 };
        let strategy2 = PromotionStrategy::FrequencyBased { threshold: 0.0 };
        assert_eq!(strategy1, strategy2);

        let strategy3 = PromotionStrategy::FrequencyBased { threshold: 1.0 };
        let strategy4 = PromotionStrategy::FrequencyBased { threshold: 1.0 };
        assert_eq!(strategy3, strategy4);

        // Test NaN handling
        let strategy5 = PromotionStrategy::FrequencyBased {
            threshold: f64::NAN,
        };
        let strategy6 = PromotionStrategy::FrequencyBased {
            threshold: f64::NAN,
        };
        assert_ne!(strategy5, strategy6); // NaN != NaN
    }

    #[test]
    fn test_promotion_strategy_age_based_edge_cases() {
        let strategy1 = PromotionStrategy::AgeBased {
            min_age: Duration::ZERO,
        };
        let strategy2 = PromotionStrategy::AgeBased {
            min_age: Duration::ZERO,
        };
        assert_eq!(strategy1, strategy2);

        let strategy3 = PromotionStrategy::AgeBased {
            min_age: Duration::MAX,
        };
        let strategy4 = PromotionStrategy::AgeBased {
            min_age: Duration::MAX,
        };
        assert_eq!(strategy3, strategy4);
    }

    #[test]
    fn test_layer_config_accessor_methods() {
        let memory_config = MemoryCacheConfig::new().with_eviction_policy(EvictionPolicy::Lfu);
        let disk_config = DiskCacheConfig::new("/tmp/test");

        let memory_layer = LayerConfig::Memory(memory_config.clone());
        let disk_layer = LayerConfig::Disk(disk_config.clone());

        // Test invalidation strategy access
        assert_eq!(
            memory_layer.invalidation_strategy(),
            &memory_config.invalidation_strategy
        );
        assert_eq!(
            disk_layer.invalidation_strategy(),
            &disk_config.invalidation_strategy
        );

        // Test metrics enabled access
        assert_eq!(memory_layer.metrics_enabled(), memory_config.enable_metrics);
        assert_eq!(disk_layer.metrics_enabled(), disk_config.enable_metrics);
    }

    #[test]
    fn test_config_builder_method_chaining() {
        // Test that builder methods can be chained extensively
        let config = MemoryCacheConfig::new()
            .with_max_entries(1000)
            .with_max_memory(1024 * 1024)
            .with_default_ttl(Duration::from_secs(300))
            .with_eviction_policy(EvictionPolicy::Lru)
            .with_max_entries(2000) // Override previous value
            .with_eviction_policy(EvictionPolicy::Lfu); // Override previous value

        assert_eq!(config.max_entries, 2000); // Should use last value
        assert_eq!(config.eviction_policy, EvictionPolicy::Lfu);
        assert_eq!(config.max_memory_bytes, Some(1024 * 1024));
    }

    #[test]
    fn test_disk_config_subdirectory_settings() {
        let config = DiskCacheConfig::new("/cache")
            .with_subdirectories(true, 3)
            .with_subdirectories(false, 0); // Disable

        assert!(!config.use_subdirectories);
        assert_eq!(config.subdirectory_levels, 0);

        // Re-enable with different level
        let config = config.with_subdirectories(true, 5);
        assert!(config.use_subdirectories);
        assert_eq!(config.subdirectory_levels, 5);
    }

    #[test]
    fn test_cache_config_combinations() {
        // Test various valid combinations
        let config1 = CacheConfig::memory_only()
            .with_metrics(true)
            .with_tracing(true);
        assert!(config1.validate().is_ok());
        assert!(config1.disk.is_none());
        assert!(config1.multi_layer.is_none());

        let config2 = CacheConfig::with_disk("/tmp/cache")
            .with_metrics(false)
            .with_tracing(false);
        assert!(config2.validate().is_ok());
        assert!(config2.disk.is_some());

        let multi_layer = MultiLayerCacheConfig::new()
            .add_memory_layer(MemoryCacheConfig::default())
            .add_disk_layer(DiskCacheConfig::default());
        let config3 = CacheConfig::multi_layer(multi_layer);
        assert!(config3.validate().is_ok());
        assert!(config3.multi_layer.is_some());
    }

    #[test]
    fn test_config_serialization_edge_cases() {
        // Test configs that might cause serialization issues
        let config_with_unicode = DiskCacheConfig::new("/cache/测试目录");
        assert_eq!(
            config_with_unicode.cache_dir,
            PathBuf::from("/cache/测试目录")
        );

        let config_with_special_chars = DiskCacheConfig::new("/cache/!@#$%^&*()");
        assert_eq!(
            config_with_special_chars.cache_dir,
            PathBuf::from("/cache/!@#$%^&*()")
        );
    }

    #[test]
    fn test_config_clone_and_equality() {
        let original = MemoryCacheConfig::new()
            .with_max_entries(5000)
            .with_eviction_policy(EvictionPolicy::Lfu);

        let cloned = original.clone();
        assert_eq!(original, cloned);

        // Modify clone and ensure they're different
        let modified = cloned.with_max_entries(6000);
        assert_ne!(original, modified);
        assert_eq!(original.max_entries, 5000);
        assert_eq!(modified.max_entries, 6000);
    }
}
