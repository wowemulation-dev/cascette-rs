//! Memory pooling system for NGDP cache workloads
//!
//! This module provides high-performance memory pooling optimized for NGDP/CASC
//! file access patterns. NGDP files are typically large and accessed in bursts during game patches.
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss

use bytes::{Bytes, BytesMut};
use std::{
    collections::VecDeque,
    sync::{
        Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

/// Size class for NGDP-specific file types
/// Optimized for common NGDP file sizes observed in the wild
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NgdpSizeClass {
    /// Small files: Ribbit responses, config files (< 16KB)
    Small = 0,
    /// Medium files: Archive indices, patch manifests (16KB - 256KB)
    Medium = 1,
    /// Large files: Root files, install manifests (256KB - 8MB)
    Large = 2,
    /// Huge files: Encoding files, large archives (8MB - 32MB)
    Huge = 3,
}

impl NgdpSizeClass {
    /// Get size class for a given byte count
    pub fn from_size(size: usize) -> Self {
        match size {
            0..=16384 => Self::Small,           // 16KB
            16385..=262_144 => Self::Medium,    // 256KB
            262_145..=8_388_608 => Self::Large, // 8MB
            _ => Self::Huge,                    // > 8MB
        }
    }

    /// Get the buffer size for this class
    pub fn buffer_size(self) -> usize {
        match self {
            Self::Small => 16 * 1024,       // 16KB
            Self::Medium => 256 * 1024,     // 256KB
            Self::Large => 8 * 1024 * 1024, // 8MB
            Self::Huge => 32 * 1024 * 1024, // 32MB
        }
    }

    /// Get the maximum pool size for this class
    pub fn max_pool_size(self) -> usize {
        match self {
            Self::Small => 64,  // Up to 1MB pooled
            Self::Medium => 32, // Up to 8MB pooled
            Self::Large => 8,   // Up to 64MB pooled
            Self::Huge => 2,    // Up to 64MB pooled
        }
    }

    /// Get all size classes in order
    pub fn all() -> &'static [Self] {
        &[Self::Small, Self::Medium, Self::Large, Self::Huge]
    }
}

/// Statistics for a memory pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total allocations served by the pool
    pub allocations: u64,
    /// Total bytes allocated
    pub bytes_allocated: u64,
    /// Number of buffer reuses (cache hits)
    pub reuses: u64,
    /// Current number of buffers in pool
    pub pool_size: usize,
    /// Maximum pool size reached
    pub max_pool_size: usize,
    /// Number of times pool was empty
    pub pool_misses: u64,
    /// Average allocation size
    pub avg_allocation_size: usize,
}

impl PoolStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            allocations: 0,
            bytes_allocated: 0,
            reuses: 0,
            pool_size: 0,
            max_pool_size: 0,
            pool_misses: 0,
            avg_allocation_size: 0,
        }
    }

    /// Calculate reuse rate (percentage of allocations served from pool)
    pub fn reuse_rate(&self) -> f64 {
        if self.allocations == 0 {
            0.0
        } else {
            (self.reuses as f64) / (self.allocations as f64)
        }
    }
}

impl Default for PoolStats {
    fn default() -> Self {
        Self::new()
    }
}

/// A single memory pool for a specific size class
struct SizeClassPool {
    /// Queue of available buffers
    buffers: Mutex<VecDeque<BytesMut>>,
    /// Size class this pool manages
    size_class: NgdpSizeClass,
    /// Current pool size (lock-free for monitoring)
    current_size: AtomicUsize,
    /// Maximum pool size allowed
    max_size: usize,
    /// Pool statistics
    stats: RwLock<PoolStats>,
}

impl SizeClassPool {
    /// Create a new size class pool
    fn new(size_class: NgdpSizeClass) -> Self {
        Self {
            buffers: Mutex::new(VecDeque::new()),
            size_class,
            current_size: AtomicUsize::new(0),
            max_size: size_class.max_pool_size(),
            stats: RwLock::new(PoolStats::new()),
        }
    }

    /// Allocate a buffer, reusing from pool if available
    fn allocate(&self, size: usize) -> BytesMut {
        let buffer_size = self.size_class.buffer_size();

        // Try to reuse from pool first
        if let Ok(mut buffers) = self.buffers.try_lock()
            && let Some(mut buffer) = buffers.pop_front()
        {
            self.current_size.fetch_sub(1, Ordering::Relaxed);

            // Reset buffer for reuse
            buffer.clear();
            buffer.reserve(size.max(buffer_size));

            // Update stats
            if let Ok(mut stats) = self.stats.try_write() {
                stats.allocations += 1;
                stats.bytes_allocated += size as u64;
                stats.reuses += 1;
                stats.avg_allocation_size =
                    (stats.bytes_allocated / stats.allocations.max(1)) as usize;
            }

            return buffer;
        }

        // Pool miss - allocate new buffer
        let buffer = BytesMut::with_capacity(size.max(buffer_size));

        // Update stats
        if let Ok(mut stats) = self.stats.try_write() {
            stats.allocations += 1;
            stats.bytes_allocated += size as u64;
            stats.pool_misses += 1;
            stats.avg_allocation_size = (stats.bytes_allocated / stats.allocations.max(1)) as usize;
        }

        buffer
    }

    /// Return a buffer to the pool for reuse
    fn deallocate(&self, buffer: BytesMut) {
        let current_size = self.current_size.load(Ordering::Relaxed);

        // Only keep buffer if pool isn't full
        if current_size < self.max_size
            && let Ok(mut buffers) = self.buffers.try_lock()
            && buffers.len() < self.max_size
        {
            buffers.push_back(buffer);
            let new_size = self.current_size.fetch_add(1, Ordering::Relaxed) + 1;

            // Update max pool size stats
            if let Ok(mut stats) = self.stats.try_write() {
                stats.pool_size = new_size;
                stats.max_pool_size = stats.max_pool_size.max(new_size);
            }
        }
        // If pool is full or lock contention, buffer is dropped
    }

    /// Get current statistics
    fn stats(&self) -> PoolStats {
        let current_size = self.current_size.load(Ordering::Relaxed);

        if let Ok(mut stats) = self.stats.try_write() {
            stats.pool_size = current_size;
            stats.clone()
        } else if let Ok(stats) = self.stats.try_read() {
            let mut stats_copy = stats.clone();
            stats_copy.pool_size = current_size;
            stats_copy
        } else {
            PoolStats::new()
        }
    }

    /// Clear all buffers from the pool
    fn clear(&self) {
        if let Ok(mut buffers) = self.buffers.try_lock() {
            buffers.clear();
            self.current_size.store(0, Ordering::Relaxed);
        }
    }
}

/// High-performance memory pool optimized for NGDP workloads
///
/// Provides separate pools for different size classes to minimize memory
/// fragmentation and improve cache locality for NGDP file access patterns.
pub struct NgdpMemoryPool {
    /// Pools for each size class
    pools: [SizeClassPool; 4],
    /// Pool creation time for metrics
    created_at: Instant,
}

impl NgdpMemoryPool {
    /// Create a new NGDP memory pool
    pub fn new() -> Self {
        Self {
            pools: [
                SizeClassPool::new(NgdpSizeClass::Small),
                SizeClassPool::new(NgdpSizeClass::Medium),
                SizeClassPool::new(NgdpSizeClass::Large),
                SizeClassPool::new(NgdpSizeClass::Huge),
            ],
            created_at: Instant::now(),
        }
    }

    /// Allocate a buffer optimized for the given size
    ///
    /// Returns a `BytesMut` that may be reused from the pool or freshly allocated.
    /// The returned buffer will have at least `size` capacity.
    pub fn allocate(&self, size: usize) -> BytesMut {
        let size_class = NgdpSizeClass::from_size(size);
        self.pools[size_class as usize].allocate(size)
    }

    /// Allocate a buffer and convert to `Bytes`
    ///
    /// This is a convenience method for cases where immutable bytes are needed.
    pub fn allocate_bytes(&self, size: usize) -> Bytes {
        self.allocate(size).freeze()
    }

    /// Return a buffer to the pool for potential reuse
    ///
    /// The buffer should be cleared/reset before being returned to avoid
    /// data leaks between allocations.
    pub fn deallocate(&self, buffer: BytesMut) {
        let size_class = NgdpSizeClass::from_size(buffer.capacity());
        self.pools[size_class as usize].deallocate(buffer);
    }

    /// Get statistics for a specific size class
    pub fn size_class_stats(&self, size_class: NgdpSizeClass) -> PoolStats {
        self.pools[size_class as usize].stats()
    }

    /// Get aggregated statistics across all pools
    pub fn total_stats(&self) -> NgdpPoolStats {
        let mut total_stats = NgdpPoolStats::new();

        for (i, pool) in self.pools.iter().enumerate() {
            let stats = pool.stats();
            let _size_class = NgdpSizeClass::all()[i]; // Prefix with _ to suppress warning

            total_stats.allocations += stats.allocations;
            total_stats.bytes_allocated += stats.bytes_allocated;
            total_stats.reuses += stats.reuses;
            total_stats.pool_misses += stats.pool_misses;
            total_stats.size_class_stats[i] = stats;
        }

        total_stats.created_at = self.created_at;

        if total_stats.allocations > 0 {
            total_stats.avg_allocation_size =
                (total_stats.bytes_allocated / total_stats.allocations) as usize;
        }

        total_stats
    }

    /// Clear all pools and free memory
    pub fn clear(&self) {
        for pool in &self.pools {
            pool.clear();
        }
    }

    /// Get the age of the pool
    pub fn age(&self) -> std::time::Duration {
        Instant::now() - self.created_at
    }

    /// Warm up the pools with pre-allocated buffers
    ///
    /// This can improve performance by pre-allocating buffers for common
    /// NGDP file access patterns during application startup.
    pub fn warm_up(&self) {
        for size_class in NgdpSizeClass::all() {
            let pool = &self.pools[*size_class as usize];
            let warm_count = (size_class.max_pool_size() / 2).max(1);

            // Pre-allocate and immediately return buffers to warm the pool
            for _ in 0..warm_count {
                let buffer = BytesMut::with_capacity(size_class.buffer_size());
                pool.deallocate(buffer);
            }
        }
    }
}

impl Default for NgdpMemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive statistics for the NGDP memory pool
#[derive(Debug, Clone)]
pub struct NgdpPoolStats {
    /// Total allocations across all size classes
    pub allocations: u64,
    /// Total bytes allocated
    pub bytes_allocated: u64,
    /// Total reuses from pool
    pub reuses: u64,
    /// Total pool misses (had to allocate new)
    pub pool_misses: u64,
    /// Average allocation size
    pub avg_allocation_size: usize,
    /// Statistics per size class
    pub size_class_stats: [PoolStats; 4],
    /// Pool creation time
    pub created_at: Instant,
}

impl NgdpPoolStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            allocations: 0,
            bytes_allocated: 0,
            reuses: 0,
            pool_misses: 0,
            avg_allocation_size: 0,
            size_class_stats: [
                PoolStats::new(),
                PoolStats::new(),
                PoolStats::new(),
                PoolStats::new(),
            ],
            created_at: Instant::now(),
        }
    }

    /// Calculate overall reuse rate
    pub fn reuse_rate(&self) -> f64 {
        if self.allocations == 0 {
            0.0
        } else {
            (self.reuses as f64) / (self.allocations as f64)
        }
    }

    /// Calculate pool miss rate
    pub fn miss_rate(&self) -> f64 {
        if self.allocations == 0 {
            0.0
        } else {
            (self.pool_misses as f64) / (self.allocations as f64)
        }
    }

    /// Get the age of the pool
    pub fn age(&self) -> std::time::Duration {
        Instant::now() - self.created_at
    }

    /// Get statistics for a specific size class
    pub fn get_size_class_stats(&self, size_class: NgdpSizeClass) -> &PoolStats {
        &self.size_class_stats[size_class as usize]
    }

    /// Calculate memory efficiency (bytes in pool vs total allocated)
    pub fn memory_efficiency(&self) -> f64 {
        let total_pooled_memory: usize = self
            .size_class_stats
            .iter()
            .zip(NgdpSizeClass::all().iter())
            .map(|(stats, size_class)| stats.pool_size * size_class.buffer_size())
            .sum();

        if self.bytes_allocated == 0 {
            0.0
        } else {
            (total_pooled_memory as f64) / (self.bytes_allocated as f64)
        }
    }
}

impl Default for NgdpPoolStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-local memory pool for zero-contention allocations
///
/// Uses thread-local storage to provide lock-free allocations for
/// high-frequency NGDP operations.
#[allow(clippy::struct_field_names)] // Field names describe their purpose clearly
pub struct ThreadLocalPool {
    /// Small buffer pool (local to thread)
    small_buffers: VecDeque<BytesMut>,
    /// Medium buffer pool (local to thread)
    medium_buffers: VecDeque<BytesMut>,
    /// Maximum buffers per size class
    max_buffers: usize,
}

impl ThreadLocalPool {
    /// Create a new thread-local pool
    fn new() -> Self {
        Self {
            small_buffers: VecDeque::new(),
            medium_buffers: VecDeque::new(),
            max_buffers: 8, // Limit per-thread memory usage
        }
    }

    /// Allocate from thread-local pool
    pub fn allocate(&mut self, size: usize) -> BytesMut {
        let size_class = NgdpSizeClass::from_size(size);
        let buffer_size = size_class.buffer_size();

        let buffers = match size_class {
            NgdpSizeClass::Small => &mut self.small_buffers,
            NgdpSizeClass::Medium => &mut self.medium_buffers,
            // Large and huge allocations bypass thread-local pool
            _ => {
                return BytesMut::with_capacity(size.max(buffer_size));
            }
        };

        if let Some(mut buffer) = buffers.pop_front() {
            buffer.clear();
            if buffer.capacity() < size {
                buffer.reserve(size - buffer.capacity());
            }
            buffer
        } else {
            BytesMut::with_capacity(size.max(buffer_size))
        }
    }

    /// Return buffer to thread-local pool
    pub fn deallocate(&mut self, buffer: BytesMut) {
        let size_class = NgdpSizeClass::from_size(buffer.capacity());

        let buffers = match size_class {
            NgdpSizeClass::Small => &mut self.small_buffers,
            NgdpSizeClass::Medium => &mut self.medium_buffers,
            _ => return, // Large/huge buffers are dropped
        };

        if buffers.len() < self.max_buffers {
            buffers.push_back(buffer);
        }
        // If pool is full, buffer is dropped
    }

    /// Clear the thread-local pool
    pub fn clear(&mut self) {
        self.small_buffers.clear();
        self.medium_buffers.clear();
    }
}

thread_local! {
    static THREAD_POOL: std::cell::RefCell<ThreadLocalPool> =
        std::cell::RefCell::new(ThreadLocalPool::new());
}

/// Allocate from thread-local pool for maximum performance
///
/// This provides the fastest allocation path for small to medium
/// NGDP allocations by avoiding any synchronization overhead.
pub fn allocate_thread_local(size: usize) -> BytesMut {
    THREAD_POOL.with(|pool| pool.borrow_mut().allocate(size))
}

/// Return buffer to thread-local pool
pub fn deallocate_thread_local(buffer: BytesMut) {
    THREAD_POOL.with(|pool| pool.borrow_mut().deallocate(buffer));
}

/// Clear thread-local pool (useful for tests or memory cleanup)
pub fn clear_thread_local_pool() {
    THREAD_POOL.with(|pool| pool.borrow_mut().clear());
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_size_class_classification() {
        assert_eq!(NgdpSizeClass::from_size(1024), NgdpSizeClass::Small);
        assert_eq!(NgdpSizeClass::from_size(16384), NgdpSizeClass::Small);
        assert_eq!(NgdpSizeClass::from_size(16385), NgdpSizeClass::Medium);
        assert_eq!(NgdpSizeClass::from_size(262_144), NgdpSizeClass::Medium);
        assert_eq!(NgdpSizeClass::from_size(262_145), NgdpSizeClass::Large);
        assert_eq!(
            NgdpSizeClass::from_size(8 * 1024 * 1024),
            NgdpSizeClass::Large
        );
        assert_eq!(
            NgdpSizeClass::from_size(8 * 1024 * 1024 + 1),
            NgdpSizeClass::Huge
        );
        assert_eq!(
            NgdpSizeClass::from_size(32 * 1024 * 1024),
            NgdpSizeClass::Huge
        );
    }

    #[test]
    fn test_size_class_buffer_sizes() {
        assert_eq!(NgdpSizeClass::Small.buffer_size(), 16 * 1024);
        assert_eq!(NgdpSizeClass::Medium.buffer_size(), 256 * 1024);
        assert_eq!(NgdpSizeClass::Large.buffer_size(), 8 * 1024 * 1024);
        assert_eq!(NgdpSizeClass::Huge.buffer_size(), 32 * 1024 * 1024);
    }

    #[test]
    fn test_memory_pool_basic_allocation() {
        let pool = NgdpMemoryPool::new();

        // Test small allocation
        let small_buffer = pool.allocate(1024);
        assert!(small_buffer.capacity() >= 1024);

        // Test medium allocation
        let medium_buffer = pool.allocate(128 * 1024);
        assert!(medium_buffer.capacity() >= 128 * 1024);

        // Test large allocation
        let large_buffer = pool.allocate(4 * 1024 * 1024);
        assert!(large_buffer.capacity() >= 4 * 1024 * 1024);

        // Test huge allocation
        let huge_buffer = pool.allocate(16 * 1024 * 1024);
        assert!(huge_buffer.capacity() >= 16 * 1024 * 1024);
    }

    #[test]
    fn test_memory_pool_reuse() {
        let pool = NgdpMemoryPool::new();

        // Allocate and deallocate a buffer
        let buffer = pool.allocate(1024);
        let capacity = buffer.capacity();
        pool.deallocate(buffer);

        // Next allocation should reuse the buffer
        let reused_buffer = pool.allocate(1024);
        assert_eq!(reused_buffer.capacity(), capacity);

        // Check statistics
        let stats = pool.size_class_stats(NgdpSizeClass::Small);
        assert_eq!(stats.allocations, 2);
        assert_eq!(stats.reuses, 1);
    }

    #[test]
    fn test_pool_statistics() {
        let pool = NgdpMemoryPool::new();

        // Perform some allocations
        let _buf1 = pool.allocate(1024);
        let _buf2 = pool.allocate(128 * 1024);
        let buf3 = pool.allocate(4 * 1024 * 1024);

        // Return one buffer
        pool.deallocate(buf3);

        let total_stats = pool.total_stats();
        assert_eq!(total_stats.allocations, 3);
        assert_eq!(total_stats.reuses, 0); // No reuses yet

        // Test another allocation for reuse
        let _buf4 = pool.allocate(4 * 1024 * 1024);
        let total_stats_after = pool.total_stats();
        assert_eq!(total_stats_after.allocations, 4);
        assert_eq!(total_stats_after.reuses, 1); // One reuse

        assert!(total_stats_after.reuse_rate() > 0.0);
        assert!(total_stats_after.reuse_rate() <= 1.0);
    }

    #[test]
    fn test_pool_warm_up() {
        let pool = NgdpMemoryPool::new();

        // Check initial state
        let initial_stats = pool.total_stats();
        assert_eq!(initial_stats.allocations, 0);

        // Warm up the pool
        pool.warm_up();

        // Pool should have buffers ready
        for size_class in NgdpSizeClass::all() {
            let stats = pool.size_class_stats(*size_class);
            assert!(stats.pool_size > 0);
        }
    }

    #[test]
    fn test_pool_clear() {
        let pool = NgdpMemoryPool::new();

        // Warm up and then clear
        pool.warm_up();
        let stats_before = pool.total_stats();
        assert!(
            stats_before
                .size_class_stats
                .iter()
                .any(|s| s.pool_size > 0)
        );

        pool.clear();

        // All pools should be empty
        for size_class in NgdpSizeClass::all() {
            let stats = pool.size_class_stats(*size_class);
            assert_eq!(stats.pool_size, 0);
        }
    }

    #[test]
    fn test_thread_local_pool() {
        // Test basic allocation
        let buffer = allocate_thread_local(1024);
        assert!(buffer.capacity() >= 1024);

        // Return to pool
        let capacity = buffer.capacity();
        deallocate_thread_local(buffer);

        // Should reuse the same buffer
        let reused_buffer = allocate_thread_local(1024);
        assert_eq!(reused_buffer.capacity(), capacity);
    }

    #[test]
    fn test_thread_local_pool_clear() {
        // Allocate and return some buffers
        let buf1 = allocate_thread_local(1024);
        let buf2 = allocate_thread_local(128 * 1024);

        deallocate_thread_local(buf1);
        deallocate_thread_local(buf2);

        // Clear the pool
        clear_thread_local_pool();

        // Next allocation should create new buffer
        let _buf3 = allocate_thread_local(1024);
        // We can't easily test that it's new, but at least it should work
    }

    // ... rest of tests remain the same
}
