//! LocalStorage cache implementation for WASM platforms
//!
//! This module provides a cache backend using browser LocalStorage for WASM targets.
//! LocalStorage has limitations compared to native caching:
//!
//! - **Storage limit**: ~5-10MB depending on browser (sufficient for protocol responses)
//! - **Synchronous API**: Operations are synchronous but wrapped in async
//! - **String-only storage**: Values are base64-encoded
//! - **No background cleanup**: Expiration is checked lazily on reads
//!
//! For protocol responses (BPSV data, configs), LocalStorage provides adequate
//! persistence. For larger data, consider IndexedDB in future implementations.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(missing_docs)]

use crate::error::{CacheError, CacheResult};
use crate::key::CacheKey;
use crate::stats::CacheStats;
use crate::traits::AsyncCache;
use async_trait::async_trait;
use base64::Engine;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use web_sys::Storage;

/// Key prefix for all cascette cache entries in LocalStorage
const KEY_PREFIX: &str = "cascette_cache:";

/// Entry metadata stored alongside values in LocalStorage
#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    /// Base64-encoded value
    value_b64: String,
    /// Unix timestamp in milliseconds when entry was created
    created_at_ms: u64,
    /// Unix timestamp in milliseconds when entry expires (0 for no expiration)
    expires_at_ms: u64,
    /// Size of the original value in bytes
    size_bytes: usize,
}

impl StoredEntry {
    fn new(value: &[u8], ttl: Option<Duration>) -> Self {
        let now_ms = current_time_ms();
        let expires_at_ms = ttl.map_or(0, |t| now_ms + t.as_millis() as u64);

        Self {
            value_b64: base64_encode(value),
            created_at_ms: now_ms,
            expires_at_ms,
            size_bytes: value.len(),
        }
    }

    fn is_expired(&self) -> bool {
        if self.expires_at_ms == 0 {
            return false;
        }
        current_time_ms() >= self.expires_at_ms
    }

    fn value(&self) -> Option<Vec<u8>> {
        base64_decode(&self.value_b64)
    }
}

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    // Use js_sys::Date for WASM
    js_sys::Date::now() as u64
}

/// Base64 encode bytes
fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Base64 decode string
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

/// Get localStorage from the window
fn get_local_storage() -> CacheResult<Storage> {
    let window = web_sys::window().ok_or_else(|| {
        CacheError::Config("No window object available (not in browser?)".to_string())
    })?;

    window
        .local_storage()
        .map_err(|_| CacheError::Config("Failed to access localStorage".to_string()))?
        .ok_or_else(|| CacheError::Config("localStorage not available".to_string()))
}

/// Make a prefixed key for localStorage
fn make_storage_key(cache_key: &str) -> String {
    format!("{KEY_PREFIX}{cache_key}")
}

/// LocalStorage-backed cache for WASM platforms
///
/// Provides a simple key-value cache using browser LocalStorage.
/// Suitable for caching protocol responses and small configuration data.
///
/// # Example
///
/// ```ignore
/// use cascette_cache::local_storage_cache::LocalStorageCache;
/// use cascette_cache::key::RibbitKey;
///
/// let cache: LocalStorageCache<RibbitKey> = LocalStorageCache::new(
///     Duration::from_secs(300), // 5 minute default TTL
///     5 * 1024 * 1024,          // 5MB max size
/// )?;
///
/// let key = RibbitKey::new("versions", "us");
/// cache.put(key, Bytes::from("data")).await?;
/// ```
pub struct LocalStorageCache<K: CacheKey> {
    /// Default TTL for entries without explicit TTL
    default_ttl: Duration,
    /// Maximum total size in bytes (soft limit)
    max_size_bytes: usize,
    /// Cache statistics
    hits: AtomicU64,
    misses: AtomicU64,
    puts: AtomicU64,
    removes: AtomicU64,
    /// Phantom data for key type
    _marker: PhantomData<K>,
}

impl<K: CacheKey> LocalStorageCache<K> {
    /// Create a new LocalStorage cache
    ///
    /// # Arguments
    ///
    /// * `default_ttl` - Default time-to-live for entries without explicit TTL
    /// * `max_size_bytes` - Soft limit on total storage usage (browser enforces hard limit)
    pub fn new(default_ttl: Duration, max_size_bytes: usize) -> CacheResult<Self> {
        // Verify localStorage is available
        let _ = get_local_storage()?;

        Ok(Self {
            default_ttl,
            max_size_bytes,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            puts: AtomicU64::new(0),
            removes: AtomicU64::new(0),
            _marker: PhantomData,
        })
    }

    /// Create with default settings (5 minute TTL, 5MB limit)
    pub fn with_defaults() -> CacheResult<Self> {
        Self::new(Duration::from_secs(300), 5 * 1024 * 1024)
    }

    /// Get the internal storage key for a cache key
    fn storage_key(&self, key: &K) -> String {
        make_storage_key(key.as_cache_key())
    }

    /// Get an entry from storage, checking expiration
    fn get_entry(&self, key: &K) -> CacheResult<Option<StoredEntry>> {
        let storage = get_local_storage()?;
        let storage_key = self.storage_key(key);

        let value_str = storage.get_item(&storage_key).map_err(|_| {
            CacheError::Io(std::io::Error::other("Failed to read from localStorage"))
        })?;

        let Some(value_str) = value_str else {
            return Ok(None);
        };

        let entry: StoredEntry = serde_json::from_str(&value_str).map_err(|e| {
            CacheError::Serialization(format!("Failed to deserialize cache entry: {e}"))
        })?;

        // Check expiration
        if entry.is_expired() {
            // Remove expired entry
            let _ = storage.remove_item(&storage_key);
            return Ok(None);
        }

        Ok(Some(entry))
    }

    /// Store an entry
    fn put_entry(&self, key: &K, entry: StoredEntry) -> CacheResult<()> {
        let storage = get_local_storage()?;
        let storage_key = self.storage_key(key);

        let value_str = serde_json::to_string(&entry).map_err(|e| {
            CacheError::Serialization(format!("Failed to serialize cache entry: {e}"))
        })?;

        storage
            .set_item(&storage_key, &value_str)
            .map_err(|_| CacheError::StorageQuotaExceeded)?;

        Ok(())
    }

    /// Remove an entry
    fn remove_entry(&self, key: &K) -> CacheResult<bool> {
        let storage = get_local_storage()?;
        let storage_key = self.storage_key(key);

        // Check if it exists first
        let exists = storage
            .get_item(&storage_key)
            .map_err(|_| CacheError::Io(std::io::Error::other("Failed to read from localStorage")))?
            .is_some();

        if exists {
            storage.remove_item(&storage_key).map_err(|_| {
                CacheError::Io(std::io::Error::other("Failed to remove from localStorage"))
            })?;
        }

        Ok(exists)
    }

    /// Get all cache keys from localStorage
    fn get_all_keys(&self) -> CacheResult<Vec<String>> {
        let storage = get_local_storage()?;
        let len = storage.length().map_err(|_| {
            CacheError::Io(std::io::Error::other("Failed to get localStorage length"))
        })?;

        let mut keys = Vec::new();
        for i in 0..len {
            if let Ok(Some(key)) = storage.key(i) {
                if key.starts_with(KEY_PREFIX) {
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }

    /// Calculate total size of cached data
    fn calculate_size(&self) -> CacheResult<usize> {
        let storage = get_local_storage()?;
        let keys = self.get_all_keys()?;
        let mut total = 0;

        for key in keys {
            if let Ok(Some(value_str)) = storage.get_item(&key) {
                if let Ok(entry) = serde_json::from_str::<StoredEntry>(&value_str) {
                    if !entry.is_expired() {
                        total += entry.size_bytes;
                    }
                }
            }
        }

        Ok(total)
    }

    /// Clean up expired entries (call periodically to reclaim space)
    pub fn cleanup_expired(&self) -> CacheResult<usize> {
        let storage = get_local_storage()?;
        let keys = self.get_all_keys()?;
        let mut removed = 0;

        for key in keys {
            if let Ok(Some(value_str)) = storage.get_item(&key) {
                if let Ok(entry) = serde_json::from_str::<StoredEntry>(&value_str) {
                    if entry.is_expired() {
                        let _ = storage.remove_item(&key);
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Evict entries to make room (LRU-ish based on creation time)
    fn evict_if_needed(&self, needed_bytes: usize) -> CacheResult<()> {
        let current_size = self.calculate_size()?;
        if current_size + needed_bytes <= self.max_size_bytes {
            return Ok(());
        }

        // Clean up expired entries first
        self.cleanup_expired()?;

        let current_size = self.calculate_size()?;
        if current_size + needed_bytes <= self.max_size_bytes {
            return Ok(());
        }

        // Still need more space - evict oldest entries
        let storage = get_local_storage()?;
        let keys = self.get_all_keys()?;

        // Collect entries with creation times
        let mut entries: Vec<(String, u64, usize)> = Vec::new();
        for key in keys {
            if let Ok(Some(value_str)) = storage.get_item(&key) {
                if let Ok(entry) = serde_json::from_str::<StoredEntry>(&value_str) {
                    entries.push((key, entry.created_at_ms, entry.size_bytes));
                }
            }
        }

        // Sort by creation time (oldest first)
        entries.sort_by_key(|e| e.1);

        // Evict until we have enough space
        let mut freed = 0;
        let target_free = (current_size + needed_bytes).saturating_sub(self.max_size_bytes);

        for (key, _, size) in entries {
            if freed >= target_free {
                break;
            }
            let _ = storage.remove_item(&key);
            freed += size;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl<K: CacheKey + 'static> AsyncCache<K> for LocalStorageCache<K> {
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>> {
        match self.get_entry(key)? {
            Some(entry) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Ok(entry.value().map(Bytes::from))
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
        }
    }

    async fn put(&self, key: K, value: Bytes) -> CacheResult<()> {
        self.put_with_ttl(key, value, self.default_ttl).await
    }

    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()> {
        // Evict if needed
        self.evict_if_needed(value.len())?;

        let entry = StoredEntry::new(&value, Some(ttl));
        self.put_entry(&key, entry)?;
        self.puts.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn contains(&self, key: &K) -> CacheResult<bool> {
        Ok(self.get_entry(key)?.is_some())
    }

    async fn remove(&self, key: &K) -> CacheResult<bool> {
        let removed = self.remove_entry(key)?;
        if removed {
            self.removes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(removed)
    }

    async fn clear(&self) -> CacheResult<()> {
        let storage = get_local_storage()?;
        let keys = self.get_all_keys()?;

        for key in keys {
            let _ = storage.remove_item(&key);
        }

        Ok(())
    }

    async fn stats(&self) -> CacheResult<CacheStats> {
        let keys = self.get_all_keys()?;
        let size = self.calculate_size()?;
        let now_ms = js_sys::Date::now() as u64;

        Ok(CacheStats {
            get_count: self.hits.load(Ordering::Relaxed) + self.misses.load(Ordering::Relaxed),
            hit_count: self.hits.load(Ordering::Relaxed),
            miss_count: self.misses.load(Ordering::Relaxed),
            put_count: self.puts.load(Ordering::Relaxed),
            remove_count: self.removes.load(Ordering::Relaxed),
            eviction_count: 0,
            expiration_count: 0,
            entry_count: keys.len(),
            memory_usage_bytes: size,
            max_memory_usage_bytes: self.max_size_bytes,
            created_at_ms: now_ms, // We don't track creation time, use current
            updated_at_ms: now_ms,
            avg_get_time: std::time::Duration::ZERO,
            avg_put_time: std::time::Duration::ZERO,
        })
    }

    async fn size(&self) -> CacheResult<usize> {
        let keys = self.get_all_keys()?;
        // Count non-expired entries
        let storage = get_local_storage()?;
        let mut count = 0;

        for key in keys {
            if let Ok(Some(value_str)) = storage.get_item(&key) {
                if let Ok(entry) = serde_json::from_str::<StoredEntry>(&value_str) {
                    if !entry.is_expired() {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

/// Configuration for LocalStorage cache
#[derive(Debug, Clone)]
pub struct LocalStorageCacheConfig {
    /// Default TTL for entries
    pub default_ttl: Duration,
    /// Maximum storage size (soft limit)
    pub max_size_bytes: usize,
}

impl Default for LocalStorageCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5 minutes
            max_size_bytes: 5 * 1024 * 1024,       // 5MB
        }
    }
}

impl LocalStorageCacheConfig {
    /// Create new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set default TTL
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Set maximum size
    pub fn with_max_size(mut self, max_bytes: usize) -> Self {
        self.max_size_bytes = max_bytes;
        self
    }

    /// Build the cache
    pub fn build<K: CacheKey>(self) -> CacheResult<LocalStorageCache<K>> {
        LocalStorageCache::new(self.default_ttl, self.max_size_bytes)
    }
}

#[cfg(test)]
mod tests {
    // Tests would need to run in a browser environment via wasm-pack test
    // Skipping unit tests for now as they require browser APIs
}
