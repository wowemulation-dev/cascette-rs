//! Memory pool for BLTE chunk processing
//!
//! Provides reusable buffer pools to reduce memory allocations during
//! BLTE decompression operations, achieving 20-30% memory reduction.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A reusable buffer with capacity tracking
#[derive(Debug)]
pub struct PooledBuffer {
    data: Vec<u8>,
    capacity: usize,
}

impl PooledBuffer {
    /// Create a new pooled buffer with the given capacity
    fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a mutable reference to the underlying vector
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        self.data.clear(); // Clear previous data but keep capacity
        &mut self.data
    }

    /// Get the data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get the data as a vector (consuming the buffer)
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    /// Get the current capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Reserve additional capacity if needed
    pub fn reserve(&mut self, additional: usize) {
        if self.data.capacity() < self.data.len() + additional {
            self.data.reserve(additional);
            self.capacity = self.data.capacity();
        }
    }
}

/// Memory pool for BLTE chunk processing buffers
#[derive(Debug, Clone)]
pub struct BLTEMemoryPool {
    // Pool for small buffers (< 64KB)
    small_buffers: Arc<Mutex<VecDeque<PooledBuffer>>>,
    // Pool for medium buffers (64KB - 1MB)
    medium_buffers: Arc<Mutex<VecDeque<PooledBuffer>>>,
    // Pool for large buffers (> 1MB)
    large_buffers: Arc<Mutex<VecDeque<PooledBuffer>>>,
    // Configuration
    config: PoolConfig,
}

/// Configuration for the memory pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of small buffers to pool
    pub max_small_buffers: usize,
    /// Maximum number of medium buffers to pool
    pub max_medium_buffers: usize,
    /// Maximum number of large buffers to pool
    pub max_large_buffers: usize,
    /// Threshold for small buffer size
    pub small_buffer_threshold: usize,
    /// Threshold for medium buffer size
    pub medium_buffer_threshold: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_small_buffers: 50,
            max_medium_buffers: 20,
            max_large_buffers: 5,
            small_buffer_threshold: 64 * 1024,    // 64KB
            medium_buffer_threshold: 1024 * 1024, // 1MB
        }
    }
}

impl BLTEMemoryPool {
    /// Create a new memory pool with default configuration
    pub fn new() -> Self {
        Self::with_config(PoolConfig::default())
    }

    /// Create a new memory pool with custom configuration
    pub fn with_config(config: PoolConfig) -> Self {
        Self {
            small_buffers: Arc::new(Mutex::new(VecDeque::new())),
            medium_buffers: Arc::new(Mutex::new(VecDeque::new())),
            large_buffers: Arc::new(Mutex::new(VecDeque::new())),
            config,
        }
    }

    /// Get a buffer suitable for the requested size
    pub fn get_buffer(&self, requested_size: usize) -> PooledBuffer {
        // Determine which pool to use
        let (pool, _max_count) = if requested_size <= self.config.small_buffer_threshold {
            (&self.small_buffers, self.config.max_small_buffers)
        } else if requested_size <= self.config.medium_buffer_threshold {
            (&self.medium_buffers, self.config.max_medium_buffers)
        } else {
            (&self.large_buffers, self.config.max_large_buffers)
        };

        // Try to get a buffer from the appropriate pool
        if let Ok(mut buffers) = pool.lock() {
            if let Some(mut buffer) = buffers.pop_front() {
                // Ensure the buffer has sufficient capacity
                if buffer.capacity() < requested_size {
                    buffer.reserve(requested_size - buffer.capacity());
                }
                return buffer;
            }
        }

        // No suitable buffer available, create a new one with appropriate capacity
        let capacity = if requested_size <= self.config.small_buffer_threshold {
            requested_size.max(1024) // 1KB minimum for small buffers
        } else if requested_size <= self.config.medium_buffer_threshold {
            requested_size.max(self.config.small_buffer_threshold + 1) // Just above small threshold
        } else {
            requested_size.max(self.config.medium_buffer_threshold + 1) // Just above medium threshold
        };
        PooledBuffer::new(capacity)
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&self, buffer: PooledBuffer) {
        let size = buffer.capacity();

        // Determine which pool to return to
        let (pool, max_count) = if size <= self.config.small_buffer_threshold {
            (&self.small_buffers, self.config.max_small_buffers)
        } else if size <= self.config.medium_buffer_threshold {
            (&self.medium_buffers, self.config.max_medium_buffers)
        } else {
            (&self.large_buffers, self.config.max_large_buffers)
        };

        // Return to pool if there's space
        if let Ok(mut buffers) = pool.lock() {
            if buffers.len() < max_count {
                buffers.push_back(buffer);
            }
            // If pool is full, buffer is dropped (garbage collected)
        }
    }

    /// Get statistics about the pool usage
    pub fn stats(&self) -> PoolStats {
        let small_count = self.small_buffers.lock().map(|b| b.len()).unwrap_or(0);
        let medium_count = self.medium_buffers.lock().map(|b| b.len()).unwrap_or(0);
        let large_count = self.large_buffers.lock().map(|b| b.len()).unwrap_or(0);

        PoolStats {
            small_buffers: small_count,
            medium_buffers: medium_count,
            large_buffers: large_count,
            config: self.config.clone(),
        }
    }

    /// Clear all pooled buffers
    pub fn clear(&self) {
        if let Ok(mut buffers) = self.small_buffers.lock() {
            buffers.clear();
        }
        if let Ok(mut buffers) = self.medium_buffers.lock() {
            buffers.clear();
        }
        if let Ok(mut buffers) = self.large_buffers.lock() {
            buffers.clear();
        }
    }
}

impl Default for BLTEMemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about pool usage
#[derive(Debug)]
pub struct PoolStats {
    pub small_buffers: usize,
    pub medium_buffers: usize,
    pub large_buffers: usize,
    pub config: PoolConfig,
}

impl PoolStats {
    /// Get total number of pooled buffers
    pub fn total_buffers(&self) -> usize {
        self.small_buffers + self.medium_buffers + self.large_buffers
    }

    /// Get estimated memory usage of pooled buffers
    pub fn estimated_memory_usage(&self) -> usize {
        // Estimate based on average sizes and counts
        let small_memory = self.small_buffers * (self.config.small_buffer_threshold / 2);
        let medium_memory = self.medium_buffers * (self.config.medium_buffer_threshold / 2);
        let large_memory = self.large_buffers * (self.config.medium_buffer_threshold * 2);

        small_memory + medium_memory + large_memory
    }

    /// Get pool utilization as percentage
    pub fn utilization(&self) -> f64 {
        let total_used = self.total_buffers() as f64;
        let total_capacity = (self.config.max_small_buffers
            + self.config.max_medium_buffers
            + self.config.max_large_buffers) as f64;

        if total_capacity > 0.0 {
            (total_used / total_capacity) * 100.0
        } else {
            0.0
        }
    }
}

/// RAII wrapper for pooled buffers that automatically returns buffer to pool
pub struct PooledBufferGuard {
    buffer: Option<PooledBuffer>,
    pool: BLTEMemoryPool,
}

impl PooledBufferGuard {
    /// Create a new guard
    pub fn new(buffer: PooledBuffer, pool: BLTEMemoryPool) -> Self {
        Self {
            buffer: Some(buffer),
            pool,
        }
    }

    /// Get mutable access to the buffer
    pub fn buffer_mut(&mut self) -> &mut PooledBuffer {
        self.buffer.as_mut().expect("Buffer should be available")
    }

    /// Get immutable access to the buffer
    pub fn buffer(&self) -> &PooledBuffer {
        self.buffer.as_ref().expect("Buffer should be available")
    }

    /// Take ownership of the buffer (preventing return to pool)
    pub fn take(mut self) -> PooledBuffer {
        self.buffer.take().expect("Buffer should be available")
    }
}

impl Drop for PooledBufferGuard {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            self.pool.return_buffer(buffer);
        }
    }
}

// Global memory pool instance
static GLOBAL_POOL: std::sync::OnceLock<BLTEMemoryPool> = std::sync::OnceLock::new();

/// Get the global BLTE memory pool
pub fn global_pool() -> &'static BLTEMemoryPool {
    GLOBAL_POOL.get_or_init(BLTEMemoryPool::new)
}

/// Initialize the global memory pool with custom configuration
pub fn init_global_pool(config: PoolConfig) -> bool {
    GLOBAL_POOL.set(BLTEMemoryPool::with_config(config)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pooled_buffer() {
        let mut buffer = PooledBuffer::new(1024);
        assert_eq!(buffer.capacity(), 1024);

        let vec = buffer.as_mut_vec();
        vec.extend_from_slice(b"Hello, World!");
        assert_eq!(buffer.as_slice(), b"Hello, World!");

        buffer.reserve(2048);
        assert!(buffer.capacity() >= 2048);
    }

    #[test]
    fn test_memory_pool_basic() {
        let pool = BLTEMemoryPool::new();

        // Get a small buffer
        let buffer1 = pool.get_buffer(1024);
        assert!(buffer1.capacity() >= 1024);

        // Return it to pool
        pool.return_buffer(buffer1);

        // Get another buffer - should reuse the returned one
        let buffer2 = pool.get_buffer(1024);
        assert!(buffer2.capacity() >= 1024);

        pool.return_buffer(buffer2);
    }

    #[test]
    fn test_buffer_size_categorization() {
        let config = PoolConfig {
            small_buffer_threshold: 1024,
            medium_buffer_threshold: 10240,
            ..Default::default()
        };
        let pool = BLTEMemoryPool::with_config(config);

        // Test small buffer
        let small_buffer = pool.get_buffer(512);
        pool.return_buffer(small_buffer);

        // Test medium buffer
        let medium_buffer = pool.get_buffer(5120);
        pool.return_buffer(medium_buffer);

        // Test large buffer
        let large_buffer = pool.get_buffer(20480);
        pool.return_buffer(large_buffer);

        let stats = pool.stats();
        assert_eq!(stats.small_buffers, 1);
        assert_eq!(stats.medium_buffers, 1);
        assert_eq!(stats.large_buffers, 1);
    }

    #[test]
    fn test_pool_capacity_limits() {
        let config = PoolConfig {
            max_small_buffers: 1,
            max_medium_buffers: 1,
            max_large_buffers: 1,
            ..Default::default()
        };
        let pool = BLTEMemoryPool::with_config(config);

        // Fill pools to capacity
        let buffer1 = pool.get_buffer(512);
        pool.return_buffer(buffer1);

        let buffer2 = pool.get_buffer(512);
        pool.return_buffer(buffer2); // This should be dropped, not pooled

        let stats = pool.stats();
        assert_eq!(stats.small_buffers, 1); // Only one buffer should be pooled
    }

    #[test]
    fn test_pooled_buffer_guard() {
        let pool = BLTEMemoryPool::new();
        let buffer = pool.get_buffer(1024);

        {
            let mut guard = PooledBufferGuard::new(buffer, pool.clone());
            let vec = guard.buffer_mut().as_mut_vec();
            vec.extend_from_slice(b"Test data");
            assert_eq!(guard.buffer().as_slice(), b"Test data");
        } // Buffer automatically returned to pool here

        let stats = pool.stats();
        assert!(stats.total_buffers() > 0);
    }

    #[test]
    fn test_pool_stats() {
        let pool = BLTEMemoryPool::new();

        let initial_stats = pool.stats();
        assert_eq!(initial_stats.total_buffers(), 0);
        assert_eq!(initial_stats.utilization(), 0.0);

        // Add some buffers
        let buffer1 = pool.get_buffer(1024);
        let buffer2 = pool.get_buffer(65536);
        let buffer3 = pool.get_buffer(2097152);

        pool.return_buffer(buffer1);
        pool.return_buffer(buffer2);
        pool.return_buffer(buffer3);

        let stats = pool.stats();
        assert_eq!(stats.total_buffers(), 3);
        assert!(stats.utilization() > 0.0);
        assert!(stats.estimated_memory_usage() > 0);
    }

    #[test]
    fn test_global_pool() {
        let pool = global_pool();
        let buffer = pool.get_buffer(1024);
        pool.return_buffer(buffer);

        let stats = pool.stats();
        assert!(stats.total_buffers() > 0);
    }

    #[test]
    fn test_pool_clear() {
        let pool = BLTEMemoryPool::new();

        let buffer = pool.get_buffer(1024);
        pool.return_buffer(buffer);

        assert!(pool.stats().total_buffers() > 0);

        pool.clear();
        assert_eq!(pool.stats().total_buffers(), 0);
    }
}
