//! Type-aware memory pool for NGDP content optimization
//!
//! This module provides content-type aware memory pooling that understands NGDP file patterns
//! and optimizes allocation strategies based on the type of content being cached. Different
//! NGDP file types have different size characteristics and access patterns, so this system
//! maintains separate pools for each content type.
//!
//! # Architecture
//!
//! The memory pooling system consists of:
//!
//! - **ContentTypeHint**: Enum defining NGDP file types with typical sizes
//! - **MemoryPool**: Async trait for type-aware allocation operations
//! - **SizedMemoryPool**: Implementation with separate sub-pools per content type
//! - **Integration with NgdpBytes**: Zero-copy operations with existing validation
//!
//! # Content Type Optimization
//!
//! Each content type has different allocation patterns:
//!
//! - **Config**: Small, frequently accessed, short-lived
//! - **Encoding**: Very large, long-lived, sequential access
//! - **Archive**: Large, random access, medium lifetime
//! - **Root**: Medium-large, structured access, long-lived (2MB typical)
//! - **Install**: Medium, structured, medium lifetime (512KB typical)
//! - **Download**: Small-medium, streaming access (256KB typical)
//! - **Blte**: Variable size, decompression target, short-lived
//! - **Generic**: Fallback for unknown content types
//!
//! # Usage Example
//!
//! ```rust
//! use cascette_cache::memory::{ContentTypeHint, MemoryPool, SizedMemoryPool, BackgroundMemoryManager};
//! use cascette_cache::validation::NgdpBytes;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = Arc::new(SizedMemoryPool::new());
//!
//! // Start background optimization
//! let mut bg_manager = BackgroundMemoryManager::new(pool.clone())?;
//! bg_manager.start_optimization()?;
//!
//! // Allocate for different content types
//! let config_buffer = pool.allocate_for_type(ContentTypeHint::Config, 16384).await?;
//! let encoding_buffer = pool.allocate_for_type(ContentTypeHint::Encoding, 16 * 1024 * 1024).await?;
//!
//! // Clone buffer before using with NgdpBytes to allow deallocation
//! let ngdp_data = NgdpBytes::from_pool_buffer(config_buffer.clone(), None);
//!
//! // Return buffers to appropriate pools
//! pool.deallocate(config_buffer).await?;
//! pool.deallocate(encoding_buffer).await?;
//!
//! // Shutdown background manager
//! bg_manager.shutdown().await?;
//! # Ok(())
//! # }
//! ```
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::cast_lossless)] // u32/u8 to u64 casts are safe
#![allow(clippy::single_match_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss
#![allow(clippy::match_same_arms)] // Match arms have semantic meaning even when returning same value
#![allow(clippy::suboptimal_flops)] // Exponential moving average clarity is more important than FMA optimization

use crate::{
    error::{NgdpCacheError, NgdpCacheResult},
    pool::{NgdpMemoryPool, NgdpSizeClass},
};
use async_trait::async_trait;
use bytes::BytesMut;
use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::sleep,
};

/// Content type hints for NGDP file optimization
///
/// Each content type represents a different NGDP file format with specific
/// size characteristics and access patterns. This allows the memory pool
/// to optimize allocation strategies and buffer sizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentTypeHint {
    /// Configuration files (build config, CDN config)
    ///
    /// Characteristics:
    /// - Small size: typically small configuration files
    /// - High frequency access during startup and updates
    /// - Short to medium lifetime (5-60 minutes)
    /// - Text-based content, well compressible
    Config,

    /// Encoding files (content key to encoding key mappings)
    ///
    /// Characteristics:
    /// - Very large size: 10MB - 20MB typical
    /// - Sequential access during patch analysis
    /// - Long lifetime (hours to days)
    /// - Binary format with repeated patterns
    Encoding,

    /// Archive data files (compressed game assets)
    ///
    /// Characteristics:
    /// - Large size: 1MB - 10MB typical
    /// - Random access patterns for asset streaming
    /// - Medium to long lifetime (minutes to hours)
    /// - Already compressed BLTE content
    Archive,

    /// Root files (file tree catalog)
    ///
    /// Characteristics:
    /// - Medium-large size: 512KB - 4MB typical
    /// - Structured access following file paths
    /// - Long lifetime (hours to days)
    /// - Contains file metadata and structure
    Root,

    /// Install manifests (installation tracking)
    ///
    /// Characteristics:
    /// - Medium size: 128KB - 1MB typical
    /// - Structured access by tags and priorities
    /// - Medium lifetime (minutes to hours)
    /// - Tagged installation metadata
    Install,

    /// Download manifests (streaming priority)
    ///
    /// Characteristics:
    /// - Small-medium size: 64KB - 512KB typical
    /// - Sequential access during streaming
    /// - Short to medium lifetime (minutes)
    /// - Priority and streaming metadata
    Download,

    /// BLTE decompressed content (game asset data)
    ///
    /// Characteristics:
    /// - Variable size: typically large manifest files
    /// - Target for decompression operations
    /// - Short lifetime (seconds to minutes)
    /// - Decompressed asset content
    Blte,

    /// Generic/unknown content type
    ///
    /// Characteristics:
    /// - Unknown size patterns
    /// - Unknown access patterns
    /// - Default allocation strategy
    /// - Fallback for unrecognized content
    Generic,
}

impl ContentTypeHint {
    /// Get typical size for this content type
    ///
    /// Returns the expected size in bytes for optimal buffer pre-allocation.
    /// This is used to select appropriate size classes and pool parameters.
    pub fn typical_size(self) -> usize {
        match self {
            Self::Config => 16 * 1024,          // Small config size
            Self::Encoding => 16 * 1024 * 1024, // Large encoding size
            Self::Archive => 8 * 1024 * 1024,   // Medium archive size
            Self::Root => 2 * 1024 * 1024,      // 2MB
            Self::Install => 512 * 1024,        // 512KB
            Self::Download => 256 * 1024,       // 256KB
            Self::Blte => 1024 * 1024,          // 1MB (variable)
            Self::Generic => 64 * 1024,         // 64KB (conservative)
        }
    }

    /// Get size class mapping for this content type
    ///
    /// Maps content type to the most appropriate NgdpSizeClass for
    /// efficient allocation and pool management.
    pub fn size_class(self) -> NgdpSizeClass {
        NgdpSizeClass::from_size(self.typical_size())
    }

    /// Get access pattern characteristics
    ///
    /// Returns information about expected access patterns which can be used
    /// to optimize caching policies and buffer management.
    pub fn access_pattern(self) -> AccessPattern {
        match self {
            Self::Config => AccessPattern {
                sequential: true,
                random: false,
                burst_likely: true,
                reuse_probability: 0.8,
            },
            Self::Encoding => AccessPattern {
                sequential: true,
                random: false,
                burst_likely: false,
                reuse_probability: 0.3,
            },
            Self::Archive => AccessPattern {
                sequential: false,
                random: true,
                burst_likely: true,
                reuse_probability: 0.6,
            },
            Self::Root => AccessPattern {
                sequential: false,
                random: true,
                burst_likely: false,
                reuse_probability: 0.7,
            },
            Self::Install => AccessPattern {
                sequential: true,
                random: false,
                burst_likely: false,
                reuse_probability: 0.5,
            },
            Self::Download => AccessPattern {
                sequential: true,
                random: false,
                burst_likely: true,
                reuse_probability: 0.4,
            },
            Self::Blte => AccessPattern {
                sequential: true,
                random: false,
                burst_likely: false,
                reuse_probability: 0.2,
            },
            Self::Generic => AccessPattern {
                sequential: false,
                random: true,
                burst_likely: false,
                reuse_probability: 0.3,
            },
        }
    }

    /// Get expected lifetime for this content type
    ///
    /// Returns the expected cache lifetime which helps with eviction policies
    /// and pool sizing decisions.
    pub fn expected_lifetime(self) -> Duration {
        match self {
            Self::Config => Duration::from_secs(300), // 5 minutes
            Self::Encoding => Duration::from_secs(3600 * 4), // 4 hours
            Self::Archive => Duration::from_secs(1800), // 30 minutes
            Self::Root => Duration::from_secs(3600 * 2), // 2 hours
            Self::Install => Duration::from_secs(900), // 15 minutes
            Self::Download => Duration::from_secs(300), // 5 minutes
            Self::Blte => Duration::from_secs(60),    // 1 minute
            Self::Generic => Duration::from_secs(600), // 10 minutes
        }
    }

    /// Get all content type variants
    pub fn all() -> &'static [Self] {
        &[
            Self::Config,
            Self::Encoding,
            Self::Archive,
            Self::Root,
            Self::Install,
            Self::Download,
            Self::Blte,
            Self::Generic,
        ]
    }
}

/// Access pattern characteristics for a content type
///
/// Describes the expected access patterns which can influence caching
/// strategies and memory allocation decisions.
#[derive(Debug, Clone)]
pub struct AccessPattern {
    /// Whether sequential access is common
    pub sequential: bool,
    /// Whether random access is common
    pub random: bool,
    /// Whether burst access patterns are likely
    pub burst_likely: bool,
    /// Probability of buffer reuse (0.0 to 1.0)
    pub reuse_probability: f32,
}

/// Statistics for type-aware memory pool operations
#[derive(Debug, Clone)]
pub struct MemoryPoolStats {
    /// Total allocations per content type
    pub allocations_by_type: HashMap<ContentTypeHint, u64>,
    /// Total bytes allocated per content type
    pub bytes_by_type: HashMap<ContentTypeHint, u64>,
    /// Buffer reuse count per content type
    pub reuses_by_type: HashMap<ContentTypeHint, u64>,
    /// Pool misses per content type
    pub misses_by_type: HashMap<ContentTypeHint, u64>,
    /// Average allocation size per content type
    pub avg_size_by_type: HashMap<ContentTypeHint, usize>,
    /// Pool creation timestamp
    pub created_at: Instant,
    /// Last statistics update
    pub updated_at: Instant,
}

impl MemoryPoolStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            allocations_by_type: HashMap::new(),
            bytes_by_type: HashMap::new(),
            reuses_by_type: HashMap::new(),
            misses_by_type: HashMap::new(),
            avg_size_by_type: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Get total allocations across all content types
    pub fn total_allocations(&self) -> u64 {
        self.allocations_by_type.values().sum()
    }

    /// Get total bytes allocated across all content types
    pub fn total_bytes(&self) -> u64 {
        self.bytes_by_type.values().sum()
    }

    /// Get total reuses across all content types
    pub fn total_reuses(&self) -> u64 {
        self.reuses_by_type.values().sum()
    }

    /// Get overall reuse rate
    #[allow(clippy::cast_precision_loss)] // Stats calculation intentionally accepts precision loss
    pub fn reuse_rate(&self) -> f64 {
        let total_allocations = self.total_allocations();
        if total_allocations == 0 {
            0.0
        } else {
            self.total_reuses() as f64 / total_allocations as f64
        }
    }

    /// Get reuse rate for specific content type
    pub fn reuse_rate_for_type(&self, content_type: ContentTypeHint) -> f64 {
        let allocations = self.allocations_by_type.get(&content_type).unwrap_or(&0);
        let reuses = self.reuses_by_type.get(&content_type).unwrap_or(&0);

        if *allocations == 0 {
            0.0
        } else {
            *reuses as f64 / *allocations as f64
        }
    }

    /// Get pool age
    pub fn age(&self) -> Duration {
        Instant::now() - self.created_at
    }
}

impl Default for MemoryPoolStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Async trait for type-aware memory pool operations
///
/// Provides allocation and deallocation operations that are aware of NGDP
/// content types and can optimize memory management accordingly.
#[async_trait]
pub trait MemoryPool: Send + Sync {
    /// Allocate buffer optimized for specific content type
    ///
    /// This method takes content type hints into account when selecting
    /// appropriate buffer sizes and allocation strategies.
    async fn allocate_for_type(
        &self,
        content_type: ContentTypeHint,
        requested_size: usize,
    ) -> NgdpCacheResult<BytesMut>;

    /// Deallocate buffer and return to appropriate pool
    ///
    /// The implementation should determine the appropriate pool based on
    /// buffer characteristics and return it for potential reuse.
    async fn deallocate(&self, buffer: BytesMut) -> NgdpCacheResult<()>;

    /// Get current pool statistics
    ///
    /// Returns detailed statistics about allocations, reuses, and performance
    /// metrics for all content types.
    async fn get_stats(&self) -> NgdpCacheResult<MemoryPoolStats>;

    /// Clear all pools and free memory
    ///
    /// This operation clears all sub-pools and releases memory back to the system.
    /// Useful for cleanup or during low-memory conditions.
    async fn clear(&self) -> NgdpCacheResult<()>;

    /// Warm up pools for expected content types
    ///
    /// Pre-allocates buffers for commonly used content types to reduce
    /// allocation latency during actual operations.
    async fn warm_up(&self) -> NgdpCacheResult<()>;
}

/// Content type aware memory pool with sub-pools for each NGDP file type
///
/// This implementation maintains separate NgdpMemoryPool instances for each
/// content type, allowing for type-specific optimization of buffer sizes,
/// pool depths, and allocation strategies.
pub struct SizedMemoryPool {
    /// Sub-pools for each content type
    pools: HashMap<ContentTypeHint, Arc<NgdpMemoryPool>>,
    /// Pool statistics (thread-safe)
    stats: Arc<RwLock<MemoryPoolStats>>,
    /// Allocation counter for statistics
    allocation_counter: AtomicU64,
}

impl SizedMemoryPool {
    /// Create a new sized memory pool
    ///
    /// Initializes sub-pools for all NGDP content types with optimized
    /// configurations based on expected usage patterns.
    pub fn new() -> Self {
        let mut pools = HashMap::new();

        // Initialize sub-pools for each content type
        for &content_type in ContentTypeHint::all() {
            pools.insert(content_type, Arc::new(NgdpMemoryPool::new()));
        }

        Self {
            pools,
            stats: Arc::new(RwLock::new(MemoryPoolStats::new())),
            allocation_counter: AtomicU64::new(0),
        }
    }

    /// Get sub-pool for specific content type
    #[allow(clippy::expect_used)] // SAFETY: All content types are initialized in new()
    fn get_pool(&self, content_type: ContentTypeHint) -> &Arc<NgdpMemoryPool> {
        // This is safe because we initialize all content types in new()
        self.pools
            .get(&content_type)
            .expect("Pool should exist for all content types")
    }

    /// Update statistics for allocation
    fn update_allocation_stats(&self, content_type: ContentTypeHint, size: usize, was_reuse: bool) {
        if let Ok(mut stats) = self.stats.write() {
            // Update allocations and get current values
            let allocations_entry = stats.allocations_by_type.entry(content_type).or_insert(0);
            *allocations_entry += 1;
            let current_allocations = *allocations_entry;

            let bytes_entry = stats.bytes_by_type.entry(content_type).or_insert(0);
            *bytes_entry += size as u64;
            let current_bytes = *bytes_entry;

            if was_reuse {
                *stats.reuses_by_type.entry(content_type).or_insert(0) += 1;
            } else {
                *stats.misses_by_type.entry(content_type).or_insert(0) += 1;
            }

            // Update average size using the current values
            let avg_size = current_bytes / current_allocations;
            stats
                .avg_size_by_type
                .insert(content_type, avg_size as usize);

            stats.updated_at = Instant::now();
        }
    }
}

#[async_trait]
impl MemoryPool for SizedMemoryPool {
    async fn allocate_for_type(
        &self,
        content_type: ContentTypeHint,
        requested_size: usize,
    ) -> NgdpCacheResult<BytesMut> {
        let pool = self.get_pool(content_type);

        // Use the larger of requested size or typical size for this content type
        let optimal_size = requested_size.max(content_type.typical_size());

        // Check pool statistics before allocation to determine if this will be a reuse
        let size_class = NgdpSizeClass::from_size(optimal_size);
        let pool_stats_before = pool.size_class_stats(size_class);
        let had_buffers = pool_stats_before.pool_size > 0;

        // Allocate from the appropriate sub-pool
        let buffer = pool.allocate(optimal_size);

        // Update our type-aware statistics
        self.update_allocation_stats(content_type, optimal_size, had_buffers);

        // Increment global allocation counter
        self.allocation_counter.fetch_add(1, Ordering::Relaxed);

        Ok(buffer)
    }

    async fn deallocate(&self, buffer: BytesMut) -> NgdpCacheResult<()> {
        // Determine content type from buffer size (best effort)
        // In a real implementation, we might want to track this more precisely
        let buffer_size = buffer.capacity();
        let size_class = NgdpSizeClass::from_size(buffer_size);

        // Find the most likely content type for this buffer size
        let likely_content_type = ContentTypeHint::all()
            .iter()
            .find(|&&ct| ct.size_class() == size_class)
            .unwrap_or(&ContentTypeHint::Generic);

        let pool = self.get_pool(*likely_content_type);
        pool.deallocate(buffer);

        Ok(())
    }

    async fn get_stats(&self) -> NgdpCacheResult<MemoryPoolStats> {
        if let Ok(stats) = self.stats.read() {
            Ok(stats.clone())
        } else {
            Err(NgdpCacheError::Cache(crate::error::CacheError::Backend(
                "Failed to acquire stats lock".to_string(),
            )))
        }
    }

    async fn clear(&self) -> NgdpCacheResult<()> {
        // Clear all sub-pools
        for pool in self.pools.values() {
            pool.clear();
        }

        // Reset statistics
        if let Ok(mut stats) = self.stats.write() {
            *stats = MemoryPoolStats::new();
        }

        // Reset allocation counter
        self.allocation_counter.store(0, Ordering::Relaxed);

        Ok(())
    }

    async fn warm_up(&self) -> NgdpCacheResult<()> {
        // Warm up each sub-pool
        for pool in self.pools.values() {
            pool.warm_up();
        }
        Ok(())
    }
}

impl Default for SizedMemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Background optimization tasks that can be performed on memory pools
///
/// These tasks are executed asynchronously to optimize memory pool performance
/// without blocking allocation operations.
#[derive(Debug, Clone)]
pub enum OptimizationTask {
    /// Monitor pool usage patterns and collect statistics
    ///
    /// This task tracks allocation patterns, hit rates, and size distributions
    /// to identify optimization opportunities.
    MonitorUsage {
        /// Content type to monitor
        content_type: ContentTypeHint,
        /// Monitoring interval
        interval: Duration,
    },

    /// Adjust pool sizes based on observed usage patterns
    ///
    /// Dynamically tunes pool sizes to match actual allocation patterns,
    /// reducing memory waste while maintaining performance.
    TunePoolSize {
        /// Content type to tune
        content_type: ContentTypeHint,
        /// Target reuse rate (0.0 to 1.0)
        target_reuse_rate: f32,
        /// Maximum size adjustment per iteration
        max_adjustment: f32,
    },

    /// Perform pool defragmentation
    ///
    /// Consolidates fragmented memory and reorganizes pools to improve
    /// locality and reduce memory overhead.
    DefragmentPool {
        /// Content type to defragment
        content_type: ContentTypeHint,
        /// Minimum fragmentation threshold to trigger defrag
        fragmentation_threshold: f32,
    },

    /// Warm up pools based on predicted usage patterns
    ///
    /// Pre-allocates buffers for content types that are likely to be
    /// requested soon based on historical patterns.
    WarmUpPools {
        /// Content types to warm up with their predicted allocation counts
        predictions: Vec<(ContentTypeHint, usize)>,
    },

    /// Detect and respond to memory pressure
    ///
    /// Monitors system memory usage and adjusts pool behavior during
    /// high memory pressure conditions.
    MemoryPressureCheck {
        /// Memory pressure threshold (0.0 to 1.0)
        pressure_threshold: f32,
        /// Action to take when pressure is detected
        response: PressureResponse,
    },

    /// Clean up unused pool entries
    ///
    /// Removes old, unused buffers from pools to free memory and
    /// prevent unbounded growth during low activity periods.
    CleanupUnused {
        /// Maximum age for unused buffers
        max_age: Duration,
        /// Minimum pool size to maintain after cleanup
        min_pool_size: usize,
    },
}

/// Response actions for memory pressure detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureResponse {
    /// Reduce pool sizes by a percentage
    ReducePools(u8),
    /// Clear pools below a certain size class
    ClearSmallPools,
    /// Switch to emergency allocation mode
    EmergencyMode,
    /// Log warning and continue normal operation
    LogWarning,
}

/// Usage pattern tracking for adaptive pool sizing
///
/// Tracks allocation patterns over time to identify trends and optimize
/// pool configurations automatically.
#[derive(Debug, Clone)]
pub struct UsagePattern {
    /// Content type being tracked
    pub content_type: ContentTypeHint,
    /// Recent allocation sizes (rolling window)
    pub recent_sizes: Vec<usize>,
    /// Recent allocation intervals (rolling window)
    pub recent_intervals: Vec<Duration>,
    /// Average allocation size over the tracking period
    pub avg_allocation_size: usize,
    /// Peak allocation rate (allocations per second)
    pub peak_allocation_rate: f64,
    /// Current reuse rate
    pub current_reuse_rate: f32,
    /// Trend direction (positive = increasing, negative = decreasing)
    pub trend: f32,
    /// Last update timestamp
    pub last_update: Instant,
    /// Pattern confidence (0.0 to 1.0)
    pub confidence: f32,
}

impl UsagePattern {
    /// Create a new usage pattern tracker
    pub fn new(content_type: ContentTypeHint) -> Self {
        Self {
            content_type,
            recent_sizes: Vec::with_capacity(100),
            recent_intervals: Vec::with_capacity(100),
            avg_allocation_size: content_type.typical_size(),
            peak_allocation_rate: 0.0,
            current_reuse_rate: 0.0,
            trend: 0.0,
            last_update: Instant::now(),
            confidence: 0.0,
        }
    }

    /// Update pattern with new allocation data
    pub fn update_allocation(&mut self, size: usize, was_reused: bool) {
        let now = Instant::now();
        let interval = now.duration_since(self.last_update);

        // Add to rolling windows
        self.recent_sizes.push(size);
        self.recent_intervals.push(interval);

        // Maintain window size
        if self.recent_sizes.len() > 100 {
            self.recent_sizes.remove(0);
            self.recent_intervals.remove(0);
        }

        // Update statistics
        self.avg_allocation_size = if self.recent_sizes.is_empty() {
            size
        } else {
            self.recent_sizes.iter().sum::<usize>() / self.recent_sizes.len()
        };

        // Calculate allocation rate
        let total_time: Duration = self.recent_intervals.iter().sum();
        if !total_time.is_zero() {
            self.peak_allocation_rate = self.recent_sizes.len() as f64 / total_time.as_secs_f64();
        }

        // Update reuse rate (simple exponential moving average)
        let alpha = 0.1;
        let new_reuse = if was_reused { 1.0 } else { 0.0 };
        self.current_reuse_rate = alpha * new_reuse + (1.0 - alpha) * self.current_reuse_rate;

        // Calculate trend (simplified linear regression)
        self.update_trend();

        // Update confidence based on sample size
        self.confidence = (self.recent_sizes.len() as f32 / 100.0).min(1.0);

        self.last_update = now;
    }

    /// Update trend calculation
    fn update_trend(&mut self) {
        if self.recent_sizes.len() < 10 {
            return;
        }

        let n = self.recent_sizes.len();
        let half = n / 2;

        let first_half_avg: f32 =
            self.recent_sizes[0..half].iter().sum::<usize>() as f32 / half as f32;
        let second_half_avg: f32 =
            self.recent_sizes[half..].iter().sum::<usize>() as f32 / (n - half) as f32;

        self.trend = (second_half_avg - first_half_avg) / first_half_avg;
    }

    /// Get recommended pool size based on current pattern
    pub fn recommended_pool_size(&self) -> usize {
        if self.confidence < 0.3 {
            // Low confidence, use default
            return self.content_type.size_class().max_pool_size();
        }

        let base_size = self.content_type.size_class().max_pool_size() as f32;
        let rate_adjustment = if self.current_reuse_rate > 0.7 {
            1.5 // High reuse, increase pool size
        } else if self.current_reuse_rate < 0.3 {
            0.7 // Low reuse, decrease pool size
        } else {
            1.0 // Normal reuse, keep current size
        };

        let trend_adjustment = 1.0 + (self.trend * 0.2).clamp(-0.3, 0.5);

        let recommended = base_size * rate_adjustment * trend_adjustment;
        recommended.round() as usize
    }

    /// Check if pattern suggests pool tuning is needed
    pub fn needs_tuning(&self) -> bool {
        self.confidence > 0.5
            && (
                self.current_reuse_rate < 0.3 ||  // Low reuse rate
            self.current_reuse_rate > 0.9 ||  // Very high reuse rate (might need more buffers)
            self.trend.abs() > 0.3
                // Significant trend change
            )
    }
}

/// Strategy for automatic pool tuning
///
/// Determines how pools should be adjusted based on usage patterns
/// and system constraints.
#[derive(Debug, Clone)]
pub enum PoolTuningStrategy {
    /// Conservative tuning - makes small adjustments
    Conservative {
        /// Maximum adjustment factor per tuning cycle
        max_adjustment: f32,
        /// Minimum confidence required for adjustments
        min_confidence: f32,
    },

    /// Aggressive tuning - makes larger adjustments for faster adaptation
    Aggressive {
        /// Maximum adjustment factor per tuning cycle
        max_adjustment: f32,
        /// Minimum confidence required for adjustments
        min_confidence: f32,
    },

    /// Adaptive tuning - adjusts strategy based on system conditions
    Adaptive {
        /// Current adjustment factor (dynamic)
        current_adjustment: f32,
        /// Confidence threshold (dynamic)
        confidence_threshold: f32,
        /// Performance score (higher = better)
        performance_score: f32,
    },
}

impl PoolTuningStrategy {
    /// Get conservative tuning strategy
    pub fn conservative() -> Self {
        Self::Conservative {
            max_adjustment: 0.2,
            min_confidence: 0.7,
        }
    }

    /// Get aggressive tuning strategy
    pub fn aggressive() -> Self {
        Self::Aggressive {
            max_adjustment: 0.5,
            min_confidence: 0.5,
        }
    }

    /// Get adaptive tuning strategy
    pub fn adaptive() -> Self {
        Self::Adaptive {
            current_adjustment: 0.3,
            confidence_threshold: 0.6,
            performance_score: 0.5,
        }
    }

    /// Calculate pool size adjustment based on usage pattern
    pub fn calculate_adjustment(
        &self,
        pattern: &UsagePattern,
        current_size: usize,
    ) -> Option<usize> {
        let (max_adj, min_conf) = match self {
            Self::Conservative {
                max_adjustment,
                min_confidence,
            }
            | Self::Aggressive {
                max_adjustment,
                min_confidence,
            } => (*max_adjustment, *min_confidence),
            Self::Adaptive {
                current_adjustment,
                confidence_threshold,
                ..
            } => (*current_adjustment, *confidence_threshold),
        };

        if pattern.confidence < min_conf || !pattern.needs_tuning() {
            return None;
        }

        let recommended = pattern.recommended_pool_size();
        let current = current_size as f32;
        let target = recommended as f32;

        let raw_adjustment = (target - current) / current;
        let clamped_adjustment = raw_adjustment.clamp(-max_adj, max_adj);

        let new_size = (current * (1.0 + clamped_adjustment)).round() as usize;

        // Ensure minimum pool size
        let min_size = match pattern.content_type {
            ContentTypeHint::Config | ContentTypeHint::Download => 4,
            ContentTypeHint::Encoding | ContentTypeHint::Archive => 1,
            _ => 2,
        };

        Some(new_size.max(min_size))
    }
}

/// Configuration for background memory optimization
#[derive(Debug, Clone)]
pub struct BackgroundConfig {
    /// Base interval for optimization tasks
    pub base_interval: Duration,
    /// Memory pressure monitoring interval
    pub pressure_check_interval: Duration,
    /// Usage pattern monitoring interval
    pub pattern_monitoring_interval: Duration,
    /// Pool tuning interval
    pub tuning_interval: Duration,
    /// Cleanup interval for unused buffers
    pub cleanup_interval: Duration,
    /// Maximum memory pressure threshold (0.0 to 1.0)
    pub memory_pressure_threshold: f32,
    /// Pool tuning strategy
    pub tuning_strategy: PoolTuningStrategy,
    /// Enable defragmentation tasks
    pub enable_defragmentation: bool,
    /// Enable automatic warm-up
    pub enable_auto_warmup: bool,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            base_interval: Duration::from_secs(30),
            pressure_check_interval: Duration::from_secs(10),
            pattern_monitoring_interval: Duration::from_secs(15),
            tuning_interval: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            memory_pressure_threshold: 0.85,
            tuning_strategy: PoolTuningStrategy::conservative(),
            enable_defragmentation: false, // Conservative default
            enable_auto_warmup: true,
        }
    }
}

impl BackgroundConfig {
    /// Create configuration optimized for testing with much faster intervals
    #[cfg(test)]
    pub fn test_config() -> Self {
        Self {
            base_interval: Duration::from_millis(10),
            pressure_check_interval: Duration::from_millis(5),
            pattern_monitoring_interval: Duration::from_millis(8),
            tuning_interval: Duration::from_millis(20),
            cleanup_interval: Duration::from_millis(50),
            memory_pressure_threshold: 0.85,
            tuning_strategy: PoolTuningStrategy::conservative(),
            enable_defragmentation: false,
            enable_auto_warmup: true,
        }
    }
}

/// Background memory manager for optimizing pool performance
///
/// Runs asynchronous tasks to monitor memory usage patterns, adjust pool sizes,
/// detect memory pressure, and perform maintenance operations without blocking
/// allocation requests.
pub struct BackgroundMemoryManager {
    /// Reference to the pool being managed
    pool: Arc<SizedMemoryPool>,
    /// Configuration for optimization tasks
    config: BackgroundConfig,
    /// Usage pattern trackers for each content type
    usage_patterns: Arc<RwLock<HashMap<ContentTypeHint, UsagePattern>>>,
    /// Channel for sending optimization tasks
    task_sender: mpsc::UnboundedSender<OptimizationTask>,
    /// Task receiver (moved to background worker)
    task_receiver: Option<mpsc::UnboundedReceiver<OptimizationTask>>,
    /// Background worker task handle
    worker_handle: Option<JoinHandle<()>>,
    /// Shutdown signal
    shutdown_sender: Option<oneshot::Sender<()>>,
    /// Running state
    is_running: Arc<AtomicBool>,
    /// Statistics for background operations
    background_stats: Arc<RwLock<BackgroundStats>>,
}

/// Statistics for background optimization operations
#[derive(Debug, Clone, Default)]
pub struct BackgroundStats {
    /// Total optimization tasks executed
    pub tasks_executed: u64,
    /// Pool tuning operations performed
    pub tuning_operations: u64,
    /// Memory pressure events detected
    pub pressure_events: u64,
    /// Defragmentation operations performed
    pub defrag_operations: u64,
    /// Warm-up operations performed
    pub warmup_operations: u64,
    /// Cleanup operations performed
    pub cleanup_operations: u64,
    /// Last optimization run timestamp
    pub last_optimization: Option<Instant>,
    /// Total background CPU time (estimated)
    pub cpu_time_ms: u64,
}

impl BackgroundMemoryManager {
    /// Create a new background memory manager
    pub fn new(pool: Arc<SizedMemoryPool>) -> NgdpCacheResult<Self> {
        Self::with_config(pool, BackgroundConfig::default())
    }

    /// Create a new background memory manager with custom configuration
    pub fn with_config(
        pool: Arc<SizedMemoryPool>,
        config: BackgroundConfig,
    ) -> NgdpCacheResult<Self> {
        let (task_sender, task_receiver) = mpsc::unbounded_channel();

        let usage_patterns = Arc::new(RwLock::new(HashMap::new()));

        // Initialize usage patterns for all content types
        if let Ok(mut patterns) = usage_patterns.write() {
            for &content_type in ContentTypeHint::all() {
                patterns.insert(content_type, UsagePattern::new(content_type));
            }
        }

        Ok(Self {
            pool,
            config,
            usage_patterns,
            task_sender,
            task_receiver: Some(task_receiver),
            worker_handle: None,
            shutdown_sender: None,
            is_running: Arc::new(AtomicBool::new(false)),
            background_stats: Arc::new(RwLock::new(BackgroundStats::default())),
        })
    }

    /// Start the background optimization tasks
    pub fn start_optimization(&mut self) -> NgdpCacheResult<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(()); // Already running
        }

        let task_receiver = self.task_receiver.take().ok_or_else(|| {
            crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                "Background manager already started".to_string(),
            ))
        })?;

        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        self.shutdown_sender = Some(shutdown_sender);

        let pool = self.pool.clone();
        let config = self.config.clone();
        let usage_patterns = self.usage_patterns.clone();
        let is_running = self.is_running.clone();
        let background_stats = self.background_stats.clone();

        // Start background worker
        self.worker_handle = Some(tokio::spawn(async move {
            Self::background_worker(
                pool,
                config,
                usage_patterns,
                task_receiver,
                shutdown_receiver,
                is_running,
                background_stats,
            )
            .await;
        }));

        self.is_running.store(true, Ordering::Relaxed);

        // Schedule periodic tasks
        self.schedule_periodic_tasks()?;

        Ok(())
    }

    /// Schedule periodic optimization tasks
    fn schedule_periodic_tasks(&self) -> NgdpCacheResult<()> {
        // Schedule usage monitoring for all content types
        for &content_type in ContentTypeHint::all() {
            let task = OptimizationTask::MonitorUsage {
                content_type,
                interval: self.config.pattern_monitoring_interval,
            };
            self.task_sender.send(task).map_err(|_| {
                crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                    "Failed to schedule monitoring task".to_string(),
                ))
            })?;
        }

        // Schedule memory pressure checks
        let pressure_task = OptimizationTask::MemoryPressureCheck {
            pressure_threshold: self.config.memory_pressure_threshold,
            response: PressureResponse::LogWarning,
        };
        self.task_sender.send(pressure_task).map_err(|_| {
            crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                "Failed to schedule pressure check".to_string(),
            ))
        })?;

        // Schedule cleanup tasks
        let cleanup_task = OptimizationTask::CleanupUnused {
            max_age: Duration::from_secs(600), // 10 minutes
            min_pool_size: 1,
        };
        self.task_sender.send(cleanup_task).map_err(|_| {
            crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                "Failed to schedule cleanup task".to_string(),
            ))
        })?;

        Ok(())
    }

    /// Background worker that processes optimization tasks
    async fn background_worker(
        pool: Arc<SizedMemoryPool>,
        config: BackgroundConfig,
        usage_patterns: Arc<RwLock<HashMap<ContentTypeHint, UsagePattern>>>,
        mut task_receiver: mpsc::UnboundedReceiver<OptimizationTask>,
        mut shutdown_receiver: oneshot::Receiver<()>,
        is_running: Arc<AtomicBool>,
        background_stats: Arc<RwLock<BackgroundStats>>,
    ) {
        loop {
            tokio::select! {
                task = task_receiver.recv() => {
                    match task {
                        Some(task) => {
                            let start_time = Instant::now();
                            Self::execute_task(&pool, &config, &usage_patterns, task).await;

                            // Update stats
                            if let Ok(mut stats) = background_stats.write() {
                                stats.tasks_executed += 1;
                                stats.last_optimization = Some(Instant::now());
                                stats.cpu_time_ms += start_time.elapsed().as_millis() as u64;
                            }
                        }
                        None => break, // Channel closed
                    }
                }
                _ = &mut shutdown_receiver => {
                    break; // Shutdown requested
                }
            }
        }

        is_running.store(false, Ordering::Relaxed);
    }

    /// Execute a specific optimization task
    async fn execute_task(
        pool: &Arc<SizedMemoryPool>,
        config: &BackgroundConfig,
        usage_patterns: &Arc<RwLock<HashMap<ContentTypeHint, UsagePattern>>>,
        task: OptimizationTask,
    ) {
        match task {
            OptimizationTask::MonitorUsage {
                content_type,
                interval,
            } => {
                Self::monitor_usage_pattern(pool, usage_patterns, content_type).await;
                // Reschedule monitoring
                sleep(interval).await;
            }

            OptimizationTask::TunePoolSize {
                content_type,
                target_reuse_rate,
                max_adjustment,
            } => {
                Self::tune_pool_size(
                    pool,
                    usage_patterns,
                    content_type,
                    target_reuse_rate,
                    max_adjustment,
                );
            }

            OptimizationTask::DefragmentPool {
                content_type,
                fragmentation_threshold,
            } => {
                if config.enable_defragmentation {
                    Self::defragment_pool(pool, content_type, fragmentation_threshold);
                }
            }

            OptimizationTask::WarmUpPools { predictions } => {
                if config.enable_auto_warmup {
                    Self::warm_up_pools(pool, predictions).await;
                }
            }

            OptimizationTask::MemoryPressureCheck {
                pressure_threshold,
                response,
            } => {
                Self::check_memory_pressure(pool, pressure_threshold, response).await;
                // Reschedule pressure check
                sleep(config.pressure_check_interval).await;
            }

            OptimizationTask::CleanupUnused {
                max_age,
                min_pool_size,
            } => {
                Self::cleanup_unused_buffers(pool, max_age, min_pool_size);
                // Reschedule cleanup
                sleep(config.cleanup_interval).await;
            }
        }
    }

    /// Monitor usage patterns for a specific content type
    async fn monitor_usage_pattern(
        pool: &Arc<SizedMemoryPool>,
        usage_patterns: &Arc<RwLock<HashMap<ContentTypeHint, UsagePattern>>>,
        content_type: ContentTypeHint,
    ) {
        // Get current pool statistics
        if let Ok(stats) = pool.get_stats().await {
            let allocations = stats.allocations_by_type.get(&content_type).unwrap_or(&0);
            let reuses = stats.reuses_by_type.get(&content_type).unwrap_or(&0);
            let avg_size = stats.avg_size_by_type.get(&content_type).unwrap_or(&0);

            let was_reused = if *allocations > 0 {
                (*reuses as f64) / (*allocations as f64) > 0.5
            } else {
                false
            };

            // Update usage pattern
            if let Ok(mut patterns) = usage_patterns.write() {
                if let Some(pattern) = patterns.get_mut(&content_type) {
                    pattern.update_allocation(*avg_size, was_reused);
                }
            }
        }
    }

    /// Tune pool size based on usage patterns
    fn tune_pool_size(
        _pool: &Arc<SizedMemoryPool>,
        usage_patterns: &Arc<RwLock<HashMap<ContentTypeHint, UsagePattern>>>,
        content_type: ContentTypeHint,
        _target_reuse_rate: f32,
        _max_adjustment: f32,
    ) {
        // Get usage pattern and current pool stats
        if let Ok(patterns) = usage_patterns.read() {
            if let Some(pattern) = patterns.get(&content_type) {
                if pattern.needs_tuning() {
                    // In a full implementation, this would adjust the pool size
                    // For now, we just log the recommendation
                    #[allow(unused_variables)]
                    let recommended_size = pattern.recommended_pool_size();

                    #[cfg(feature = "tracing")]
                    tracing::info!(
                        "Pool tuning recommendation for {:?}: {} buffers (confidence: {:.2})",
                        content_type,
                        recommended_size,
                        pattern.confidence
                    );
                }
            }
        }
    }

    /// Defragment pool to reduce memory fragmentation
    fn defragment_pool(
        _pool: &Arc<SizedMemoryPool>,
        _content_type: ContentTypeHint,
        _fragmentation_threshold: f32,
    ) {
        // Defragmentation would be implemented here
        // This is a complex operation that would require careful coordination
        // with ongoing allocations
    }

    /// Warm up pools based on predicted usage
    async fn warm_up_pools(
        pool: &Arc<SizedMemoryPool>,
        predictions: Vec<(ContentTypeHint, usize)>,
    ) {
        for (content_type, count) in predictions {
            // Pre-allocate and deallocate buffers to warm the pool
            let typical_size = content_type.typical_size();

            for _ in 0..count {
                if let Ok(buffer) = pool.allocate_for_type(content_type, typical_size).await {
                    // Immediately return to pool to warm it up
                    let _ = pool.deallocate(buffer).await;
                }
            }
        }
    }

    /// Check for memory pressure and respond accordingly
    async fn check_memory_pressure(
        pool: &Arc<SizedMemoryPool>,
        pressure_threshold: f32,
        response: PressureResponse,
    ) {
        // Simple memory pressure detection (would be more sophisticated in production)
        let current_pressure = Self::estimate_memory_pressure();

        if current_pressure > pressure_threshold {
            match response {
                PressureResponse::ReducePools(percentage) => {
                    // Reduce pool sizes by percentage
                    let reduction_factor = 1.0 - (percentage as f32 / 100.0);
                    Self::scale_all_pools(pool, reduction_factor).await;
                }
                PressureResponse::ClearSmallPools => {
                    // Clear pools for small content types
                    let _ = pool.clear().await;
                }
                PressureResponse::EmergencyMode => {
                    // Switch to emergency allocation (bypass pools)
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Memory pressure emergency mode activated");
                }
                PressureResponse::LogWarning => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Memory pressure detected: {:.2}", current_pressure);
                }
            }
        }
    }

    /// Clean up unused buffers from pools
    fn cleanup_unused_buffers(
        _pool: &Arc<SizedMemoryPool>,
        _max_age: Duration,
        _min_pool_size: usize,
    ) {
        // Cleanup implementation would track buffer ages and remove old ones
        // This requires extending the pool implementation with age tracking
    }

    /// Estimate current memory pressure (simplified)
    fn estimate_memory_pressure() -> f32 {
        // This is a simplified implementation
        // In production, this would check system memory usage, swap usage, etc.
        0.3 // Return low pressure for demo
    }

    /// Scale all pools by a factor
    async fn scale_all_pools(pool: &Arc<SizedMemoryPool>, _factor: f32) {
        // This would scale pool sizes by the given factor
        // For now, we just clear the pools as a simple response
        let _ = pool.clear().await;
    }

    /// Submit a custom optimization task
    pub fn submit_task(&self, task: OptimizationTask) -> NgdpCacheResult<()> {
        self.task_sender.send(task).map_err(|_| {
            crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                "Failed to submit optimization task".to_string(),
            ))
        })?;
        Ok(())
    }

    /// Get current usage patterns for all content types
    pub fn get_usage_patterns(&self) -> NgdpCacheResult<HashMap<ContentTypeHint, UsagePattern>> {
        self.usage_patterns
            .read()
            .map(|patterns| patterns.clone())
            .map_err(|_| {
                crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                    "Failed to read usage patterns".to_string(),
                ))
            })
    }

    /// Get background optimization statistics
    pub fn get_background_stats(&self) -> NgdpCacheResult<BackgroundStats> {
        self.background_stats
            .read()
            .map(|stats| stats.clone())
            .map_err(|_| {
                crate::error::NgdpCacheError::Cache(crate::error::CacheError::Backend(
                    "Failed to read background stats".to_string(),
                ))
            })
    }

    /// Check if background optimization is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Shutdown the background optimization tasks
    pub async fn shutdown(&mut self) -> NgdpCacheResult<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Send shutdown signal
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }

        // Wait for background worker to finish
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.await;
        }

        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Force trigger pool tuning for all content types
    pub fn trigger_tuning(&self) -> NgdpCacheResult<()> {
        for &content_type in ContentTypeHint::all() {
            let task = OptimizationTask::TunePoolSize {
                content_type,
                target_reuse_rate: 0.7,
                max_adjustment: 0.3,
            };
            self.submit_task(task)?;
        }
        Ok(())
    }

    /// Force memory pressure response
    pub fn trigger_pressure_response(&self, response: PressureResponse) -> NgdpCacheResult<()> {
        let task = OptimizationTask::MemoryPressureCheck {
            pressure_threshold: 0.0, // Force trigger
            response,
        };
        self.submit_task(task)
    }

    /// Update background configuration
    pub fn update_config(&mut self, config: BackgroundConfig) {
        self.config = config;
    }
}

// Implement Drop to ensure graceful shutdown
impl Drop for BackgroundMemoryManager {
    fn drop(&mut self) {
        if self.is_running.load(Ordering::Relaxed) {
            // Send shutdown signal if still running
            if let Some(shutdown_sender) = self.shutdown_sender.take() {
                let _ = shutdown_sender.send(());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_hint_characteristics() {
        // Test typical sizes
        assert_eq!(ContentTypeHint::Config.typical_size(), 16 * 1024);
        assert_eq!(ContentTypeHint::Encoding.typical_size(), 16 * 1024 * 1024);
        assert_eq!(ContentTypeHint::Archive.typical_size(), 8 * 1024 * 1024);
        assert_eq!(ContentTypeHint::Root.typical_size(), 2 * 1024 * 1024);

        // Test size class mappings
        assert_eq!(ContentTypeHint::Config.size_class(), NgdpSizeClass::Small);
        assert_eq!(ContentTypeHint::Encoding.size_class(), NgdpSizeClass::Huge);
        assert_eq!(ContentTypeHint::Archive.size_class(), NgdpSizeClass::Large);

        // Test access patterns
        let config_pattern = ContentTypeHint::Config.access_pattern();
        assert!(config_pattern.sequential);
        assert!(config_pattern.burst_likely);
        assert!(config_pattern.reuse_probability > 0.5);

        let encoding_pattern = ContentTypeHint::Encoding.access_pattern();
        assert!(encoding_pattern.sequential);
        assert!(!encoding_pattern.burst_likely);
    }

    #[test]
    fn test_content_type_hint_lifetimes() {
        // Config files should have short to medium lifetime
        let config_lifetime = ContentTypeHint::Config.expected_lifetime();
        assert!(config_lifetime >= Duration::from_secs(60));
        assert!(config_lifetime <= Duration::from_secs(3600));

        // Encoding files should have long lifetime
        let encoding_lifetime = ContentTypeHint::Encoding.expected_lifetime();
        assert!(encoding_lifetime >= Duration::from_secs(3600));

        // BLTE content should have short lifetime
        let blte_lifetime = ContentTypeHint::Blte.expected_lifetime();
        assert!(blte_lifetime <= Duration::from_secs(300));
    }

    #[tokio::test]
    async fn test_sized_memory_pool_basic_operations() {
        let pool = SizedMemoryPool::new();

        // Test allocation for different content types
        let config_buffer = pool
            .allocate_for_type(ContentTypeHint::Config, 8192)
            .await
            .expect("Test operation should succeed");
        assert!(config_buffer.capacity() >= 8192);

        let encoding_buffer = pool
            .allocate_for_type(ContentTypeHint::Encoding, 1024 * 1024)
            .await
            .expect("Test operation should succeed");
        assert!(encoding_buffer.capacity() >= 1024 * 1024);

        // Test deallocation
        pool.deallocate(config_buffer)
            .await
            .expect("Test operation should succeed");
        pool.deallocate(encoding_buffer)
            .await
            .expect("Test operation should succeed");
    }

    #[tokio::test]
    async fn test_sized_memory_pool_statistics() {
        let pool = SizedMemoryPool::new();

        // Perform some allocations
        let _buf1 = pool
            .allocate_for_type(ContentTypeHint::Config, 4096)
            .await
            .expect("Test operation should succeed");
        let _buf2 = pool
            .allocate_for_type(ContentTypeHint::Config, 8192)
            .await
            .expect("Test operation should succeed");
        let _buf3 = pool
            .allocate_for_type(ContentTypeHint::Encoding, 1024 * 1024)
            .await
            .expect("Test operation should succeed");

        let stats = pool
            .get_stats()
            .await
            .expect("Test operation should succeed");

        // Check that statistics were recorded
        assert_eq!(stats.total_allocations(), 3);
        assert_eq!(
            *stats
                .allocations_by_type
                .get(&ContentTypeHint::Config)
                .expect("Operation should succeed"),
            2
        );
        assert_eq!(
            *stats
                .allocations_by_type
                .get(&ContentTypeHint::Encoding)
                .expect("Operation should succeed"),
            1
        );

        // Check bytes allocated
        assert!(stats.total_bytes() > 0);
        assert!(
            *stats
                .bytes_by_type
                .get(&ContentTypeHint::Config)
                .expect("Operation should succeed")
                >= 16 * 1024
        );
        assert!(
            *stats
                .bytes_by_type
                .get(&ContentTypeHint::Encoding)
                .expect("Operation should succeed")
                >= 14 * 1024 * 1024
        );
    }

    #[tokio::test]
    async fn test_sized_memory_pool_reuse() {
        let pool = SizedMemoryPool::new();

        // Allocate and deallocate to populate pool
        let buffer = pool
            .allocate_for_type(ContentTypeHint::Config, 4096)
            .await
            .expect("Test operation should succeed");
        pool.deallocate(buffer)
            .await
            .expect("Test operation should succeed");

        // Next allocation should potentially reuse
        let _reused_buffer = pool
            .allocate_for_type(ContentTypeHint::Config, 4096)
            .await
            .expect("Test operation should succeed");

        let stats = pool
            .get_stats()
            .await
            .expect("Test operation should succeed");
        assert!(stats.total_allocations() >= 2);
    }

    #[tokio::test]
    async fn test_sized_memory_pool_warm_up() {
        let pool = SizedMemoryPool::new();

        // Warm up the pools
        pool.warm_up().await.expect("Test operation should succeed");

        // Allocations after warm-up should be faster (hard to test directly)
        let _buffer = pool
            .allocate_for_type(ContentTypeHint::Config, 4096)
            .await
            .expect("Test operation should succeed");
    }

    #[tokio::test]
    async fn test_sized_memory_pool_clear() {
        let pool = SizedMemoryPool::new();

        // Make some allocations
        let _buf = pool
            .allocate_for_type(ContentTypeHint::Config, 4096)
            .await
            .expect("Test operation should succeed");

        // Clear the pools
        pool.clear().await.expect("Test operation should succeed");

        // Statistics should be reset
        let stats = pool
            .get_stats()
            .await
            .expect("Test operation should succeed");
        assert_eq!(stats.total_allocations(), 0);
        assert_eq!(stats.total_bytes(), 0);
    }

    #[test]
    fn test_memory_pool_stats_operations() {
        let mut stats = MemoryPoolStats::new();

        // Initially empty
        assert_eq!(stats.total_allocations(), 0);
        assert_eq!(stats.total_bytes(), 0);
        assert_eq!(stats.reuse_rate(), 0.0);

        // Add some data
        stats
            .allocations_by_type
            .insert(ContentTypeHint::Config, 10);
        stats.bytes_by_type.insert(ContentTypeHint::Config, 1024);
        stats.reuses_by_type.insert(ContentTypeHint::Config, 3);

        assert_eq!(stats.total_allocations(), 10);
        assert_eq!(stats.total_bytes(), 1024);
        assert_eq!(stats.reuse_rate(), 0.3);
        assert_eq!(stats.reuse_rate_for_type(ContentTypeHint::Config), 0.3);
        assert_eq!(stats.reuse_rate_for_type(ContentTypeHint::Encoding), 0.0);
    }

    #[test]
    fn test_access_pattern_characteristics() {
        let config_pattern = ContentTypeHint::Config.access_pattern();
        assert!(config_pattern.sequential);
        assert!(config_pattern.burst_likely);

        let archive_pattern = ContentTypeHint::Archive.access_pattern();
        assert!(archive_pattern.random);
        assert!(archive_pattern.burst_likely);

        let encoding_pattern = ContentTypeHint::Encoding.access_pattern();
        assert!(encoding_pattern.sequential);
        assert!(!encoding_pattern.burst_likely);
    }

    #[test]
    fn test_ngdp_bytes_pool_integration() {
        let buffer = bytes::BytesMut::with_capacity(1024);

        // Test creating NgdpBytes from pool buffer without key
        let ngdp_bytes = crate::validation::NgdpBytes::from_pool_buffer(buffer, None);
        assert_eq!(ngdp_bytes.len(), 0);

        // Test extracting bytes for deallocation
        let extracted_bytes = ngdp_bytes.into_bytes_for_deallocation();
        assert_eq!(extracted_bytes.len(), 0);
    }

    #[test]
    fn test_content_type_hint_all() {
        let all_types = ContentTypeHint::all();
        assert_eq!(all_types.len(), 8);
        assert!(all_types.contains(&ContentTypeHint::Config));
        assert!(all_types.contains(&ContentTypeHint::Encoding));
        assert!(all_types.contains(&ContentTypeHint::Archive));
        assert!(all_types.contains(&ContentTypeHint::Root));
        assert!(all_types.contains(&ContentTypeHint::Install));
        assert!(all_types.contains(&ContentTypeHint::Download));
        assert!(all_types.contains(&ContentTypeHint::Blte));
        assert!(all_types.contains(&ContentTypeHint::Generic));
    }

    #[tokio::test]
    async fn test_background_memory_manager_creation() {
        let pool = Arc::new(SizedMemoryPool::new());
        let bg_manager =
            BackgroundMemoryManager::with_config(pool, BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        assert!(!bg_manager.is_running());
    }

    #[tokio::test]
    async fn test_background_memory_manager_start_stop() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool.clone(), BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        // Start optimization
        bg_manager
            .start_optimization()
            .expect("Test operation should succeed");
        assert!(bg_manager.is_running());

        // Allow some time for tasks to be scheduled (shorter for test config)
        tokio::time::sleep(Duration::from_millis(25)).await;

        // Stop optimization
        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed");
        assert!(!bg_manager.is_running());
    }

    #[tokio::test]
    async fn test_usage_pattern_tracking() {
        let mut pattern = UsagePattern::new(ContentTypeHint::Config);

        assert_eq!(pattern.confidence, 0.0);
        assert_eq!(pattern.current_reuse_rate, 0.0);

        // Simulate some allocations
        pattern.update_allocation(16384, true); // reused
        pattern.update_allocation(8192, false); // not reused
        pattern.update_allocation(32768, true); // reused

        assert!(pattern.confidence > 0.0);
        assert!(pattern.current_reuse_rate > 0.0);
        assert!(pattern.avg_allocation_size > 0);
    }

    #[tokio::test]
    async fn test_usage_pattern_recommendations() {
        let mut pattern = UsagePattern::new(ContentTypeHint::Config);

        // Build up confidence with many allocations and low reuse rate to trigger tuning
        for i in 0..60 {
            let size = 16384 + (i * 100);
            let was_reused = i % 5 == 0; // 20% reuse rate (low)
            pattern.update_allocation(size, was_reused);
        }

        assert!(pattern.confidence > 0.5);
        let recommended_size = pattern.recommended_pool_size();
        assert!(recommended_size > 0);

        // Pattern should suggest tuning with low reuse rate
        assert!(pattern.needs_tuning());
    }

    #[tokio::test]
    async fn test_pool_tuning_strategy() {
        let conservative = PoolTuningStrategy::conservative();
        let aggressive = PoolTuningStrategy::aggressive();

        let mut pattern = UsagePattern::new(ContentTypeHint::Config);

        // Build pattern with high confidence and low reuse rate to trigger tuning
        for i in 0..100 {
            pattern.update_allocation(16384, i % 10 == 0); // 10% reuse rate (very low)
        }

        let current_size = 32;
        let conservative_adj = conservative.calculate_adjustment(&pattern, current_size);
        let aggressive_adj = aggressive.calculate_adjustment(&pattern, current_size);

        // Both should suggest adjustments for very low reuse rate
        assert!(conservative_adj.is_some());
        assert!(aggressive_adj.is_some());
    }

    #[tokio::test]
    async fn test_background_config_defaults() {
        let config = BackgroundConfig::default();

        assert!(config.base_interval > Duration::from_secs(0));
        assert!(config.memory_pressure_threshold > 0.0);
        assert!(config.memory_pressure_threshold <= 1.0);
        assert!(config.enable_auto_warmup);
        assert!(!config.enable_defragmentation); // Conservative default
    }

    #[tokio::test]
    async fn test_optimization_task_creation() {
        let monitor_task = OptimizationTask::MonitorUsage {
            content_type: ContentTypeHint::Config,
            interval: Duration::from_secs(30),
        };

        let tune_task = OptimizationTask::TunePoolSize {
            content_type: ContentTypeHint::Encoding,
            target_reuse_rate: 0.7,
            max_adjustment: 0.2,
        };

        let pressure_task = OptimizationTask::MemoryPressureCheck {
            pressure_threshold: 0.85,
            response: PressureResponse::LogWarning,
        };

        // Tasks should be created without panicking
        match monitor_task {
            OptimizationTask::MonitorUsage {
                content_type,
                interval,
            } => {
                assert_eq!(content_type, ContentTypeHint::Config);
                assert_eq!(interval, Duration::from_secs(30));
            }
            _ => unreachable!("Expected MonitorUsage task"),
        }

        match tune_task {
            OptimizationTask::TunePoolSize {
                content_type,
                target_reuse_rate,
                max_adjustment,
            } => {
                assert_eq!(content_type, ContentTypeHint::Encoding);
                assert_eq!(target_reuse_rate, 0.7);
                assert_eq!(max_adjustment, 0.2);
            }
            _ => unreachable!("Expected TunePoolSize task"),
        }

        match pressure_task {
            OptimizationTask::MemoryPressureCheck {
                pressure_threshold,
                response,
            } => {
                assert_eq!(pressure_threshold, 0.85);
                assert_eq!(response, PressureResponse::LogWarning);
            }
            _ => unreachable!("Expected MemoryPressureCheck task"),
        }
    }

    #[tokio::test]
    async fn test_background_manager_task_submission() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool.clone(), BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        bg_manager
            .start_optimization()
            .expect("Test operation should succeed");

        let task = OptimizationTask::MonitorUsage {
            content_type: ContentTypeHint::Archive,
            interval: Duration::from_millis(5), // Fast interval for testing
        };

        // Should be able to submit custom tasks
        let result = bg_manager.submit_task(task);
        assert!(result.is_ok());

        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed");
    }

    #[tokio::test]
    async fn test_background_manager_statistics() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool.clone(), BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        bg_manager
            .start_optimization()
            .expect("Test operation should succeed");

        // Allow some background processing (shorter for test config)
        tokio::time::sleep(Duration::from_millis(15)).await;

        let stats = bg_manager
            .get_background_stats()
            .expect("Test operation should succeed");
        // With fast test intervals, some tasks may have executed
        // Just verify we can get stats successfully
        let _ = stats.tasks_executed; // Verify field exists and is accessible

        let patterns = bg_manager
            .get_usage_patterns()
            .expect("Test operation should succeed");
        assert_eq!(patterns.len(), ContentTypeHint::all().len());

        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed");
    }

    #[tokio::test]
    async fn test_background_manager_trigger_operations() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool.clone(), BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        bg_manager
            .start_optimization()
            .expect("Test operation should succeed");

        // Test trigger operations
        let tuning_result = bg_manager.trigger_tuning();
        assert!(tuning_result.is_ok());

        let pressure_result = bg_manager.trigger_pressure_response(PressureResponse::LogWarning);
        assert!(pressure_result.is_ok());

        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed");
    }

    #[tokio::test]
    async fn test_pressure_response_variants() {
        let responses = [
            PressureResponse::ReducePools(25),
            PressureResponse::ClearSmallPools,
            PressureResponse::EmergencyMode,
            PressureResponse::LogWarning,
        ];

        // All variants should be creatable
        for response in responses {
            match response {
                PressureResponse::ReducePools(pct) => assert!(pct <= 100),
                _ => {} // Other variants are simple
            }
        }
    }

    #[tokio::test]
    async fn test_usage_pattern_trend_calculation() {
        let mut pattern = UsagePattern::new(ContentTypeHint::Config);

        // Simulate increasing allocation sizes
        for i in 1..=20 {
            pattern.update_allocation(1000 * i, i % 2 == 0);
        }

        // Should detect increasing trend
        assert!(
            pattern.trend > 0.0,
            "Expected positive trend for increasing sizes"
        );
        assert!(pattern.confidence > 0.1);
    }

    #[tokio::test]
    async fn test_background_manager_config_update() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool, BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        let new_config = BackgroundConfig {
            memory_pressure_threshold: 0.9,
            enable_defragmentation: true,
            ..BackgroundConfig::test_config()
        };

        bg_manager.update_config(new_config);

        // Configuration should be updated (we can't easily test this without exposing config)
        // But at least the operation should not panic
        assert!(!bg_manager.is_running()); // Should still not be running
    }

    #[tokio::test]
    async fn test_background_manager_double_start_stop() {
        let pool = Arc::new(SizedMemoryPool::new());
        let mut bg_manager =
            BackgroundMemoryManager::with_config(pool.clone(), BackgroundConfig::test_config())
                .expect("Test operation should succeed");

        // Starting twice should be safe
        bg_manager
            .start_optimization()
            .expect("Test operation should succeed");
        bg_manager
            .start_optimization()
            .expect("Test operation should succeed"); // Should not error
        assert!(bg_manager.is_running());

        // Stopping twice should be safe
        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed");
        bg_manager
            .shutdown()
            .await
            .expect("Test operation should succeed"); // Should not error
        assert!(!bg_manager.is_running());
    }
}
