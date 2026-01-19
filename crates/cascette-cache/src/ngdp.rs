//! NGDP-specific cache implementations for content resolution
//!
//! This module provides the core NGDP resolution chain implementation,
//! content-addressed caching, BLTE block caching, and archive range caching
//! as specified in the cascette-cache implementation guide.
#![allow(clippy::single_match_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss

use crate::{
    error::{NgdpCacheError, NgdpCacheResult},
    integration::{EncodingFileOps, FormatConfig, RootFileOps},
    key::{ArchiveRangeKey, BlteBlockKey, EncodingFileKey, RootFileKey},
    memory_cache::MemoryCache,
    traits::AsyncCache,
    validation::{NgdpValidationHooks, ValidationHooks},
};
use bytes::Bytes;
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::root::{ContentFlags, LocaleFlags};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

/// Configuration for NGDP resolution cache
#[derive(Debug, Clone)]
pub struct NgdpResolutionConfig {
    /// Maximum number of root files to cache
    pub max_root_files: usize,
    /// Maximum number of encoding file pages to cache
    pub max_encoding_pages: usize,
    /// TTL for cached root files
    pub root_file_ttl: Duration,
    /// TTL for cached encoding files
    pub encoding_file_ttl: Duration,
    /// Enable content validation
    pub enable_validation: bool,
    /// Format parsing configuration
    pub format_config: FormatConfig,
}

impl Default for NgdpResolutionConfig {
    fn default() -> Self {
        Self {
            max_root_files: 100,
            max_encoding_pages: 1000,
            root_file_ttl: Duration::from_secs(3600), // 1 hour
            encoding_file_ttl: Duration::from_secs(7200), // 2 hours
            enable_validation: true,
            format_config: FormatConfig::default(),
        }
    }
}

/// NGDP Resolution Cache - orchestrates the entire content resolution chain
///
/// This cache handles the NGDP resolution process:
/// 1. Root file lookup (file path -> content key)
/// 2. Encoding file lookup (content key -> encoding key)
/// 3. Content retrieval with validation
pub struct NgdpResolutionCache {
    /// Cache for root files
    root_cache: Arc<MemoryCache<RootFileKey>>,
    /// Cache for encoding files
    encoding_cache: Arc<MemoryCache<EncodingFileKey>>,
    /// Validation hooks for content integrity
    #[allow(dead_code)]
    validation: Arc<NgdpValidationHooks>,
    /// Configuration
    #[allow(dead_code)]
    config: NgdpResolutionConfig,
    /// Resolution metrics
    metrics: Arc<RwLock<ResolutionMetrics>>,
}

/// Metrics for NGDP resolution operations
#[derive(Debug, Default, Clone)]
pub struct ResolutionMetrics {
    /// Total number of resolution attempts
    pub total_resolutions: u64,
    /// Number of successful resolutions
    pub successful_resolutions: u64,
    /// Number of root file cache hits
    pub root_cache_hits: u64,
    /// Number of root file cache misses
    pub root_cache_misses: u64,
    /// Number of encoding file cache hits
    pub encoding_cache_hits: u64,
    /// Number of encoding file cache misses
    pub encoding_cache_misses: u64,
    /// Average resolution time in microseconds
    pub avg_resolution_time_us: u64,
}

impl NgdpResolutionCache {
    /// Create a new NGDP resolution cache
    pub fn new(config: NgdpResolutionConfig) -> NgdpCacheResult<Self> {
        let root_config = crate::config::MemoryCacheConfig::new()
            .with_max_entries(config.max_root_files)
            .with_default_ttl(config.root_file_ttl);

        let encoding_config = crate::config::MemoryCacheConfig::new()
            .with_max_entries(config.max_encoding_pages)
            .with_default_ttl(config.encoding_file_ttl);

        let root_cache = Arc::new(MemoryCache::new(root_config)?);
        let encoding_cache = Arc::new(MemoryCache::new(encoding_config)?);
        let validation = Arc::new(NgdpValidationHooks::default());

        Ok(Self {
            root_cache,
            encoding_cache,
            validation,
            config,
            metrics: Arc::new(RwLock::new(ResolutionMetrics::default())),
        })
    }

    /// Resolve a file path to its content key using root file
    pub async fn resolve_file_to_content(
        &self,
        root_content_key: ContentKey,
        file_path: &str,
    ) -> NgdpCacheResult<Option<ContentKey>> {
        let start = Instant::now();

        // Update initial metrics
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
            })?;
            metrics.total_resolutions += 1;
        }

        // Try to get parsed root file from cache
        let root_file = match RootFileOps::get_parsed_root(
            self.root_cache.as_ref(),
            self.validation.as_ref(),
            root_content_key,
            &self.config.format_config,
        )
        .await?
        {
            Some(root) => {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.root_cache_hits += 1;
                drop(metrics);
                root
            }
            None => {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.root_cache_misses += 1;
                drop(metrics);
                // Would fetch from CDN here in full implementation
                return Ok(None);
            }
        };

        // Use RootFile's built-in resolution by path
        // Using ALL locale (matches any) and no content flags for now - could be made configurable
        let locale = LocaleFlags::new(LocaleFlags::ALL);
        let content_flags = ContentFlags::new(0);
        let result = root_file.resolve_by_path(file_path, locale, content_flags);

        let elapsed = start.elapsed().as_micros() as u64;

        // Update final metrics
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
            })?;
            if result.is_some() {
                metrics.successful_resolutions += 1;
            }
            metrics.avg_resolution_time_us =
                (metrics.avg_resolution_time_us + elapsed) / metrics.total_resolutions;
        }

        Ok(result)
    }

    /// Resolve content key to encoding key using encoding file
    pub async fn resolve_content_to_encoding(
        &self,
        encoding_key: EncodingKey,
        content_key: ContentKey,
    ) -> NgdpCacheResult<Option<EncodingKey>> {
        // Try to get parsed encoding file from cache
        let encoding_file = match EncodingFileOps::get_parsed_encoding(
            self.encoding_cache.as_ref(),
            self.validation.as_ref(),
            encoding_key,
            None, // Full file for now - could optimize with paging later
            &self.config.format_config,
        )
        .await?
        {
            Some(encoding) => {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.encoding_cache_hits += 1;
                drop(metrics);
                encoding
            }
            None => {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.encoding_cache_misses += 1;
                drop(metrics);
                // Would fetch from CDN here in full implementation
                return Ok(None);
            }
        };

        // Search through content key pages for the requested key
        // The encoding file has ckey_pages which map content keys to encoding keys
        for page in &encoding_file.ckey_pages {
            for entry in &page.entries {
                // Check if this entry's content key matches
                if entry.content_key == content_key {
                    // Found it! Return the first encoding key
                    if let Some(ekey) = entry.encoding_keys.first() {
                        return Ok(Some(*ekey));
                    }
                }
            }
        }

        // Not found in encoding file
        Ok(None)
    }

    /// Full resolution chain: file path -> content key -> encoding key
    pub async fn resolve_full_chain(
        &self,
        root_content_key: ContentKey,
        encoding_key: EncodingKey,
        file_path: &str,
    ) -> NgdpCacheResult<Option<EncodingKey>> {
        // Step 1: Resolve file path to content key
        let Some(content_key) = self
            .resolve_file_to_content(root_content_key, file_path)
            .await?
        else {
            return Ok(None);
        };

        // Step 2: Resolve content key to encoding key
        self.resolve_content_to_encoding(encoding_key, content_key)
            .await
    }

    /// Get resolution metrics
    pub fn metrics(&self) -> ResolutionMetrics {
        self.metrics
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Store a root file in cache
    pub async fn cache_root_file(
        &self,
        content_key: ContentKey,
        data: Bytes,
    ) -> NgdpCacheResult<()> {
        RootFileOps::put_raw_root(self.root_cache.as_ref(), content_key, data).await
    }

    /// Store an encoding file in cache
    pub async fn cache_encoding_file(
        &self,
        encoding_key: EncodingKey,
        data: Bytes,
        page: Option<u32>,
    ) -> NgdpCacheResult<()> {
        EncodingFileOps::put_raw_encoding(self.encoding_cache.as_ref(), encoding_key, data, page)
            .await
    }
}

/// Content-Addressed Cache with integrity verification
///
/// This cache ensures content integrity by validating content keys match
/// the actual data hash before serving cached content.
pub struct ContentAddressedCache<C> {
    /// Underlying cache implementation
    inner: Arc<C>,
    /// Validation hooks for content verification
    validation: Arc<NgdpValidationHooks>,
    /// Metrics for content validation
    metrics: Arc<RwLock<ContentValidationMetrics>>,
}

/// Metrics for content validation operations
#[derive(Debug, Default, Clone)]
pub struct ContentValidationMetrics {
    /// Total validations performed
    pub total_validations: u64,
    /// Successful validations
    pub successful_validations: u64,
    /// Failed validations (content mismatch)
    pub failed_validations: u64,
    /// Average validation time in microseconds
    pub avg_validation_time_us: u64,
}

impl<C> ContentAddressedCache<C>
where
    C: AsyncCache<BlteBlockKey> + Send + Sync,
{
    /// Create a new content-addressed cache
    pub fn new(inner: Arc<C>, validation: Arc<NgdpValidationHooks>) -> Self {
        Self {
            inner,
            validation,
            metrics: Arc::new(RwLock::new(ContentValidationMetrics::default())),
        }
    }

    /// Get content with validation
    pub async fn get_validated(&self, content_key: ContentKey) -> NgdpCacheResult<Option<Bytes>> {
        let start = Instant::now();

        // Update initial metrics
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
            })?;
            metrics.total_validations += 1;
        }

        // Get from underlying cache
        let key = BlteBlockKey::new_raw(content_key, 0);
        let Some(data) = self.inner.get(&key).await? else {
            return Ok(None);
        };

        // Validate content integrity
        let validation_result = self
            .validation
            .validate_content(&content_key, &data)
            .await?;

        let elapsed = start.elapsed().as_micros() as u64;

        if validation_result.is_valid {
            {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.successful_validations += 1;
                metrics.avg_validation_time_us =
                    (metrics.avg_validation_time_us + elapsed) / metrics.total_validations;
            }
            Ok(Some(data))
        } else {
            {
                let mut metrics = self.metrics.write().map_err(|_| {
                    crate::error::NgdpCacheError::StreamProcessingError("Lock poisoned".to_string())
                })?;
                metrics.failed_validations += 1;
            }
            Err(NgdpCacheError::ContentValidationFailed(content_key))
        }
    }

    /// Store content with validation
    pub async fn put_validated(&self, content_key: ContentKey, data: Bytes) -> NgdpCacheResult<()> {
        // Validate before storing
        let validation_result = self
            .validation
            .validate_content(&content_key, &data)
            .await?;

        if !validation_result.is_valid {
            return Err(NgdpCacheError::ContentValidationFailed(content_key));
        }

        let key = BlteBlockKey::new_raw(content_key, 0);
        self.inner.put(key, data).await.map_err(Into::into)
    }

    /// Get validation metrics
    pub fn metrics(&self) -> ContentValidationMetrics {
        self.metrics
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

/// BLTE Block Cache for partial content access
///
/// This cache manages individual BLTE blocks, enabling efficient partial
/// content access without loading entire files.
pub struct BlteBlockCache<C> {
    /// Underlying cache for blocks
    block_cache: Arc<C>,
    /// Block metadata tracking
    block_metadata: Arc<RwLock<HashMap<ContentKey, BlockMetadata>>>,
    /// Maximum blocks per content
    max_blocks_per_content: u32,
}

/// Metadata for BLTE blocks
#[derive(Debug, Clone)]
pub struct BlockMetadata {
    /// Total number of blocks
    pub total_blocks: u32,
    /// Size of each block
    pub block_sizes: Vec<u32>,
    /// Cached block indices
    pub cached_blocks: Vec<u32>,
    /// Last access time
    pub last_access: Instant,
}

impl<C> BlteBlockCache<C>
where
    C: AsyncCache<BlteBlockKey> + Send + Sync,
{
    /// Create a new BLTE block cache
    pub fn new(block_cache: Arc<C>, max_blocks_per_content: u32) -> Self {
        Self {
            block_cache,
            block_metadata: Arc::new(RwLock::new(HashMap::new())),
            max_blocks_per_content,
        }
    }

    /// Get a specific BLTE block
    pub async fn get_block(
        &self,
        content_key: ContentKey,
        block_index: u32,
        decompressed: bool,
    ) -> NgdpCacheResult<Option<Bytes>> {
        // Update metadata
        {
            let Ok(mut metadata) = self.block_metadata.write() else {
                return Err(crate::error::NgdpCacheError::StreamProcessingError(
                    "Lock poisoned".to_string(),
                ));
            };
            if let Some(meta) = metadata.get_mut(&content_key) {
                meta.last_access = Instant::now();
            }
        }

        let key = if decompressed {
            BlteBlockKey::new_decompressed(content_key, block_index)
        } else {
            BlteBlockKey::new_raw(content_key, block_index)
        };

        self.block_cache.get(&key).await.map_err(Into::into)
    }

    /// Store a BLTE block
    pub async fn put_block(
        &self,
        content_key: ContentKey,
        block_index: u32,
        data: Bytes,
        decompressed: bool,
    ) -> NgdpCacheResult<()> {
        // Check block limit
        {
            let metadata = self
                .block_metadata
                .read()
                .map_err(|_| NgdpCacheError::StreamProcessingError("Lock poisoned".to_string()))?;
            if let Some(meta) = metadata.get(&content_key) {
                if meta.cached_blocks.len() >= self.max_blocks_per_content as usize
                    && !meta.cached_blocks.contains(&block_index)
                {
                    return Err(NgdpCacheError::CacheFull);
                }
            }
        }

        let key = if decompressed {
            BlteBlockKey::new_decompressed(content_key, block_index)
        } else {
            BlteBlockKey::new_raw(content_key, block_index)
        };

        // Store block
        self.block_cache.put(key, data.clone()).await?;

        // Update metadata
        {
            let Ok(mut metadata) = self.block_metadata.write() else {
                return Err(crate::error::NgdpCacheError::StreamProcessingError(
                    "Lock poisoned".to_string(),
                ));
            };
            let meta = metadata
                .entry(content_key)
                .or_insert_with(|| BlockMetadata {
                    total_blocks: 0,
                    block_sizes: Vec::new(),
                    cached_blocks: Vec::new(),
                    last_access: Instant::now(),
                });

            if !meta.cached_blocks.contains(&block_index) {
                meta.cached_blocks.push(block_index);
                if block_index as usize >= meta.block_sizes.len() {
                    meta.block_sizes.resize(block_index as usize + 1, 0);
                }
                meta.block_sizes[block_index as usize] = data.len() as u32;
            }
            meta.last_access = Instant::now();
        }

        Ok(())
    }

    /// Get metadata for content
    pub fn get_metadata(&self, content_key: &ContentKey) -> Option<BlockMetadata> {
        self.block_metadata
            .read()
            .ok()
            .and_then(|guard| guard.get(content_key).cloned())
    }

    /// Clear old entries based on last access time
    pub fn evict_old_entries(&self, max_age: Duration) {
        let now = Instant::now();
        let Ok(mut metadata) = self.block_metadata.write() else {
            return;
        };
        metadata.retain(|_, meta| now.duration_since(meta.last_access) < max_age);
    }
}

/// Archive Cache with range request optimization
///
/// This cache manages archive data with support for efficient range requests,
/// allowing partial archive access without downloading entire files.
pub struct ArchiveCache<C> {
    /// Underlying cache for archive ranges
    range_cache: Arc<C>,
    /// Archive metadata
    archive_metadata: Arc<RwLock<HashMap<String, ArchiveMetadata>>>,
    /// Maximum ranges per archive
    max_ranges_per_archive: usize,
}

/// Metadata for archive files
#[derive(Debug, Clone)]
pub struct ArchiveMetadata {
    /// Total size of archive
    pub total_size: u64,
    /// Cached ranges (offset, length)
    pub cached_ranges: Vec<(u64, u32)>,
    /// Last access time
    pub last_access: Instant,
    /// Number of range requests
    pub access_count: u64,
}

impl<C> ArchiveCache<C>
where
    C: AsyncCache<ArchiveRangeKey> + Send + Sync,
{
    /// Create a new archive cache
    pub fn new(range_cache: Arc<C>, max_ranges_per_archive: usize) -> Self {
        Self {
            range_cache,
            archive_metadata: Arc::new(RwLock::new(HashMap::new())),
            max_ranges_per_archive,
        }
    }

    /// Get archive range data
    pub async fn get_range(
        &self,
        archive_id: &str,
        offset: u64,
        length: u32,
    ) -> NgdpCacheResult<Option<Bytes>> {
        // Update metadata
        {
            let Ok(mut metadata) = self.archive_metadata.write() else {
                return Err(crate::error::NgdpCacheError::StreamProcessingError(
                    "Lock poisoned".to_string(),
                ));
            };
            if let Some(meta) = metadata.get_mut(archive_id) {
                meta.last_access = Instant::now();
                meta.access_count += 1;
            }
        }

        let key = ArchiveRangeKey::new(archive_id, offset, length);
        self.range_cache.get(&key).await.map_err(Into::into)
    }

    /// Store archive range data
    pub async fn put_range(
        &self,
        archive_id: &str,
        offset: u64,
        length: u32,
        data: Bytes,
    ) -> NgdpCacheResult<()> {
        // Check range limit
        {
            let metadata = self
                .archive_metadata
                .read()
                .map_err(|_| NgdpCacheError::StreamProcessingError("Lock poisoned".to_string()))?;
            if let Some(meta) = metadata.get(archive_id) {
                if meta.cached_ranges.len() >= self.max_ranges_per_archive
                    && !meta.cached_ranges.contains(&(offset, length))
                {
                    return Err(NgdpCacheError::CacheFull);
                }
            }
        }

        let key = ArchiveRangeKey::new(archive_id, offset, length);
        self.range_cache.put(key, data).await?;

        // Update metadata
        {
            let Ok(mut metadata) = self.archive_metadata.write() else {
                return Err(crate::error::NgdpCacheError::StreamProcessingError(
                    "Lock poisoned".to_string(),
                ));
            };
            let meta = metadata
                .entry(archive_id.to_string())
                .or_insert_with(|| ArchiveMetadata {
                    total_size: 0,
                    cached_ranges: Vec::new(),
                    last_access: Instant::now(),
                    access_count: 0,
                });

            let range = (offset, length);
            if !meta.cached_ranges.contains(&range) {
                meta.cached_ranges.push(range);
            }
            meta.last_access = Instant::now();
        }

        Ok(())
    }

    /// Check if a range is cached
    pub fn is_range_cached(&self, archive_id: &str, offset: u64, length: u32) -> bool {
        self.archive_metadata
            .read()
            .ok()
            .and_then(|guard| {
                guard
                    .get(archive_id)
                    .map(|meta| meta.cached_ranges.contains(&(offset, length)))
            })
            .unwrap_or(false)
    }

    /// Get metadata for an archive
    pub fn get_metadata(&self, archive_id: &str) -> Option<ArchiveMetadata> {
        self.archive_metadata
            .read()
            .ok()
            .and_then(|guard| guard.get(archive_id).cloned())
    }

    /// Find overlapping cached ranges
    pub fn find_overlapping_ranges(
        &self,
        archive_id: &str,
        offset: u64,
        length: u32,
    ) -> Vec<(u64, u32)> {
        let Ok(metadata) = self.archive_metadata.read() else {
            return Vec::new();
        };
        if let Some(meta) = metadata.get(archive_id) {
            let end = offset + u64::from(length);
            meta.cached_ranges
                .iter()
                .filter(|(r_offset, r_length)| {
                    let r_end = r_offset + u64::from(*r_length);
                    // Check for overlap
                    *r_offset < end && offset < r_end
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::MemoryCacheConfig;

    #[tokio::test]
    async fn test_ngdp_resolution_cache_creation() {
        let config = NgdpResolutionConfig::default();
        let cache = NgdpResolutionCache::new(config).expect("Operation should succeed");
        let metrics = cache.metrics();
        assert_eq!(metrics.total_resolutions, 0);
        assert_eq!(metrics.successful_resolutions, 0);
    }

    #[tokio::test]
    async fn test_content_addressed_cache() {
        let config = MemoryCacheConfig::default();
        let inner = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));
        let validation = Arc::new(NgdpValidationHooks::default());
        let cache = ContentAddressedCache::new(inner, validation);

        let data = Bytes::from("test data");
        let content_key = ContentKey::from_data(&data);

        // Store and retrieve
        cache
            .put_validated(content_key, data.clone())
            .await
            .expect("Operation should succeed");
        let retrieved = cache
            .get_validated(content_key)
            .await
            .expect("Operation should succeed");
        assert!(retrieved.is_some());

        let metrics = cache.metrics();
        assert_eq!(metrics.total_validations, 1);
        assert_eq!(metrics.successful_validations, 1);
    }

    #[tokio::test]
    async fn test_blte_block_cache() {
        let config = MemoryCacheConfig::default();
        let inner = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));
        let cache = BlteBlockCache::new(inner, 10);

        let content_key = ContentKey::from_data(b"test content");
        let block_data = Bytes::from("block 0 data");

        // Store and retrieve block
        cache
            .put_block(content_key, 0, block_data.clone(), false)
            .await
            .expect("Operation should succeed");

        let retrieved = cache
            .get_block(content_key, 0, false)
            .await
            .expect("Operation should succeed");
        assert_eq!(retrieved, Some(block_data));

        // Check metadata
        let metadata = cache
            .get_metadata(&content_key)
            .expect("Operation should succeed");
        assert_eq!(metadata.cached_blocks, vec![0]);
    }

    #[tokio::test]
    async fn test_archive_cache() {
        let config = MemoryCacheConfig::default();
        let inner = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));
        let cache = ArchiveCache::new(inner, 10);

        let archive_id = "data.001";
        let data = Bytes::from("archive range data");

        // Store and retrieve range
        cache
            .put_range(archive_id, 1024, 512, data.clone())
            .await
            .expect("Operation should succeed");

        let retrieved = cache
            .get_range(archive_id, 1024, 512)
            .await
            .expect("Operation should succeed");
        assert_eq!(retrieved, Some(data));

        // Check if range is cached
        assert!(cache.is_range_cached(archive_id, 1024, 512));
        assert!(!cache.is_range_cached(archive_id, 2048, 512));

        // Find overlapping ranges
        let overlapping = cache.find_overlapping_ranges(archive_id, 1200, 400);
        assert_eq!(overlapping, vec![(1024, 512)]);
    }

    #[tokio::test]
    async fn test_blte_block_cache_limits() {
        let config = MemoryCacheConfig::default();
        let inner = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));
        let cache = BlteBlockCache::new(inner, 2); // Max 2 blocks per content

        let content_key = ContentKey::from_data(b"test content");

        // Store 2 blocks (within limit)
        cache
            .put_block(content_key, 0, Bytes::from("block 0"), false)
            .await
            .expect("Operation should succeed");
        cache
            .put_block(content_key, 1, Bytes::from("block 1"), false)
            .await
            .expect("Operation should succeed");

        // Try to store 3rd block (should fail)
        let result = cache
            .put_block(content_key, 2, Bytes::from("block 2"), false)
            .await;
        assert!(matches!(result, Err(NgdpCacheError::CacheFull)));
    }

    #[tokio::test]
    async fn test_archive_cache_overlapping_ranges() {
        let config = MemoryCacheConfig::default();
        let inner = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));
        let cache = ArchiveCache::new(inner, 10);

        let archive_id = "data.001";

        // Store multiple ranges
        cache
            .put_range(archive_id, 0, 1024, Bytes::from("range1"))
            .await
            .expect("Operation should succeed");
        cache
            .put_range(archive_id, 1024, 1024, Bytes::from("range2"))
            .await
            .expect("Operation should succeed");
        cache
            .put_range(archive_id, 3072, 1024, Bytes::from("range3"))
            .await
            .expect("Operation should succeed");

        // Find overlapping with a large range
        let overlapping = cache.find_overlapping_ranges(archive_id, 512, 2048);
        assert_eq!(overlapping.len(), 2);
        assert!(overlapping.contains(&(0, 1024)));
        assert!(overlapping.contains(&(1024, 1024)));
    }
}
