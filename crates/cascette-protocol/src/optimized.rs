//! Performance optimizations for NGDP protocol operations
//!
//! This module provides zero-copy and memory-optimized implementations
//! for common protocol operations.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

thread_local! {
    /// Thread-local cache key buffer to reduce allocations
    static CACHE_KEY_BUFFER: std::cell::RefCell<String> = std::cell::RefCell::new(String::with_capacity(128));
}

/// Optimized cache key generation with minimal allocations
pub fn format_cache_key(prefix: &str, endpoint: &str) -> String {
    CACHE_KEY_BUFFER.with(|buffer| {
        let mut buf = buffer.borrow_mut();
        buf.clear();
        buf.reserve(prefix.len() + endpoint.len() + 1);
        buf.push_str(prefix);
        buf.push(':');
        buf.push_str(endpoint);
        buf.clone()
    })
}

/// Zero-copy string interning for frequently used protocol strings
pub struct StringInterner {
    strings: HashMap<String, Arc<str>>,
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: HashMap::with_capacity(256), // Pre-allocate for common protocol strings
        }
    }

    /// Intern a string, returning an `Arc<str>` for zero-copy sharing
    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(interned) = self.strings.get(s) {
            Arc::clone(interned)
        } else {
            let arc_str: Arc<str> = Arc::from(s);
            self.strings.insert(s.to_owned(), Arc::clone(&arc_str));
            arc_str
        }
    }
}

/// Global string interner for protocol constants
static GLOBAL_INTERNER: OnceLock<std::sync::Mutex<StringInterner>> = OnceLock::new();

/// Intern common protocol strings for zero-copy operations
#[allow(clippy::expect_used)]
// expect_used: Mutex poisoning indicates a panic in another thread while holding
// the lock. This is a fatal, unrecoverable state for the application.
pub fn intern_string(s: &str) -> Arc<str> {
    let interner = GLOBAL_INTERNER.get_or_init(|| std::sync::Mutex::new(StringInterner::new()));

    interner
        .lock()
        .expect("string interner mutex poisoned")
        .intern(s)
}

/// Pre-computed hashes for common protocol endpoints
/// Reduces string hashing overhead in hot paths
pub struct EndpointHashes {
    hashes: HashMap<&'static str, u64>,
}

impl Default for EndpointHashes {
    fn default() -> Self {
        Self::new()
    }
}

impl EndpointHashes {
    pub fn new() -> Self {
        let mut hashes = HashMap::with_capacity(32);

        // Pre-compute hashes for common NGDP endpoints
        let common_endpoints = [
            "v1/products/wow/versions",
            "v1/products/wow/bgdl",
            "v1/products/wow/cdns",
            "v1/products/wowt/versions",
            "v1/products/wowt/bgdl",
            "v1/products/wowt/cdns",
            "v1/products/wowdev/versions",
            "v1/products/wowdev/bgdl",
            "v1/products/wowdev/cdns",
        ];

        for endpoint in &common_endpoints {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hash_fn = DefaultHasher::new();
            endpoint.hash(&mut hash_fn);
            hashes.insert(*endpoint, hash_fn.finish());
        }

        Self { hashes }
    }

    /// Get pre-computed hash for endpoint, or compute it if not cached
    pub fn get_hash(&self, endpoint: &str) -> u64 {
        if let Some(&hash) = self.hashes.get(endpoint) {
            hash
        } else {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hash_fn = DefaultHasher::new();
            endpoint.hash(&mut hash_fn);
            hash_fn.finish()
        }
    }
}

/// Global endpoint hash cache
static ENDPOINT_HASHES: OnceLock<EndpointHashes> = OnceLock::new();

/// Get optimized hash for endpoint
pub fn endpoint_hash(endpoint: &str) -> u64 {
    ENDPOINT_HASHES
        .get_or_init(EndpointHashes::new)
        .get_hash(endpoint)
}

/// Memory pool for frequently allocated byte buffers
/// Reduces allocation pressure for protocol responses
pub struct ByteBufferPool {
    small: Vec<Vec<u8>>,  // < 1KB
    medium: Vec<Vec<u8>>, // 1KB - 64KB
    large: Vec<Vec<u8>>,  // > 64KB
}

impl Default for ByteBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteBufferPool {
    pub fn new() -> Self {
        Self {
            small: Vec::with_capacity(32),
            medium: Vec::with_capacity(16),
            large: Vec::with_capacity(4),
        }
    }

    /// Get a buffer from the pool or allocate a new one
    pub fn get_buffer(&mut self, size: usize) -> Vec<u8> {
        let pool = if size < 1024 {
            &mut self.small
        } else if size < 65536 {
            &mut self.medium
        } else {
            &mut self.large
        };

        if let Some(mut buf) = pool.pop() {
            buf.clear();
            buf.reserve(size.saturating_sub(buf.capacity()));
            buf
        } else {
            Vec::with_capacity(size)
        }
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&mut self, mut buf: Vec<u8>) {
        // Only keep reasonably sized buffers to prevent memory bloat
        if buf.capacity() > 1024 * 1024 {
            return; // Drop buffers > 1MB
        }

        let pool = if buf.capacity() < 1024 {
            &mut self.small
        } else if buf.capacity() < 65536 {
            &mut self.medium
        } else {
            &mut self.large
        };

        // Limit pool size to prevent unbounded growth
        if pool.len() < pool.capacity() {
            buf.clear();
            pool.push(buf);
        }
    }
}

thread_local! {
    /// Thread-local buffer pool
    static BUFFER_POOL: std::cell::RefCell<ByteBufferPool> =
        std::cell::RefCell::new(ByteBufferPool::new());
}

/// Get an optimized buffer from the thread-local pool
pub fn get_buffer(size: usize) -> Vec<u8> {
    BUFFER_POOL.with(|pool| pool.borrow_mut().get_buffer(size))
}

/// Return a buffer to the thread-local pool
pub fn return_buffer(buf: Vec<u8>) {
    BUFFER_POOL.with(|pool| pool.borrow_mut().return_buffer(buf));
}

/// RAII buffer wrapper that automatically returns to pool
pub struct PooledBuffer {
    buffer: Option<Vec<u8>>,
}

impl PooledBuffer {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: Some(get_buffer(size)),
        }
    }

    /// Get mutable reference to the buffer
    pub fn as_mut_slice(&mut self) -> &mut Vec<u8> {
        #[allow(clippy::expect_used)]
        // expect_used: PooledBuffer is only created with Some(buffer)
        // and into_vec is the only method that takes the buffer.
        self.buffer
            .as_mut()
            .expect("PooledBuffer should contain a buffer")
    }

    /// Get immutable reference to the buffer
    pub fn as_slice(&self) -> &Vec<u8> {
        #[allow(clippy::expect_used)]
        // expect_used: PooledBuffer is only created with Some(buffer)
        // and into_vec is the only method that takes the buffer.
        self.buffer
            .as_ref()
            .expect("PooledBuffer should contain a buffer")
    }

    /// Convert to owned Vec, consuming the wrapper
    pub fn into_vec(mut self) -> Vec<u8> {
        #[allow(clippy::expect_used)]
        // expect_used: PooledBuffer is only created with Some(buffer)
        // and into_vec is the only method that takes the buffer.
        self.buffer
            .take()
            .expect("PooledBuffer should contain a buffer")
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buf) = self.buffer.take() {
            return_buffer(buf);
        }
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
