//! Zero-copy cache operations optimized for NGDP large file handling
//!
//! This module provides zero-copy data structures and operations for:
//! - Efficient handling of large NGDP files
//! - Memory-mapped data sharing between cache layers
//! - Reference-counted data with atomic cleanup
//! - Streaming operations on cached data
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss
#![allow(unused_imports)] // Some imports used conditionally or in future features
#![allow(dead_code)] // Reference counting fields maintained for future use
#![allow(missing_docs)] // Internal module with implementation in progress

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{
    io::{self, Read, Write},
    ops::{Deref, Range},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Zero-copy cache entry with reference counting and memory sharing
#[derive(Debug, Clone)]
pub struct ZeroCopyEntry {
    /// Shared data buffer (reference counted)
    data: Bytes,
    /// Total reference count across all cache layers
    ref_count: Arc<AtomicUsize>,
    /// Original size for accounting
    original_size: usize,
    /// Creation timestamp for expiry
    created_at: std::time::Instant,
}

impl ZeroCopyEntry {
    /// Create new zero-copy entry
    pub fn new(data: Bytes) -> Self {
        let original_size = data.len();
        Self {
            data,
            ref_count: Arc::new(AtomicUsize::new(1)),
            original_size,
            created_at: std::time::Instant::now(),
        }
    }

    /// Create from BytesMut (zero-copy conversion)
    pub fn from_bytes_mut(data: BytesMut) -> Self {
        Self::new(data.freeze())
    }

    /// Get data as Bytes (zero-copy clone)
    pub fn data(&self) -> Bytes {
        self.data.clone()
    }

    /// Get data slice (zero allocation)
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get original size (before any potential slicing)
    pub fn original_size(&self) -> usize {
        self.original_size
    }

    /// Get current reference count
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Create a slice view of the data (zero-copy)
    pub fn slice(&self, range: Range<usize>) -> Option<ZeroCopySlice> {
        if range.end <= self.data.len() {
            Some(ZeroCopySlice {
                data: self.data.slice(range.clone()),
                parent_ref: Arc::clone(&self.ref_count),
                range,
            })
        } else {
            None
        }
    }

    /// Create streaming reader (zero-copy)
    pub fn reader(&self) -> ZeroCopyReader {
        ZeroCopyReader::new(self.data.clone())
    }

    /// Check if data can be modified (reference count == 1)
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.ref_count) == 1
    }

    /// Try to get mutable access (only if unique reference)
    pub fn try_mut(&mut self) -> Option<&mut BytesMut> {
        // This would require tracking the original BytesMut
        // For now, return None as Bytes is immutable
        None
    }

    /// Create a new entry by appending data (zero-copy when possible)
    pub fn append(&self, additional: &[u8]) -> ZeroCopyEntry {
        let mut combined = BytesMut::with_capacity(self.data.len() + additional.len());
        combined.extend_from_slice(&self.data);
        combined.extend_from_slice(additional);

        ZeroCopyEntry::from_bytes_mut(combined)
    }

    /// Check if entry has expired
    pub fn is_expired(&self, ttl: std::time::Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

impl Drop for ZeroCopyEntry {
    fn drop(&mut self) {
        self.ref_count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Deref for ZeroCopyEntry {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl AsRef<[u8]> for ZeroCopyEntry {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

/// Zero-copy slice of cached data
#[derive(Debug, Clone)]
pub struct ZeroCopySlice {
    /// Sliced data
    data: Bytes,
    /// Reference to parent entry
    parent_ref: Arc<AtomicUsize>,
    /// Range within original data
    range: Range<usize>,
}

impl ZeroCopySlice {
    /// Get slice data
    pub fn data(&self) -> Bytes {
        self.data.clone()
    }

    /// Get slice as byte array
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get slice range
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Get size of slice
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

impl Deref for ZeroCopySlice {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl AsRef<[u8]> for ZeroCopySlice {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

/// Zero-copy streaming reader for cached data
#[derive(Debug)]
pub struct ZeroCopyReader {
    /// Data buffer
    data: Bytes,
    /// Current read position
    position: usize,
}

impl ZeroCopyReader {
    /// Create new reader
    pub fn new(data: Bytes) -> Self {
        Self { data, position: 0 }
    }

    /// Get remaining bytes
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    /// Check if at end of data
    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    /// Seek to position
    pub fn seek(&mut self, pos: usize) -> io::Result<()> {
        if pos <= self.data.len() {
            self.position = pos;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek position beyond data length",
            ))
        }
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Read exact number of bytes
    pub fn read_exact_bytes(&mut self, count: usize) -> io::Result<Bytes> {
        if self.remaining() < count {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Not enough bytes remaining",
            ));
        }

        let start = self.position;
        self.position += count;
        Ok(self.data.slice(start..self.position))
    }

    /// Read all remaining bytes
    pub fn read_remaining(&mut self) -> Bytes {
        let remaining = self.data.slice(self.position..);
        self.position = self.data.len();
        remaining
    }

    /// Peek at next bytes without advancing position
    pub fn peek(&self, count: usize) -> Option<Bytes> {
        let end = self.position.checked_add(count)?;
        if end <= self.data.len() {
            Some(self.data.slice(self.position..end))
        } else {
            None
        }
    }
}

impl Read for ZeroCopyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = self.remaining().min(buf.len());
        if available == 0 {
            return Ok(0);
        }

        buf[..available].copy_from_slice(&self.data[self.position..self.position + available]);
        self.position += available;
        Ok(available)
    }
}

impl AsyncRead for ZeroCopyReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let available = self.remaining().min(buf.remaining());
        if available == 0 {
            return Poll::Ready(Ok(()));
        }

        let data_slice = &self.data[self.position..self.position + available];
        buf.put_slice(data_slice);
        self.position += available;

        Poll::Ready(Ok(()))
    }
}

/// Zero-copy cache optimized for large NGDP files
pub struct ZeroCopyCache {
    /// Cache entries by hash
    entries: std::collections::HashMap<u64, ZeroCopyEntry>,
    /// Total memory usage
    total_memory: usize,
    /// Maximum memory limit
    max_memory: usize,
    /// Cache statistics
    stats: ZeroCopyCacheStats,
}

#[derive(Debug, Default)]
pub struct ZeroCopyCacheStats {
    /// Total gets
    pub gets: u64,
    /// Cache hits
    pub hits: u64,
    /// Total puts
    pub puts: u64,
    /// Zero-copy operations
    pub zero_copy_ops: u64,
    /// Memory saved by zero-copy
    pub memory_saved_bytes: u64,
    /// Active references
    pub active_refs: u64,
}

impl ZeroCopyCache {
    /// Create new zero-copy cache
    pub fn new(max_memory: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            total_memory: 0,
            max_memory,
            stats: ZeroCopyCacheStats::default(),
        }
    }

    /// Put entry in cache
    pub fn put(&mut self, key: u64, data: Bytes) {
        let entry = ZeroCopyEntry::new(data);
        let size = entry.size();

        // Check if we need to evict
        while self.total_memory + size > self.max_memory && !self.entries.is_empty() {
            self.evict_lru();
        }

        if let Some(old_entry) = self.entries.insert(key, entry) {
            self.total_memory -= old_entry.size();
        }

        self.total_memory += size;
        self.stats.puts += 1;
    }

    /// Get entry from cache (zero-copy)
    pub fn get(&mut self, key: u64) -> Option<ZeroCopyEntry> {
        self.stats.gets += 1;

        if let Some(entry) = self.entries.get(&key) {
            self.stats.hits += 1;
            self.stats.zero_copy_ops += 1;
            self.stats.memory_saved_bytes += entry.size() as u64;
            Some(entry.clone()) // Zero-copy clone
        } else {
            None
        }
    }

    /// Get slice from cached entry (zero-copy)
    pub fn get_slice(&mut self, key: u64, range: Range<usize>) -> Option<ZeroCopySlice> {
        self.stats.gets += 1;

        if let Some(entry) = self.entries.get(&key)
            && let Some(slice) = entry.slice(range)
        {
            self.stats.hits += 1;
            self.stats.zero_copy_ops += 1;
            return Some(slice);
        }

        None
    }

    /// Get streaming reader for cached entry (zero-copy)
    pub fn get_reader(&mut self, key: u64) -> Option<ZeroCopyReader> {
        self.stats.gets += 1;

        if let Some(entry) = self.entries.get(&key) {
            self.stats.hits += 1;
            self.stats.zero_copy_ops += 1;
            Some(entry.reader())
        } else {
            None
        }
    }

    /// Remove entry from cache
    pub fn remove(&mut self, key: u64) -> bool {
        if let Some(entry) = self.entries.remove(&key) {
            self.total_memory -= entry.size();
            true
        } else {
            false
        }
    }

    /// Check if key exists in cache
    pub fn contains(&self, key: u64) -> bool {
        self.entries.contains_key(&key)
    }

    /// Get cache size in entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total memory usage
    pub fn memory_usage(&self) -> usize {
        self.total_memory
    }

    /// Get cache statistics
    pub fn stats(&self) -> &ZeroCopyCacheStats {
        &self.stats
    }

    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.stats.gets == 0 {
            0.0
        } else {
            self.stats.hits as f64 / self.stats.gets as f64
        }
    }

    /// Calculate memory efficiency (how much memory saved by zero-copy)
    pub fn memory_efficiency(&self) -> f64 {
        if self.total_memory == 0 {
            0.0
        } else {
            self.stats.memory_saved_bytes as f64 / self.total_memory as f64
        }
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_memory = 0;
    }

    /// Evict least recently used entry (simplified)
    fn evict_lru(&mut self) {
        // Find oldest entry by creation time (simplified LRU)
        if let Some((&oldest_key, _)) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.created_at)
        {
            self.remove(oldest_key);
        }
    }

    /// Compact cache by removing expired entries
    pub fn compact(&mut self, ttl: std::time::Duration) {
        let expired_keys: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(ttl))
            .map(|(&key, _)| key)
            .collect();

        for key in expired_keys {
            self.remove(key);
        }
    }

    /// Get entries with high reference counts (actively shared)
    pub fn get_highly_referenced_entries(&self, min_refs: usize) -> Vec<(u64, usize)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.ref_count() >= min_refs)
            .map(|(&key, entry)| (key, entry.ref_count()))
            .collect()
    }
}

/// Zero-copy buffer pool for efficient reuse
pub struct ZeroCopyBufferPool {
    /// Available buffers by size class
    pools: std::collections::HashMap<usize, Vec<BytesMut>>,
    /// Pool statistics
    stats: BufferPoolStats,
}

#[derive(Debug, Default)]
pub struct BufferPoolStats {
    /// Total allocations
    pub allocations: u64,
    /// Pool hits (reused buffers)
    pub hits: u64,
    /// Pool misses (new allocations)
    pub misses: u64,
}

impl ZeroCopyBufferPool {
    /// Create new buffer pool
    pub fn new() -> Self {
        Self {
            pools: std::collections::HashMap::new(),
            stats: BufferPoolStats::default(),
        }
    }

    /// Get buffer of specified size (reuse if available)
    pub fn get_buffer(&mut self, size: usize) -> BytesMut {
        self.stats.allocations += 1;

        // Round size to nearest power of 2 for better pooling
        let pool_size = size.next_power_of_two();

        if let Some(pool) = self.pools.get_mut(&pool_size)
            && let Some(mut buffer) = pool.pop()
        {
            buffer.clear();
            if buffer.capacity() < size {
                buffer.reserve(size - buffer.capacity());
            }
            self.stats.hits += 1;
            return buffer;
        }

        // Pool miss - allocate new buffer
        self.stats.misses += 1;
        BytesMut::with_capacity(size)
    }

    /// Return buffer to pool for reuse
    pub fn return_buffer(&mut self, buffer: BytesMut) {
        const MAX_POOL_SIZE: usize = 32;

        let capacity = buffer.capacity();
        let pool_size = capacity.next_power_of_two();

        // Only keep buffers in reasonable size range
        if (1024..=64 * 1024 * 1024).contains(&capacity) {
            self.pools.entry(pool_size).or_default().push(buffer);
        }

        // Limit pool size to prevent excessive memory use
        if let Some(pool) = self.pools.get_mut(&pool_size)
            && pool.len() > MAX_POOL_SIZE
        {
            pool.truncate(MAX_POOL_SIZE);
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> &BufferPoolStats {
        &self.stats
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.stats.allocations == 0 {
            0.0
        } else {
            self.stats.hits as f64 / self.stats.allocations as f64
        }
    }

    /// Clear all pools
    pub fn clear(&mut self) {
        self.pools.clear();
    }
}

impl Default for ZeroCopyBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn test_zero_copy_entry_basic() {
        let data = Bytes::from("Hello, NGDP!");
        let entry = ZeroCopyEntry::new(data.clone());

        assert_eq!(entry.size(), data.len());
        assert_eq!(entry.as_slice(), data.as_ref());
        assert_eq!(entry.ref_count(), 1);

        // Test cloning (should not copy data)
        let entry2 = entry.clone();
        assert_eq!(entry.data(), entry2.data());
    }

    #[test]
    fn test_zero_copy_slice() {
        let data = Bytes::from("Hello, NGDP world!");
        let entry = ZeroCopyEntry::new(data);

        let slice = entry.slice(7..11).expect("Test operation should succeed");
        assert_eq!(slice.as_slice(), b"NGDP");
        assert_eq!(slice.range(), 7..11);
    }

    #[test]
    fn test_zero_copy_reader_sync() {
        use std::io::Read;

        let data = Bytes::from("Hello, NGDP!");
        let entry = ZeroCopyEntry::new(data);
        let mut reader = entry.reader();

        let mut buf = [0u8; 5];
        let n = Read::read(&mut reader, &mut buf).expect("Test operation should succeed");
        assert_eq!(n, 5);
        assert_eq!(&buf, b"Hello");

        assert_eq!(reader.remaining(), 7); // "Hello, NGDP!" = 12 chars, 12 - 5 = 7
        assert_eq!(reader.position(), 5);

        let remaining = reader.read_remaining();
        assert_eq!(remaining.as_ref(), b", NGDP!");
    }

    #[tokio::test]
    async fn test_zero_copy_reader_async() {
        use tokio::io::AsyncReadExt;

        let data = Bytes::from("Hello, NGDP!");
        let entry = ZeroCopyEntry::new(data);
        let mut reader = entry.reader();

        let mut buf = [0u8; 12];
        AsyncReadExt::read_exact(&mut reader, &mut buf)
            .await
            .expect("Test operation should succeed");
        assert_eq!(&buf, b"Hello, NGDP!");
    }

    #[test]
    fn test_zero_copy_cache() {
        let mut cache = ZeroCopyCache::new(1024 * 1024); // 1MB limit

        let data1 = Bytes::from("Test data 1");
        let data2 = Bytes::from("Test data 2");

        cache.put(1, data1.clone());
        cache.put(2, data2);

        assert_eq!(cache.len(), 2);

        let entry1 = cache.get(1).expect("Test operation should succeed");
        assert_eq!(entry1.data(), data1);

        let slice = cache
            .get_slice(2, 0..4)
            .expect("Test operation should succeed");
        assert_eq!(slice.as_slice(), b"Test");

        assert_eq!(cache.hit_rate(), 1.0); // 2 hits out of 2 gets
    }

    #[test]
    fn test_buffer_pool() {
        let mut pool = ZeroCopyBufferPool::new();

        // Get buffer
        let buffer1 = pool.get_buffer(1024);
        assert!(buffer1.capacity() >= 1024);

        // Return buffer
        pool.return_buffer(buffer1);

        // Get buffer again (should reuse)
        let _buffer2 = pool.get_buffer(1024);

        assert!(pool.hit_rate() > 0.0);
    }

    #[test]
    fn test_zero_copy_memory_efficiency() {
        let mut cache = ZeroCopyCache::new(10 * 1024 * 1024); // 10MB

        let large_data = Bytes::from(vec![0u8; 1024 * 1024]); // 1MB
        cache.put(1, large_data);

        // Multiple gets should show memory savings
        for _ in 0..10 {
            cache.get(1);
        }

        assert!(cache.memory_efficiency() > 0.0);
        assert_eq!(cache.stats().zero_copy_ops, 10);
    }

    #[test]
    fn test_entry_expiration() {
        let data = Bytes::from("test");
        let mut entry = ZeroCopyEntry::new(data);

        // Entry should not be expired immediately
        assert!(!entry.is_expired(std::time::Duration::from_secs(60)));

        // Simulate old entry by modifying created_at
        entry.created_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(120))
            .expect("Test operation should succeed");
        assert!(entry.is_expired(std::time::Duration::from_secs(60)));
    }
}
