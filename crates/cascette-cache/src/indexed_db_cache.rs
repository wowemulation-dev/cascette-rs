//! IndexedDB cache implementation for WASM platforms
//!
//! This module provides a cache backend using browser IndexedDB for WASM targets.
//! IndexedDB offers larger storage limits than LocalStorage:
//!
//! - **Storage limit**: ~50MB default, can request unlimited with user permission
//! - **Asynchronous API**: Native async operations via JavaScript promises
//! - **Binary data support**: Stores ArrayBuffer directly (no base64 overhead)
//! - **No background cleanup**: Expiration is checked lazily on reads
//!
//! This implementation is suitable for caching larger NGDP content like
//! encoding files, root files, and archive indices.
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
use bytes::Bytes;
use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransaction};

/// Database name for cascette cache
const DB_NAME: &str = "cascette_cache";

/// Object store name for cache entries
const STORE_NAME: &str = "entries";

/// Current database version
const DB_VERSION: u32 = 1;

/// Convert JsValue error to CacheError
fn js_to_cache_error(e: JsValue) -> CacheError {
    let msg = if let Some(s) = e.as_string() {
        s
    } else if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        err.message().into()
    } else {
        format!("{e:?}")
    };
    CacheError::Io(std::io::Error::other(msg))
}

/// Get the IndexedDB factory
fn get_idb_factory() -> CacheResult<IdbFactory> {
    let window = web_sys::window()
        .ok_or_else(|| CacheError::Config("No window object available".to_string()))?;

    window
        .indexed_db()
        .map_err(|_| CacheError::Config("Failed to access IndexedDB".to_string()))?
        .ok_or_else(|| CacheError::Config("IndexedDB not available".to_string()))
}

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// Create a Promise from an IdbRequest
fn request_to_promise(request: &IdbRequest) -> Promise {
    Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject_clone = reject.clone();

        let on_success = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }) as Box<dyn FnOnce(_)>);

        let on_error = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = reject_clone.call1(&JsValue::UNDEFINED, &JsValue::from_str("Request failed"));
        }) as Box<dyn FnOnce(_)>);

        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        // Prevent closures from being dropped
        on_success.forget();
        on_error.forget();
    })
}

/// Create a Promise from an IdbOpenDbRequest
fn open_request_to_promise(request: &web_sys::IdbOpenDbRequest) -> Promise {
    Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject_clone = reject.clone();

        let on_success = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }) as Box<dyn FnOnce(_)>);

        let on_error = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = reject_clone.call1(&JsValue::UNDEFINED, &JsValue::from_str("Open failed"));
        }) as Box<dyn FnOnce(_)>);

        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        // Prevent closures from being dropped
        on_success.forget();
        on_error.forget();
    })
}

/// Open the IndexedDB database
async fn open_database() -> CacheResult<IdbDatabase> {
    let factory = get_idb_factory()?;

    let open_request = factory
        .open_with_u32(DB_NAME, DB_VERSION)
        .map_err(js_to_cache_error)?;

    // Set up upgrade handler for creating object store
    let on_upgrade = Closure::once(Box::new(move |event: web_sys::IdbVersionChangeEvent| {
        let target = event.target().expect("upgrade event has target");
        let request: IdbRequest = target.unchecked_into();
        let db: IdbDatabase = request.result().expect("result exists").unchecked_into();

        // Create the object store if it doesn't exist
        let store_names = db.object_store_names();
        let mut store_exists = false;
        for i in 0..store_names.length() {
            if let Some(name) = store_names.get(i) {
                if name == STORE_NAME {
                    store_exists = true;
                    break;
                }
            }
        }
        if !store_exists {
            let params = web_sys::IdbObjectStoreParameters::new();
            params.set_key_path(&JsValue::from_str("key"));

            let _ = db.create_object_store_with_optional_parameters(STORE_NAME, &params);
        }
    }) as Box<dyn FnOnce(_)>);

    open_request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    on_upgrade.forget(); // Let the closure live

    // Wait for the database to open using our promise wrapper
    let promise = open_request_to_promise(&open_request);
    JsFuture::from(promise).await.map_err(js_to_cache_error)?;

    // Get the result
    let result = open_request.result().map_err(js_to_cache_error)?;
    Ok(result.unchecked_into())
}

/// Stored entry format for IndexedDB
#[derive(Debug)]
struct StoredEntry {
    key: String,
    value: Vec<u8>,
    created_at_ms: u64,
    expires_at_ms: u64,
    size_bytes: usize,
}

impl StoredEntry {
    fn new(key: String, value: Vec<u8>, ttl: Option<Duration>) -> Self {
        let now_ms = current_time_ms();
        let expires_at_ms = ttl.map_or(0, |t| now_ms + t.as_millis() as u64);
        let size_bytes = value.len();

        Self {
            key,
            value,
            created_at_ms: now_ms,
            expires_at_ms,
            size_bytes,
        }
    }

    fn is_expired(&self) -> bool {
        if self.expires_at_ms == 0 {
            return false;
        }
        current_time_ms() >= self.expires_at_ms
    }

    /// Convert to JavaScript object for storage
    fn to_js_object(&self) -> CacheResult<Object> {
        let obj = Object::new();

        Reflect::set(&obj, &"key".into(), &self.key.clone().into())
            .map_err(|e| CacheError::Io(std::io::Error::other(format!("{e:?}"))))?;

        let arr = Uint8Array::from(self.value.as_slice());
        Reflect::set(&obj, &"value".into(), &arr)
            .map_err(|e| CacheError::Io(std::io::Error::other(format!("{e:?}"))))?;

        Reflect::set(
            &obj,
            &"created_at_ms".into(),
            &JsValue::from_f64(self.created_at_ms as f64),
        )
        .map_err(|e| CacheError::Io(std::io::Error::other(format!("{e:?}"))))?;

        Reflect::set(
            &obj,
            &"expires_at_ms".into(),
            &JsValue::from_f64(self.expires_at_ms as f64),
        )
        .map_err(|e| CacheError::Io(std::io::Error::other(format!("{e:?}"))))?;

        Reflect::set(
            &obj,
            &"size_bytes".into(),
            &JsValue::from_f64(self.size_bytes as f64),
        )
        .map_err(|e| CacheError::Io(std::io::Error::other(format!("{e:?}"))))?;

        Ok(obj)
    }

    /// Create from JavaScript object
    fn from_js_object(obj: &JsValue) -> Option<Self> {
        let key = Reflect::get(obj, &"key".into()).ok()?.as_string()?;

        let value_js = Reflect::get(obj, &"value".into()).ok()?;
        let value_arr: Uint8Array = value_js.dyn_into().ok()?;
        let value = value_arr.to_vec();

        let created_at_ms = Reflect::get(obj, &"created_at_ms".into()).ok()?.as_f64()? as u64;

        let expires_at_ms = Reflect::get(obj, &"expires_at_ms".into()).ok()?.as_f64()? as u64;

        let size_bytes = Reflect::get(obj, &"size_bytes".into()).ok()?.as_f64()? as usize;

        Some(Self {
            key,
            value,
            created_at_ms,
            expires_at_ms,
            size_bytes,
        })
    }
}

/// IndexedDB-backed cache for WASM platforms
///
/// Provides a key-value cache using browser IndexedDB for larger content.
/// Suitable for caching NGDP content files, encoding data, and archives.
///
/// # Example
///
/// ```ignore
/// use cascette_cache::indexed_db_cache::IndexedDbCache;
/// use cascette_cache::key::ContentCacheKey;
///
/// let cache: IndexedDbCache<ContentCacheKey> = IndexedDbCache::new(
///     Duration::from_secs(3600), // 1 hour default TTL
///     100 * 1024 * 1024,         // 100MB max size
/// ).await?;
///
/// let key = ContentCacheKey::new(content_key);
/// cache.put(key, Bytes::from(large_data)).await?;
/// ```
pub struct IndexedDbCache<K: CacheKey> {
    /// Database handle (lazily opened)
    db: Arc<tokio::sync::RwLock<Option<IdbDatabase>>>,
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

impl<K: CacheKey> IndexedDbCache<K> {
    /// Create a new IndexedDB cache
    ///
    /// Note: The database is lazily opened on first access.
    ///
    /// # Arguments
    ///
    /// * `default_ttl` - Default time-to-live for entries without explicit TTL
    /// * `max_size_bytes` - Soft limit on total storage usage
    pub fn new(default_ttl: Duration, max_size_bytes: usize) -> Self {
        Self {
            db: Arc::new(tokio::sync::RwLock::new(None)),
            default_ttl,
            max_size_bytes,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            puts: AtomicU64::new(0),
            removes: AtomicU64::new(0),
            _marker: PhantomData,
        }
    }

    /// Create with default settings (1 hour TTL, 100MB limit)
    pub fn with_defaults() -> Self {
        Self::new(Duration::from_secs(3600), 100 * 1024 * 1024)
    }

    /// Get database handle, opening if necessary
    async fn get_db(&self) -> CacheResult<IdbDatabase> {
        // Check if already open
        {
            let guard = self.db.read().await;
            if let Some(ref db) = *guard {
                return Ok(db.clone());
            }
        }

        // Open the database
        let db = open_database().await?;

        // Store for reuse
        {
            let mut guard = self.db.write().await;
            *guard = Some(db.clone());
        }

        Ok(db)
    }

    /// Get a transaction and object store
    async fn get_store(
        &self,
        mode: web_sys::IdbTransactionMode,
    ) -> CacheResult<(IdbTransaction, IdbObjectStore)> {
        let db = self.get_db().await?;

        let store_names = Array::of1(&JsValue::from_str(STORE_NAME));
        let tx = db
            .transaction_with_str_sequence_and_mode(&store_names, mode)
            .map_err(js_to_cache_error)?;

        let store = tx.object_store(STORE_NAME).map_err(js_to_cache_error)?;

        Ok((tx, store))
    }

    /// Get the cache key string
    fn cache_key(&self, key: &K) -> String {
        key.as_cache_key().to_string()
    }

    /// Wait for an IDB request to complete
    async fn wait_for_request(request: &IdbRequest) -> CacheResult<JsValue> {
        let promise = request_to_promise(request);
        JsFuture::from(promise).await.map_err(js_to_cache_error)?;
        request.result().map_err(js_to_cache_error)
    }

    /// Get an entry from the database
    async fn get_entry(&self, key: &K) -> CacheResult<Option<StoredEntry>> {
        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readonly)
            .await?;

        let cache_key = self.cache_key(key);
        let request = store
            .get(&JsValue::from_str(&cache_key))
            .map_err(js_to_cache_error)?;

        let result = Self::wait_for_request(&request).await?;

        if result.is_undefined() || result.is_null() {
            return Ok(None);
        }

        let entry = StoredEntry::from_js_object(&result);

        // Check expiration
        if let Some(ref e) = entry {
            if e.is_expired() {
                // Delete expired entry
                let _ = self.delete_entry(key).await;
                return Ok(None);
            }
        }

        Ok(entry)
    }

    /// Store an entry in the database
    async fn put_entry(&self, entry: StoredEntry) -> CacheResult<()> {
        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;

        let obj = entry.to_js_object()?;
        let request = store.put(&obj).map_err(js_to_cache_error)?;

        Self::wait_for_request(&request).await?;
        Ok(())
    }

    /// Delete an entry from the database
    async fn delete_entry(&self, key: &K) -> CacheResult<bool> {
        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;

        let cache_key = self.cache_key(key);

        // Check if it exists first
        let get_request = store
            .get(&JsValue::from_str(&cache_key))
            .map_err(js_to_cache_error)?;

        let result = Self::wait_for_request(&get_request).await?;
        let exists = !result.is_undefined() && !result.is_null();

        if exists {
            let delete_request = store
                .delete(&JsValue::from_str(&cache_key))
                .map_err(js_to_cache_error)?;

            Self::wait_for_request(&delete_request).await?;
        }

        Ok(exists)
    }

    /// Get all entries (for size calculation and cleanup)
    async fn get_all_entries(&self) -> CacheResult<Vec<StoredEntry>> {
        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readonly)
            .await?;

        let request = store.get_all().map_err(js_to_cache_error)?;
        let result = Self::wait_for_request(&request).await?;

        let array: Array = result.unchecked_into();
        let mut entries = Vec::new();

        for i in 0..array.length() {
            let item = array.get(i);
            if let Some(entry) = StoredEntry::from_js_object(&item) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Calculate total size of cached data
    async fn calculate_size(&self) -> CacheResult<usize> {
        let entries = self.get_all_entries().await?;
        Ok(entries
            .iter()
            .filter(|e| !e.is_expired())
            .map(|e| e.size_bytes)
            .sum())
    }

    /// Count non-expired entries
    async fn count_entries(&self) -> CacheResult<usize> {
        let entries = self.get_all_entries().await?;
        Ok(entries.iter().filter(|e| !e.is_expired()).count())
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) -> CacheResult<usize> {
        let entries = self.get_all_entries().await?;
        let expired: Vec<_> = entries.iter().filter(|e| e.is_expired()).collect();

        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;

        let mut removed = 0;
        for entry in expired {
            let request = store
                .delete(&JsValue::from_str(&entry.key))
                .map_err(js_to_cache_error)?;

            if Self::wait_for_request(&request).await.is_ok() {
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Evict entries to make room (oldest first)
    async fn evict_if_needed(&self, needed_bytes: usize) -> CacheResult<()> {
        let current_size = self.calculate_size().await?;
        if current_size + needed_bytes <= self.max_size_bytes {
            return Ok(());
        }

        // Clean up expired entries first
        self.cleanup_expired().await?;

        let current_size = self.calculate_size().await?;
        if current_size + needed_bytes <= self.max_size_bytes {
            return Ok(());
        }

        // Still need more space - evict oldest entries
        let mut entries = self.get_all_entries().await?;
        entries.sort_by_key(|e| e.created_at_ms);

        let target_free = (current_size + needed_bytes).saturating_sub(self.max_size_bytes);
        let mut freed = 0;

        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;

        for entry in entries {
            if freed >= target_free {
                break;
            }

            let request = store
                .delete(&JsValue::from_str(&entry.key))
                .map_err(js_to_cache_error)?;

            if Self::wait_for_request(&request).await.is_ok() {
                freed += entry.size_bytes;
            }
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl<K: CacheKey + 'static> AsyncCache<K> for IndexedDbCache<K> {
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>> {
        match self.get_entry(key).await? {
            Some(entry) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(Bytes::from(entry.value)))
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
        self.evict_if_needed(value.len()).await?;

        let cache_key = self.cache_key(&key);
        let entry = StoredEntry::new(cache_key, value.to_vec(), Some(ttl));
        self.put_entry(entry).await?;
        self.puts.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn contains(&self, key: &K) -> CacheResult<bool> {
        Ok(self.get_entry(key).await?.is_some())
    }

    async fn remove(&self, key: &K) -> CacheResult<bool> {
        let removed = self.delete_entry(key).await?;
        if removed {
            self.removes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(removed)
    }

    async fn clear(&self) -> CacheResult<()> {
        let (_tx, store) = self
            .get_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;

        let request = store.clear().map_err(js_to_cache_error)?;
        Self::wait_for_request(&request).await?;
        Ok(())
    }

    async fn stats(&self) -> CacheResult<CacheStats> {
        let entry_count = self.count_entries().await?;
        let memory_usage = self.calculate_size().await?;
        let now_ms = js_sys::Date::now() as u64;

        Ok(CacheStats {
            get_count: self.hits.load(Ordering::Relaxed) + self.misses.load(Ordering::Relaxed),
            hit_count: self.hits.load(Ordering::Relaxed),
            miss_count: self.misses.load(Ordering::Relaxed),
            put_count: self.puts.load(Ordering::Relaxed),
            remove_count: self.removes.load(Ordering::Relaxed),
            eviction_count: 0,
            expiration_count: 0,
            entry_count,
            memory_usage_bytes: memory_usage,
            max_memory_usage_bytes: self.max_size_bytes,
            created_at_ms: now_ms, // We don't track creation time, use current
            updated_at_ms: now_ms,
            avg_get_time: std::time::Duration::ZERO,
            avg_put_time: std::time::Duration::ZERO,
        })
    }

    async fn size(&self) -> CacheResult<usize> {
        self.count_entries().await
    }

    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

/// Configuration for IndexedDB cache
#[derive(Debug, Clone)]
pub struct IndexedDbCacheConfig {
    /// Default TTL for entries
    pub default_ttl: Duration,
    /// Maximum storage size (soft limit)
    pub max_size_bytes: usize,
}

impl Default for IndexedDbCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(3600), // 1 hour
            max_size_bytes: 100 * 1024 * 1024,      // 100MB
        }
    }
}

impl IndexedDbCacheConfig {
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
    pub fn build<K: CacheKey>(self) -> IndexedDbCache<K> {
        IndexedDbCache::new(self.default_ttl, self.max_size_bytes)
    }
}

#[cfg(test)]
mod tests {
    // Tests would need to run in a browser environment via wasm-pack test
    // Skipping unit tests for now as they require browser APIs
}
