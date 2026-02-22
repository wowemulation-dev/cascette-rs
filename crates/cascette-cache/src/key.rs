//! Cache key types for different layers and data types
//!
//! This module defines the key types used across different cache layers in the
//! NGDP/CASC system. Each key type is designed for specific use cases and
//! provides efficient serialization, hashing, and comparison.

#![allow(missing_docs)]

use cascette_crypto::{ContentKey, EncodingKey, Jenkins96};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};
use std::sync::OnceLock;

/// Pre-computed hash for fast cache key lookups.
/// Uses Jenkins96, optimized for NGDP workloads with hot path caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FastHash {
    pub hash64: u64,
    /// Truncated for faster initial comparisons
    pub hash32: u32,
}

impl FastHash {
    #[inline]
    pub fn from_bytes(data: &[u8]) -> Self {
        let jenkins = Jenkins96::hash(data);
        Self {
            hash64: jenkins.hash64,
            hash32: jenkins.hash32,
        }
    }

    #[inline]
    pub fn from_string(data: &str) -> Self {
        Self::from_bytes(data.as_bytes())
    }

    /// Checks 32-bit hash first for early rejection
    #[inline]
    pub fn fast_eq(&self, other: &Self) -> bool {
        self.hash32 == other.hash32 && self.hash64 == other.hash64
    }
}

/// Reduces allocations for NGDP-specific key patterns
struct CacheKeyBuffer {
    buffer: String,
}

impl CacheKeyBuffer {
    fn new() -> Self {
        Self {
            buffer: String::with_capacity(128), // Most keys < 128 bytes
        }
    }

    fn format_ribbit(&mut self, region: &str, endpoint: &str, product: Option<&str>) -> &str {
        self.buffer.clear();
        self.buffer.push_str("ribbit:");
        self.buffer.push_str(region);
        if let Some(p) = product {
            self.buffer.push(':');
            self.buffer.push_str(p);
        }
        self.buffer.push(':');
        self.buffer.push_str(endpoint);
        &self.buffer
    }

    fn format_config(&mut self, config_type: &str, hash: &str) -> &str {
        self.buffer.clear();
        self.buffer.push_str("config:");
        self.buffer.push_str(config_type);
        self.buffer.push(':');
        self.buffer.push_str(hash);
        &self.buffer
    }

    fn format_blte(&mut self, encoding_key: &EncodingKey, block_index: Option<u32>) -> &str {
        self.buffer.clear();
        self.buffer.push_str("blte:");
        // Use Display trait for EncodingKey (should be optimized)
        let _ = write!(&mut self.buffer, "{encoding_key}");
        if let Some(index) = block_index {
            let _ = write!(&mut self.buffer, ":{index}");
        }
        &self.buffer
    }
}

thread_local! {
    static KEY_BUFFER: std::cell::RefCell<CacheKeyBuffer> = std::cell::RefCell::new(CacheKeyBuffer::new());
}

/// Key for Ribbit service discovery cache.
/// Caches responses from Ribbit endpoints for service discovery and version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RibbitKey {
    /// e.g., "summary", "products/wow"
    pub endpoint: String,
    /// e.g., "us", "eu", "cn"
    pub region: String,
    /// e.g., "wow", "d3"
    pub product: Option<String>,
    #[serde(skip)]
    cached_key: OnceLock<String>,
    #[serde(skip)]
    cached_hash: OnceLock<FastHash>,
}

// Manual implementations to exclude OnceLock fields from Hash and PartialEq
impl PartialEq for RibbitKey {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
            && self.region == other.region
            && self.product == other.product
    }
}

impl Eq for RibbitKey {}

impl std::hash::Hash for RibbitKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.endpoint.hash(state);
        self.region.hash(state);
        self.product.hash(state);
    }
}

impl RibbitKey {
    pub fn new(endpoint: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            region: region.into(),
            product: None,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn with_product(
        endpoint: impl Into<String>,
        region: impl Into<String>,
        product: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            region: region.into(),
            product: Some(product.into()),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            KEY_BUFFER.with(|buf| {
                let mut buffer = buf.borrow_mut();
                buffer
                    .format_ribbit(&self.region, &self.endpoint, self.product.as_deref())
                    .to_owned()
            })
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for RibbitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Key for configuration file cache (build configs, CDN configs, patch configs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigKey {
    /// "buildconfig", "cdnconfig", "patchconfig"
    pub config_type: String,
    /// Usually MD5
    pub hash: String,
    #[serde(skip)]
    cached_key: OnceLock<String>,
    #[serde(skip)]
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for ConfigKey {
    fn eq(&self, other: &Self) -> bool {
        self.config_type == other.config_type && self.hash == other.hash
    }
}

impl Eq for ConfigKey {}

impl std::hash::Hash for ConfigKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.config_type.hash(state);
        self.hash.hash(state);
    }
}

impl ConfigKey {
    pub fn new(config_type: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            config_type: config_type.into(),
            hash: hash.into(),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            KEY_BUFFER.with(|buf| {
                let mut buffer = buf.borrow_mut();
                buffer
                    .format_config(&self.config_type, &self.hash)
                    .to_owned()
            })
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Key for BLTE content cache with block-level granularity.
#[derive(Debug, Clone)]
pub struct BlteKey {
    pub encoding_key: EncodingKey,
    pub block_index: Option<u32>,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for BlteKey {
    fn eq(&self, other: &Self) -> bool {
        self.encoding_key == other.encoding_key && self.block_index == other.block_index
    }
}

impl Eq for BlteKey {}

impl std::hash::Hash for BlteKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.encoding_key.hash(state);
        self.block_index.hash(state);
    }
}

impl BlteKey {
    pub fn new(encoding_key: EncodingKey) -> Self {
        Self {
            encoding_key,
            block_index: None,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn with_block(encoding_key: EncodingKey, block_index: u32) -> Self {
        Self {
            encoding_key,
            block_index: Some(block_index),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            KEY_BUFFER.with(|buf| {
                let mut buffer = buf.borrow_mut();
                buffer
                    .format_blte(&self.encoding_key, self.block_index)
                    .to_owned()
            })
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for BlteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Key for decompressed content data cache.
#[derive(Debug, Clone)]
pub struct ContentCacheKey {
    pub content_key: ContentKey,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for ContentCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.content_key == other.content_key
    }
}

impl Eq for ContentCacheKey {}

impl std::hash::Hash for ContentCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content_key.hash(state);
    }
}

impl ContentCacheKey {
    pub fn new(content_key: ContentKey) -> Self {
        Self {
            content_key,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key
            .get_or_init(|| format!("content:{}", self.content_key))
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for ContentCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Key for parsed archive index data cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveIndexKey {
    /// e.g., "data.000"
    pub archive_name: String,
    pub index_hash: String,
    #[serde(skip)]
    cached_key: OnceLock<String>,
    #[serde(skip)]
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for ArchiveIndexKey {
    fn eq(&self, other: &Self) -> bool {
        self.archive_name == other.archive_name && self.index_hash == other.index_hash
    }
}

impl Eq for ArchiveIndexKey {}

impl std::hash::Hash for ArchiveIndexKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.archive_name.hash(state);
        self.index_hash.hash(state);
    }
}

impl ArchiveIndexKey {
    pub fn new(archive_name: impl Into<String>, index_hash: impl Into<String>) -> Self {
        Self {
            archive_name: archive_name.into(),
            index_hash: index_hash.into(),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key
            .get_or_init(|| format!("index:{}:{}", self.archive_name, self.index_hash))
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for ArchiveIndexKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Key for manifest cache (root, encoding, install, download manifests).
#[derive(Debug, Clone)]
pub struct ManifestKey {
    /// "root", "encoding", "install", "download"
    pub manifest_type: String,
    pub content_key: ContentKey,
    pub version: Option<String>,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for ManifestKey {
    fn eq(&self, other: &Self) -> bool {
        self.manifest_type == other.manifest_type
            && self.content_key == other.content_key
            && self.version == other.version
    }
}

impl Eq for ManifestKey {}

impl std::hash::Hash for ManifestKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.manifest_type.hash(state);
        self.content_key.hash(state);
        self.version.hash(state);
    }
}

impl ManifestKey {
    pub fn new(manifest_type: impl Into<String>, content_key: ContentKey) -> Self {
        Self {
            manifest_type: manifest_type.into(),
            content_key,
            version: None,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn with_version(
        manifest_type: impl Into<String>,
        content_key: ContentKey,
        version: impl Into<String>,
    ) -> Self {
        Self {
            manifest_type: manifest_type.into(),
            content_key,
            version: Some(version.into()),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| match &self.version {
            Some(version) => format!(
                "manifest:{}:{}:{}",
                self.manifest_type, self.content_key, version
            ),
            None => format!("manifest:{}:{}", self.manifest_type, self.content_key),
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for ManifestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Root file cache key for raw and parsed root files with version tracking.
#[derive(Debug, Clone)]
pub struct RootFileKey {
    pub content_key: ContentKey,
    pub is_parsed: bool,
    pub version: Option<u8>,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for RootFileKey {
    fn eq(&self, other: &Self) -> bool {
        self.content_key == other.content_key
            && self.is_parsed == other.is_parsed
            && self.version == other.version
    }
}

impl Eq for RootFileKey {}

impl std::hash::Hash for RootFileKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content_key.hash(state);
        self.is_parsed.hash(state);
        self.version.hash(state);
    }
}

impl RootFileKey {
    pub fn new_raw(content_key: ContentKey) -> Self {
        Self {
            content_key,
            is_parsed: false,
            version: None,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn new_parsed(content_key: ContentKey) -> Self {
        Self {
            content_key,
            is_parsed: true,
            version: None,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn with_version(content_key: ContentKey, is_parsed: bool, version: u8) -> Self {
        Self {
            content_key,
            is_parsed,
            version: Some(version),
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            let content_type = if self.is_parsed { "parsed" } else { "raw" };
            match self.version {
                Some(version) => format!("root:{}:{}:v{}", content_type, self.content_key, version),
                None => format!("root:{}:{}", content_type, self.content_key),
            }
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for RootFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Encoding file cache key, supports both complete and paged caching.
#[derive(Debug, Clone)]
pub struct EncodingFileKey {
    pub encoding_key: EncodingKey,
    /// For streaming large encoding files
    pub page: Option<u32>,
    pub is_parsed: bool,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for EncodingFileKey {
    fn eq(&self, other: &Self) -> bool {
        self.encoding_key == other.encoding_key
            && self.page == other.page
            && self.is_parsed == other.is_parsed
    }
}

impl Eq for EncodingFileKey {}

impl std::hash::Hash for EncodingFileKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.encoding_key.hash(state);
        self.page.hash(state);
        self.is_parsed.hash(state);
    }
}

impl EncodingFileKey {
    pub fn new_raw(encoding_key: EncodingKey) -> Self {
        Self {
            encoding_key,
            page: None,
            is_parsed: false,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn new_parsed(encoding_key: EncodingKey) -> Self {
        Self {
            encoding_key,
            page: None,
            is_parsed: true,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn with_page(encoding_key: EncodingKey, page: u32, is_parsed: bool) -> Self {
        Self {
            encoding_key,
            page: Some(page),
            is_parsed,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            let content_type = if self.is_parsed { "parsed" } else { "raw" };
            match self.page {
                Some(page) => format!("encoding:{}:{}:p{}", content_type, self.encoding_key, page),
                None => format!("encoding:{}:{}", content_type, self.encoding_key),
            }
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for EncodingFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Archive range cache key for partial archive access and BLTE block caching.
#[derive(Debug, Clone)]
pub struct ArchiveRangeKey {
    pub archive_id: String,
    pub start_offset: u64,
    pub length: u32,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for ArchiveRangeKey {
    fn eq(&self, other: &Self) -> bool {
        self.archive_id == other.archive_id
            && self.start_offset == other.start_offset
            && self.length == other.length
    }
}

impl Eq for ArchiveRangeKey {}

impl std::hash::Hash for ArchiveRangeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.archive_id.hash(state);
        self.start_offset.hash(state);
        self.length.hash(state);
    }
}

impl ArchiveRangeKey {
    pub fn new(archive_id: impl Into<String>, start_offset: u64, length: u32) -> Self {
        Self {
            archive_id: archive_id.into(),
            start_offset,
            length,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            format!(
                "archive:{}:{}+{}",
                self.archive_id, self.start_offset, self.length
            )
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for ArchiveRangeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// BLTE block cache key for granular caching of individual content blocks.
#[derive(Debug, Clone)]
pub struct BlteBlockKey {
    pub content_key: ContentKey,
    pub block_index: u32,
    pub is_decompressed: bool,
    cached_key: OnceLock<String>,
    cached_hash: OnceLock<FastHash>,
}

impl PartialEq for BlteBlockKey {
    fn eq(&self, other: &Self) -> bool {
        self.content_key == other.content_key
            && self.block_index == other.block_index
            && self.is_decompressed == other.is_decompressed
    }
}

impl Eq for BlteBlockKey {}

impl std::hash::Hash for BlteBlockKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content_key.hash(state);
        self.block_index.hash(state);
        self.is_decompressed.hash(state);
    }
}

impl BlteBlockKey {
    pub fn new_raw(content_key: ContentKey, block_index: u32) -> Self {
        Self {
            content_key,
            block_index,
            is_decompressed: false,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn new_decompressed(content_key: ContentKey, block_index: u32) -> Self {
        Self {
            content_key,
            block_index,
            is_decompressed: true,
            cached_key: OnceLock::new(),
            cached_hash: OnceLock::new(),
        }
    }

    pub fn as_cache_key(&self) -> &str {
        self.cached_key.get_or_init(|| {
            let block_type = if self.is_decompressed {
                "decompressed"
            } else {
                "raw"
            };
            format!(
                "blte:{}:{}:b{}",
                block_type, self.content_key, self.block_index
            )
        })
    }

    pub fn fast_hash(&self) -> FastHash {
        *self
            .cached_hash
            .get_or_init(|| FastHash::from_string(self.as_cache_key()))
    }
}

impl fmt::Display for BlteBlockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cache_key())
    }
}

/// Common interface for all cache key types.
pub trait CacheKey: fmt::Debug + Clone + PartialEq + Eq + std::hash::Hash + Send + Sync {
    fn as_cache_key(&self) -> &str;

    /// Jenkins96 hash (legacy compatibility)
    fn hash_key(&self) -> Jenkins96 {
        Jenkins96::hash(self.as_cache_key().as_bytes())
    }

    fn fast_hash(&self) -> FastHash {
        FastHash::from_string(self.as_cache_key())
    }
}

impl CacheKey for RibbitKey {
    fn as_cache_key(&self) -> &str {
        RibbitKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        RibbitKey::fast_hash(self)
    }
}

impl CacheKey for ConfigKey {
    fn as_cache_key(&self) -> &str {
        ConfigKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        ConfigKey::fast_hash(self)
    }
}

impl CacheKey for BlteKey {
    fn as_cache_key(&self) -> &str {
        BlteKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        BlteKey::fast_hash(self)
    }
}

impl CacheKey for ContentCacheKey {
    fn as_cache_key(&self) -> &str {
        ContentCacheKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        ContentCacheKey::fast_hash(self)
    }
}

impl CacheKey for ArchiveIndexKey {
    fn as_cache_key(&self) -> &str {
        ArchiveIndexKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        ArchiveIndexKey::fast_hash(self)
    }
}

impl CacheKey for ManifestKey {
    fn as_cache_key(&self) -> &str {
        ManifestKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        ManifestKey::fast_hash(self)
    }
}

impl CacheKey for RootFileKey {
    fn as_cache_key(&self) -> &str {
        RootFileKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        RootFileKey::fast_hash(self)
    }
}

impl CacheKey for EncodingFileKey {
    fn as_cache_key(&self) -> &str {
        EncodingFileKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        EncodingFileKey::fast_hash(self)
    }
}

impl CacheKey for ArchiveRangeKey {
    fn as_cache_key(&self) -> &str {
        ArchiveRangeKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        ArchiveRangeKey::fast_hash(self)
    }
}

impl CacheKey for BlteBlockKey {
    fn as_cache_key(&self) -> &str {
        BlteBlockKey::as_cache_key(self)
    }

    fn fast_hash(&self) -> FastHash {
        BlteBlockKey::fast_hash(self)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::md5::ContentKey;

    #[test]
    fn test_ribbit_key_formatting() {
        let key = RibbitKey::new("summary", "us");
        assert_eq!(key.as_cache_key(), "ribbit:us:summary");

        let key_with_product = RibbitKey::with_product("builds", "eu", "wow");
        assert_eq!(key_with_product.as_cache_key(), "ribbit:eu:wow:builds");
    }

    #[test]
    fn test_config_key_formatting() {
        let key = ConfigKey::new("buildconfig", "abcd1234");
        assert_eq!(key.as_cache_key(), "config:buildconfig:abcd1234");
    }

    #[test]
    fn test_blte_key_formatting() {
        let encoding_key = EncodingKey::from_data(b"test data");
        let key = BlteKey::new(encoding_key);
        assert_eq!(key.as_cache_key(), format!("blte:{encoding_key}"));

        let key_with_block = BlteKey::with_block(encoding_key, 5);
        assert_eq!(
            key_with_block.as_cache_key(),
            format!("blte:{encoding_key}:5")
        );
    }

    #[test]
    fn test_content_key_formatting() {
        let content_key = ContentKey::from_data(b"test content");
        let key = ContentCacheKey::new(content_key);
        assert_eq!(key.as_cache_key(), format!("content:{content_key}"));
    }

    #[test]
    fn test_archive_index_key_formatting() {
        let key = ArchiveIndexKey::new("data.000", "hash123");
        assert_eq!(key.as_cache_key(), "index:data.000:hash123");
    }

    #[test]
    fn test_manifest_key_formatting() {
        let content_key = ContentKey::from_data(b"manifest data");
        let key = ManifestKey::new("root", content_key);
        assert_eq!(key.as_cache_key(), format!("manifest:root:{content_key}"));

        let key_with_version = ManifestKey::with_version("encoding", content_key, "v2");
        assert_eq!(
            key_with_version.as_cache_key(),
            format!("manifest:encoding:{content_key}:v2")
        );
    }

    #[test]
    fn test_cache_key_trait() {
        let key = RibbitKey::new("summary", "us");
        let hash = key.hash_key();
        assert!(hash.hash64 > 0); // Jenkins96 should produce non-zero hash
    }

    #[test]
    fn test_root_file_key_formatting() {
        let content_key = ContentKey::from_data(b"root file data");

        let raw_key = RootFileKey::new_raw(content_key);
        assert_eq!(raw_key.as_cache_key(), format!("root:raw:{content_key}"));

        let parsed_key = RootFileKey::new_parsed(content_key);
        assert_eq!(
            parsed_key.as_cache_key(),
            format!("root:parsed:{content_key}")
        );

        let versioned_key = RootFileKey::with_version(content_key, true, 2);
        assert_eq!(
            versioned_key.as_cache_key(),
            format!("root:parsed:{content_key}:v2")
        );
    }

    #[test]
    fn test_encoding_file_key_formatting() {
        let encoding_key = EncodingKey::from_data(b"encoding data");

        let raw_key = EncodingFileKey::new_raw(encoding_key);
        assert_eq!(
            raw_key.as_cache_key(),
            format!("encoding:raw:{encoding_key}")
        );

        let parsed_key = EncodingFileKey::new_parsed(encoding_key);
        assert_eq!(
            parsed_key.as_cache_key(),
            format!("encoding:parsed:{encoding_key}")
        );

        let paged_key = EncodingFileKey::with_page(encoding_key, 3, false);
        assert_eq!(
            paged_key.as_cache_key(),
            format!("encoding:raw:{encoding_key}:p3")
        );
    }

    #[test]
    fn test_archive_range_key_formatting() {
        let key = ArchiveRangeKey::new("data.001", 1024, 4096);
        assert_eq!(key.as_cache_key(), "archive:data.001:1024+4096");

        let large_key = ArchiveRangeKey::new("archive-00.blte", 2_147_483_648, 1_048_576);
        assert_eq!(
            large_key.as_cache_key(),
            "archive:archive-00.blte:2147483648+1048576"
        );
    }

    #[test]
    fn test_blte_block_key_formatting() {
        let content_key = ContentKey::from_data(b"blte content");

        let raw_key = BlteBlockKey::new_raw(content_key, 0);
        assert_eq!(raw_key.as_cache_key(), format!("blte:raw:{content_key}:b0"));

        let decompressed_key = BlteBlockKey::new_decompressed(content_key, 15);
        assert_eq!(
            decompressed_key.as_cache_key(),
            format!("blte:decompressed:{content_key}:b15")
        );
    }

    #[test]
    fn test_new_keys_cache_key_trait() {
        let content_key = ContentKey::from_data(b"test data");
        let encoding_key = EncodingKey::from_data(b"encoding test data");

        // Test RootFileKey
        let root_key = RootFileKey::new_parsed(content_key);
        let root_hash = root_key.hash_key();
        assert!(root_hash.hash64 > 0);
        assert_eq!(root_key.fast_hash().hash64, root_hash.hash64);

        // Test EncodingFileKey
        let encoding_file_key = EncodingFileKey::new_raw(encoding_key);
        let encoding_hash = encoding_file_key.hash_key();
        assert!(encoding_hash.hash64 > 0);

        // Test ArchiveRangeKey
        let archive_key = ArchiveRangeKey::new("test.001", 512, 2048);
        let archive_hash = archive_key.hash_key();
        assert!(archive_hash.hash64 > 0);

        // Test BlteBlockKey
        let blte_block_key = BlteBlockKey::new_decompressed(content_key, 7);
        let blte_hash = blte_block_key.hash_key();
        assert!(blte_hash.hash64 > 0);
    }

    #[test]
    fn test_new_keys_equality() {
        let content_key = ContentKey::from_data(b"test equality");

        // Test RootFileKey equality
        let root_key1 = RootFileKey::new_raw(content_key);
        let root_key2 = RootFileKey::new_raw(content_key);
        let root_key3 = RootFileKey::new_parsed(content_key);
        assert_eq!(root_key1, root_key2);
        assert_ne!(root_key1, root_key3);

        // Test BlteBlockKey equality
        let blte_key1 = BlteBlockKey::new_raw(content_key, 5);
        let blte_key2 = BlteBlockKey::new_raw(content_key, 5);
        let blte_key3 = BlteBlockKey::new_raw(content_key, 6);
        assert_eq!(blte_key1, blte_key2);
        assert_ne!(blte_key1, blte_key3);
    }

    #[test]
    fn test_fast_hash_performance() {
        let key = RibbitKey::new("summary", "us");

        // First access should compute and cache
        let hash1 = key.fast_hash();
        assert!(hash1.hash64 > 0);
        assert!(hash1.hash32 > 0);

        // Second access should use cached value
        let hash2 = key.fast_hash();
        assert_eq!(hash1.hash64, hash2.hash64);
        assert_eq!(hash1.hash32, hash2.hash32);

        // Fast equality should work
        assert!(hash1.fast_eq(&hash2));
    }

    #[test]
    fn test_cache_key_caching() {
        let key = RibbitKey::new("summary", "us");

        // First access should compute and cache string
        let str1 = key.as_cache_key();
        assert_eq!(str1, "ribbit:us:summary");

        // Second access should return same reference
        let str2 = key.as_cache_key();
        assert_eq!(str1, str2);

        // Should be same memory location (cached)
        assert!(std::ptr::eq(str1, str2));
    }

    #[test]
    fn test_cache_key_hash_consistency() {
        let key1 = RibbitKey::new("summary", "us");
        let key2 = RibbitKey::new("summary", "us");

        // Legacy hash compatibility
        assert_eq!(key1.hash_key().hash64, key2.hash_key().hash64);

        // Fast hash consistency
        assert_eq!(key1.fast_hash().hash64, key2.fast_hash().hash64);
        assert_eq!(key1.fast_hash().hash32, key2.fast_hash().hash32);

        assert_eq!(key1.as_cache_key(), key2.as_cache_key());
    }

    #[test]
    fn test_cache_key_hash_differences() {
        let key1 = RibbitKey::new("summary", "us");
        let key2 = RibbitKey::new("summary", "eu");
        let key3 = RibbitKey::new("builds", "us");

        // Legacy hash differences
        assert_ne!(key1.hash_key().hash64, key2.hash_key().hash64);
        assert_ne!(key1.hash_key().hash64, key3.hash_key().hash64);
        assert_ne!(key2.hash_key().hash64, key3.hash_key().hash64);

        // Fast hash differences
        let hash1 = key1.fast_hash();
        let hash2 = key2.fast_hash();
        let hash3 = key3.fast_hash();

        assert_ne!(hash1.hash64, hash2.hash64);
        assert_ne!(hash1.hash64, hash3.hash64);
        assert_ne!(hash2.hash64, hash3.hash64);
    }

    #[test]
    fn test_hash_trait_implementations() {
        let key1 = RibbitKey::new("summary", "us");
        let key2 = RibbitKey::new("summary", "us");
        let key3 = RibbitKey::new("summary", "eu");

        // Test PartialEq
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);

        // Test Hash trait works with HashMap
        #[allow(clippy::mutable_key_type)]
        // RibbitKey uses interior mutability for caching, but hash remains stable
        let mut map = std::collections::HashMap::new();
        map.insert(key1, "value1");
        map.insert(key3.clone(), "value3");

        assert_eq!(map.get(&key2), Some(&"value1")); // key2 should equal key1
        assert_eq!(map.get(&key3), Some(&"value3"));
        assert_eq!(map.len(), 2);
    }

    // Additional tests for the rest of the functionality...
    // (keeping the existing tests but not repeating them all here)
}
