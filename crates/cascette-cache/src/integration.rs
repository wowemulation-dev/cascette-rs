//! Format integration helpers for NGDP-aware cache operations
//!
//! This module provides helper functions that integrate the cache system
//! with cascette-formats and cascette-crypto for NGDP/CASC-specific operations.
//! It enables caching and validation of NGDP content with automatic parsing.
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss

use crate::{
    error::NgdpCacheResult,
    key::{ArchiveRangeKey, BlteBlockKey, EncodingFileKey, RootFileKey},
    traits::AsyncCache,
    validation::ValidationHooks,
};
use bytes::Bytes;
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::{encoding::EncodingFile, root::RootFile};

/// Configuration for format parsing operations
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// Maximum root file size to parse (default: 32MB)
    pub max_root_size: usize,
    /// Maximum encoding file size to parse in memory (default: 100MB)
    pub max_encoding_size: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            max_root_size: 32 * 1024 * 1024,
            max_encoding_size: 100 * 1024 * 1024,
        }
    }
}

/// Helper functions for root file operations with automatic parsing and validation
pub struct RootFileOps;

impl RootFileOps {
    /// Get and parse a root file from cache with validation
    pub async fn get_parsed_root<C, V>(
        cache: &C,
        validation: &V,
        content_key: ContentKey,
        config: &FormatConfig,
    ) -> NgdpCacheResult<Option<RootFile>>
    where
        C: AsyncCache<RootFileKey>,
        V: ValidationHooks,
    {
        let raw_key = RootFileKey::new_raw(content_key);
        if let Some(raw_data) = cache.get(&raw_key).await? {
            let validation_result = validation.validate_content(&content_key, &raw_data).await?;
            if !validation_result.is_valid {
                return Err(crate::error::NgdpCacheError::ContentValidationFailed(
                    content_key,
                ));
            }
            let root_file = Self::parse_root_file(&raw_data, config)?;
            return Ok(Some(root_file));
        }
        Ok(None)
    }

    /// Store raw root file data in cache
    pub async fn put_raw_root<C>(
        cache: &C,
        content_key: ContentKey,
        raw_data: Bytes,
    ) -> NgdpCacheResult<()>
    where
        C: AsyncCache<RootFileKey>,
    {
        let raw_key = RootFileKey::new_raw(content_key);
        cache.put(raw_key, raw_data).await.map_err(Into::into)
    }

    /// Get raw root file data with validation
    pub async fn get_raw_root<C, V>(
        cache: &C,
        validation: &V,
        content_key: ContentKey,
    ) -> NgdpCacheResult<Option<Bytes>>
    where
        C: AsyncCache<RootFileKey>,
        V: ValidationHooks,
    {
        let raw_key = RootFileKey::new_raw(content_key);
        if let Some(raw_data) = cache.get(&raw_key).await? {
            let validation_result = validation.validate_content(&content_key, &raw_data).await?;
            if !validation_result.is_valid {
                return Err(crate::error::NgdpCacheError::ContentValidationFailed(
                    content_key,
                ));
            }
            Ok(Some(raw_data))
        } else {
            Ok(None)
        }
    }

    /// Parse root file data with size validation
    fn parse_root_file(data: &[u8], config: &FormatConfig) -> NgdpCacheResult<RootFile> {
        if data.len() > config.max_root_size {
            return Err(crate::error::NgdpCacheError::ParseFailed(format!(
                "Root file too large: {} bytes (max: {})",
                data.len(),
                config.max_root_size
            )));
        }

        RootFile::parse(data).map_err(|e| {
            crate::error::NgdpCacheError::ParseFailed(format!("Root file parse failed: {e}"))
        })
    }
}

/// Helper functions for encoding file operations with automatic parsing and validation
pub struct EncodingFileOps;

impl EncodingFileOps {
    /// Get and parse an encoding file from cache with validation
    pub async fn get_parsed_encoding<C, V>(
        cache: &C,
        validation: &V,
        encoding_key: EncodingKey,
        page: Option<u32>,
        config: &FormatConfig,
    ) -> NgdpCacheResult<Option<EncodingFile>>
    where
        C: AsyncCache<EncodingFileKey>,
        V: ValidationHooks,
    {
        let raw_key = match page {
            Some(p) => EncodingFileKey::with_page(encoding_key, p, false),
            None => EncodingFileKey::new_raw(encoding_key),
        };

        if let Some(raw_data) = cache.get(&raw_key).await? {
            // For encoding files, we derive content key from encoding key
            let content_key = Self::content_key_from_encoding(&encoding_key);
            let validation_result = validation.validate_content(&content_key, &raw_data).await?;
            if !validation_result.is_valid {
                return Err(crate::error::NgdpCacheError::ContentValidationFailed(
                    content_key,
                ));
            }
            let encoding_file = Self::parse_encoding_file(&raw_data, config)?;
            return Ok(Some(encoding_file));
        }

        Ok(None)
    }

    /// Store raw encoding file data in cache
    pub async fn put_raw_encoding<C>(
        cache: &C,
        encoding_key: EncodingKey,
        raw_data: Bytes,
        page: Option<u32>,
    ) -> NgdpCacheResult<()>
    where
        C: AsyncCache<EncodingFileKey>,
    {
        let raw_key = match page {
            Some(p) => EncodingFileKey::with_page(encoding_key, p, false),
            None => EncodingFileKey::new_raw(encoding_key),
        };
        cache.put(raw_key, raw_data).await.map_err(Into::into)
    }

    /// Parse encoding file data with size validation
    fn parse_encoding_file(data: &[u8], config: &FormatConfig) -> NgdpCacheResult<EncodingFile> {
        if data.len() > config.max_encoding_size {
            return Err(crate::error::NgdpCacheError::ParseFailed(format!(
                "Encoding file too large: {} bytes (max: {})",
                data.len(),
                config.max_encoding_size
            )));
        }

        EncodingFile::parse(data).map_err(|e| {
            crate::error::NgdpCacheError::ParseFailed(format!("Encoding file parse failed: {e}"))
        })
    }

    /// Helper to generate content key from encoding key
    fn content_key_from_encoding(encoding_key: &EncodingKey) -> ContentKey {
        // This is a placeholder - actual implementation would depend on NGDP spec
        ContentKey::from_data(encoding_key.to_string().as_bytes())
    }
}

/// Helper functions for archive operations
pub struct ArchiveOps;

impl ArchiveOps {
    /// Get archive range data from cache
    pub async fn get_archive_range<C>(
        cache: &C,
        archive_id: &str,
        start_offset: u64,
        length: u32,
    ) -> NgdpCacheResult<Option<Bytes>>
    where
        C: AsyncCache<ArchiveRangeKey>,
    {
        let range_key = ArchiveRangeKey::new(archive_id, start_offset, length);
        cache.get(&range_key).await.map_err(Into::into)
    }

    /// Store archive range data in cache
    pub async fn put_archive_range<C>(
        cache: &C,
        archive_id: &str,
        start_offset: u64,
        length: u32,
        data: Bytes,
    ) -> NgdpCacheResult<()>
    where
        C: AsyncCache<ArchiveRangeKey>,
    {
        let range_key = ArchiveRangeKey::new(archive_id, start_offset, length);
        cache.put(range_key, data).await.map_err(Into::into)
    }
}

/// Helper functions for BLTE block operations
pub struct BlteBlockOps;

impl BlteBlockOps {
    /// Get BLTE block data from cache (raw or decompressed)
    pub async fn get_blte_block<C>(
        cache: &C,
        content_key: ContentKey,
        block_index: u32,
        decompressed: bool,
    ) -> NgdpCacheResult<Option<Bytes>>
    where
        C: AsyncCache<BlteBlockKey>,
    {
        let block_key = if decompressed {
            BlteBlockKey::new_decompressed(content_key, block_index)
        } else {
            BlteBlockKey::new_raw(content_key, block_index)
        };

        cache.get(&block_key).await.map_err(Into::into)
    }

    /// Store BLTE block data in cache
    pub async fn put_blte_block<C>(
        cache: &C,
        content_key: ContentKey,
        block_index: u32,
        data: Bytes,
        decompressed: bool,
    ) -> NgdpCacheResult<()>
    where
        C: AsyncCache<BlteBlockKey>,
    {
        let block_key = if decompressed {
            BlteBlockKey::new_decompressed(content_key, block_index)
        } else {
            BlteBlockKey::new_raw(content_key, block_index)
        };

        cache.put(block_key, data).await.map_err(Into::into)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        config::MemoryCacheConfig, memory_cache::MemoryCache, validation::NoOpValidationHooks,
    };

    #[tokio::test]
    async fn test_archive_operations() {
        let config = MemoryCacheConfig::default();
        let cache = MemoryCache::new(config).expect("Operation should succeed");

        let test_data = Bytes::from("test archive data");

        // Store and retrieve archive range
        ArchiveOps::put_archive_range(&cache, "test.001", 1024, 512, test_data.clone())
            .await
            .expect("Operation should succeed");

        let retrieved = ArchiveOps::get_archive_range(&cache, "test.001", 1024, 512)
            .await
            .expect("Operation should succeed")
            .expect("Operation should succeed");

        assert_eq!(retrieved, test_data);
    }

    #[tokio::test]
    async fn test_blte_block_operations() {
        let config = MemoryCacheConfig::default();
        let cache = MemoryCache::new(config).expect("Operation should succeed");

        let content_key = ContentKey::from_data(b"test content");
        let test_data = Bytes::from("compressed block data");

        // Store raw block
        BlteBlockOps::put_blte_block(&cache, content_key, 0, test_data.clone(), false)
            .await
            .expect("Operation should succeed");

        // Retrieve raw block
        let retrieved = BlteBlockOps::get_blte_block(&cache, content_key, 0, false)
            .await
            .expect("Operation should succeed")
            .expect("Operation should succeed");

        assert_eq!(retrieved, test_data);

        // Test decompressed block doesn't exist
        let decompressed = BlteBlockOps::get_blte_block(&cache, content_key, 0, true)
            .await
            .expect("Operation should succeed");
        assert!(decompressed.is_none());
    }

    #[tokio::test]
    async fn test_root_file_operations() {
        let config = MemoryCacheConfig::default();
        let cache = MemoryCache::new(config).expect("Operation should succeed");
        let validation = NoOpValidationHooks;

        let content_key = ContentKey::from_data(b"test root content");

        // Test that getting non-existent root file returns None
        let result = RootFileOps::get_raw_root(&cache, &validation, content_key)
            .await
            .expect("Operation should succeed");
        assert!(result.is_none());
    }
}
