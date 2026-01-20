//! CDN path resolution and URL construction
//!
//! Implements the exact CDN URL pattern: http(s)://{cdn_server}/{cdn_path}/{type}/{hash\[0:2\]}/{hash\[2:4\]}/{full_hash}
//! with proper path caching and extraction from CDN responses.

#![allow(clippy::panic)]

#[cfg(feature = "streaming")]
use std::collections::HashMap;
#[cfg(feature = "streaming")]
use std::time::{Duration, Instant};

#[cfg(feature = "streaming")]
use super::{bootstrap::CdnBootstrap, error::StreamingError};

/// CDN content types
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Configuration files (BuildConfig, CDNConfig, etc.)
    Config,
    /// Game content files and archives
    Data,
    /// Differential patch data
    Patch,
}

#[cfg(feature = "streaming")]
impl ContentType {
    /// Get the string representation for URL construction
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Data => "data",
            Self::Patch => "patch",
        }
    }
}

/// CDN path cache entry with timestamp for TTL
#[cfg(feature = "streaming")]
#[derive(Debug, Clone)]
struct CachedPath {
    path: String,
    cached_at: Instant,
}

/// CDN path cache for product-specific paths with runtime updates
#[cfg(feature = "streaming")]
#[derive(Debug, Clone)]
pub struct CdnPathCache {
    /// Cached paths by product name with timestamps
    cache: HashMap<String, CachedPath>,
    /// Time-to-live for cached paths
    ttl: Option<Duration>,
    /// Whether to automatically validate paths
    validate_paths: bool,
}

#[cfg(feature = "streaming")]
impl Default for CdnPathCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "streaming")]
impl CdnPathCache {
    /// Create a new empty path cache
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            ttl: None,
            validate_paths: false,
        }
    }

    /// Create path cache with TTL
    ///
    /// # Arguments
    /// * `ttl` - Time-to-live for cached paths
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl: Some(ttl),
            validate_paths: false,
        }
    }

    /// Create path cache with validation enabled
    pub fn with_validation(validate_paths: bool) -> Self {
        Self {
            cache: HashMap::new(),
            ttl: None,
            validate_paths,
        }
    }

    /// Create path cache with TTL and validation
    pub fn with_ttl_and_validation(ttl: Duration, validate_paths: bool) -> Self {
        Self {
            cache: HashMap::new(),
            ttl: Some(ttl),
            validate_paths,
        }
    }

    /// Get cached path for a product
    ///
    /// Returns None if path is not cached or has expired based on TTL
    pub fn get(&self, product: &str) -> Option<&str> {
        let cached_entry = self.cache.get(product)?;

        // Check TTL if configured
        if let Some(ttl) = self.ttl {
            if cached_entry.cached_at.elapsed() > ttl {
                return None; // Expired
            }
        }

        Some(&cached_entry.path)
    }

    /// Get cached path for a product without TTL check
    ///
    /// Returns the path even if it has expired, useful for fallback scenarios
    pub fn get_without_ttl_check(&self, product: &str) -> Option<&str> {
        self.cache.get(product).map(|entry| entry.path.as_str())
    }

    /// Cache path for a product
    ///
    /// CRITICAL: Path must be extracted from CDN response, never hardcoded
    pub fn set(&mut self, product: String, path: String) {
        let cached_path = CachedPath {
            path,
            cached_at: Instant::now(),
        };
        self.cache.insert(product, cached_path);
    }

    /// Update paths from bootstrap configuration
    ///
    /// Merges paths from bootstrap while preserving existing unexpired paths
    ///
    /// # Arguments
    /// * `bootstrap` - Bootstrap configuration containing new paths
    /// * `force_update` - If true, updates even if existing path is not expired
    pub fn update_from_bootstrap(&mut self, bootstrap: &CdnBootstrap, force_update: bool) {
        for (product, path) in &bootstrap.paths {
            let should_update = force_update || {
                // Update if no existing path or existing path is expired
                match self.cache.get(product) {
                    None => true,
                    Some(cached_entry) => {
                        if let Some(ttl) = self.ttl {
                            cached_entry.cached_at.elapsed() > ttl
                        } else {
                            false // No TTL, don't update unless forced
                        }
                    }
                }
            };

            if should_update {
                self.set(product.clone(), path.clone());
            }
        }
    }

    /// Bulk update paths with timestamps
    ///
    /// # Arguments
    /// * `paths` - Map of product to path
    /// * `replace_all` - If true, replaces entire cache; if false, merges
    pub fn bulk_update(&mut self, paths: HashMap<String, String>, replace_all: bool) {
        if replace_all {
            self.cache.clear();
        }

        for (product, path) in paths {
            self.set(product, path);
        }
    }

    /// Clear all cached paths
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Remove specific product path from cache
    pub fn remove(&mut self, product: &str) -> Option<String> {
        self.cache.remove(product).map(|entry| entry.path)
    }

    /// Remove expired entries from cache
    ///
    /// # Returns
    /// Number of expired entries removed
    pub fn cleanup_expired(&mut self) -> usize {
        let Some(ttl) = self.ttl else {
            return 0; // No TTL configured
        };

        let now = Instant::now();
        let initial_len = self.cache.len();

        self.cache
            .retain(|_, entry| now.duration_since(entry.cached_at) <= ttl);

        initial_len - self.cache.len()
    }

    /// Get all cached products with their paths and cache status
    pub fn entries(&self) -> Vec<(String, String, bool)> {
        let now = Instant::now();

        self.cache
            .iter()
            .map(|(product, entry)| {
                let is_expired = if let Some(ttl) = self.ttl {
                    now.duration_since(entry.cached_at) > ttl
                } else {
                    false
                };

                (product.clone(), entry.path.clone(), !is_expired)
            })
            .collect()
    }

    /// Check if a specific product path is expired
    pub fn is_expired(&self, product: &str) -> bool {
        match (self.cache.get(product), self.ttl) {
            (Some(entry), Some(ttl)) => entry.cached_at.elapsed() > ttl,
            _ => false,
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total = self.cache.len();
        let mut expired = 0;
        let mut valid = 0;

        if let Some(ttl) = self.ttl {
            let now = Instant::now();
            for entry in self.cache.values() {
                if now.duration_since(entry.cached_at) > ttl {
                    expired += 1;
                } else {
                    valid += 1;
                }
            }
        } else {
            valid = total;
        }

        CacheStats {
            total_entries: total,
            valid_entries: valid,
            expired_entries: expired,
            ttl_configured: self.ttl.is_some(),
            validation_enabled: self.validate_paths,
        }
    }

    /// Update cache TTL setting
    pub fn set_ttl(&mut self, ttl: Option<Duration>) {
        self.ttl = ttl;
    }

    /// Enable or disable path validation
    pub fn set_validation(&mut self, validate: bool) {
        self.validate_paths = validate;
    }

    /// Get number of cached paths (including expired)
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Get number of valid (non-expired) cached paths
    pub fn valid_len(&self) -> usize {
        if self.ttl.is_none() {
            return self.cache.len();
        }

        let now = Instant::now();
        let ttl = self.ttl.expect("Operation should succeed");

        self.cache
            .values()
            .filter(|entry| now.duration_since(entry.cached_at) <= ttl)
            .count()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Check if cache has any valid (non-expired) entries
    pub fn has_valid_entries(&self) -> bool {
        self.valid_len() > 0
    }
}

/// CDN URL builder following the exact specification
#[cfg(feature = "streaming")]
#[derive(Debug, Clone)]
pub struct CdnUrlBuilder {
    /// Path cache for discovered CDN paths
    path_cache: CdnPathCache,
}

#[cfg(feature = "streaming")]
impl Default for CdnUrlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "streaming")]
impl CdnUrlBuilder {
    /// Create a new URL builder
    pub fn new() -> Self {
        Self {
            path_cache: CdnPathCache::new(),
        }
    }

    /// Create URL builder with existing path cache
    pub fn with_cache(path_cache: CdnPathCache) -> Self {
        Self { path_cache }
    }

    /// Cache path for a product
    ///
    /// # Arguments
    /// * `product` - Product name (e.g., "wow", "wow_classic")
    /// * `path` - Path extracted from CDN response (e.g., "tpr/wow")
    pub fn cache_path(&mut self, product: String, path: String) {
        self.path_cache.set(product, path);
    }

    /// Get cached path for a product
    pub fn get_cached_path(&self, product: &str) -> Option<&str> {
        self.path_cache.get(product)
    }

    /// Construct CDN URL following the exact specification
    ///
    /// URL Pattern: http(s)://{cdn_server}/{cdn_path}/{type}/{hash\[0:2\]}/{hash\[2:4\]}/{full_hash}
    ///
    /// # Arguments
    /// * `cdn_server` - CDN hostname (e.g., "level3.blizzard.com")
    /// * `cdn_path` - Path from CDN response (e.g., "tpr/wow")
    /// * `content_type` - Type of content (config, data, patch)
    /// * `hash` - Full content hash (32 characters hex)
    /// * `use_https` - Whether to use HTTPS (default true)
    ///
    /// # Returns
    /// Complete CDN URL
    ///
    /// # Errors
    /// Returns error if hash is invalid format
    pub fn build_url(
        &self,
        cdn_server: &str,
        cdn_path: &str,
        content_type: ContentType,
        hash: &str,
        use_https: bool,
    ) -> Result<String, StreamingError> {
        // Validate hash format
        if hash.len() != 32 {
            return Err(StreamingError::InvalidRange {
                reason: format!(
                    "Hash must be 32 characters hex, got {} characters: {}",
                    hash.len(),
                    hash
                ),
            });
        }

        // Verify hash contains only hex characters
        if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StreamingError::InvalidRange {
                reason: format!("Hash must contain only hex characters: {}", hash),
            });
        }

        let protocol = if use_https { "https" } else { "http" };
        let hash_lower = hash.to_lowercase();
        let hash_dir1 = &hash_lower[0..2];
        let hash_dir2 = &hash_lower[2..4];

        let url = format!(
            "{}://{}/{}/{}/{}/{}/{}",
            protocol,
            cdn_server,
            cdn_path,
            content_type.as_str(),
            hash_dir1,
            hash_dir2,
            hash_lower
        );

        Ok(url)
    }

    /// Build URL using cached product path
    ///
    /// # Arguments
    /// * `cdn_server` - CDN hostname
    /// * `product` - Product name to look up cached path
    /// * `content_type` - Type of content
    /// * `hash` - Content hash
    /// * `use_https` - Whether to use HTTPS
    ///
    /// # Returns
    /// Complete CDN URL
    ///
    /// # Errors
    /// Returns error if product path not cached or hash invalid
    pub fn build_url_for_product(
        &self,
        cdn_server: &str,
        product: &str,
        content_type: ContentType,
        hash: &str,
        use_https: bool,
    ) -> Result<String, StreamingError> {
        let cdn_path =
            self.path_cache
                .get(product)
                .ok_or_else(|| StreamingError::Configuration {
                    reason: format!(
                        "No cached CDN path for product '{}'. Must query CDN endpoint first.",
                        product
                    ),
                })?;

        self.build_url(cdn_server, cdn_path, content_type, hash, use_https)
    }

    /// Extract hash directory components from a hash
    ///
    /// Returns (hash\[0:2\], hash\[2:4\]) for directory sharding
    pub fn hash_directories(hash: &str) -> Result<(String, String), StreamingError> {
        if hash.len() < 4 {
            return Err(StreamingError::InvalidRange {
                reason: format!("Hash too short for directory extraction: {}", hash),
            });
        }

        let hash_lower = hash.to_lowercase();
        Ok((hash_lower[0..2].to_string(), hash_lower[2..4].to_string()))
    }

    /// Update URL builder from bootstrap configuration
    ///
    /// # Arguments
    /// * `bootstrap` - Bootstrap configuration with new paths
    /// * `force_update` - Force update even if paths haven't expired
    pub fn update_from_bootstrap(&mut self, bootstrap: &CdnBootstrap, force_update: bool) {
        self.path_cache
            .update_from_bootstrap(bootstrap, force_update);
    }

    /// Clean up expired paths from cache
    ///
    /// # Returns
    /// Number of expired entries removed
    pub fn cleanup_expired(&mut self) -> usize {
        self.path_cache.cleanup_expired()
    }

    /// Get the path cache reference
    pub fn cache(&self) -> &CdnPathCache {
        &self.path_cache
    }

    /// Get mutable path cache reference
    pub fn cache_mut(&mut self) -> &mut CdnPathCache {
        &mut self.path_cache
    }

    /// Create URL builder with runtime configuration support
    ///
    /// # Arguments
    /// * `ttl` - Time-to-live for cached paths
    /// * `validate_paths` - Whether to validate paths
    pub fn with_runtime_config(ttl: Option<Duration>, validate_paths: bool) -> Self {
        let path_cache = if let Some(ttl) = ttl {
            CdnPathCache::with_ttl_and_validation(ttl, validate_paths)
        } else {
            CdnPathCache::with_validation(validate_paths)
        };

        Self { path_cache }
    }
}

/// Special handling for product configuration paths
#[cfg(feature = "streaming")]
impl CdnUrlBuilder {
    /// Build URL for product configuration files
    ///
    /// Product configs use special path: tpr/configs/data
    /// This is different from the regular product path
    pub fn build_product_config_url(
        &self,
        cdn_server: &str,
        hash: &str,
        use_https: bool,
    ) -> Result<String, StreamingError> {
        // Validate hash format
        if hash.len() != 32 {
            return Err(StreamingError::InvalidRange {
                reason: format!(
                    "Hash must be 32 characters hex, got {} characters: {}",
                    hash.len(),
                    hash
                ),
            });
        }

        // Verify hash contains only hex characters
        if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StreamingError::InvalidRange {
                reason: format!("Hash must contain only hex characters: {}", hash),
            });
        }

        let protocol = if use_https { "https" } else { "http" };
        let hash_lower = hash.to_lowercase();
        let hash_dir1 = &hash_lower[0..2];
        let hash_dir2 = &hash_lower[2..4];

        // Product configs don't use content type in the path
        // because "tpr/configs/data" already includes the data segment
        let url = format!(
            "{}://{}/tpr/configs/data/{}/{}/{}",
            protocol, cdn_server, hash_dir1, hash_dir2, hash_lower
        );

        Ok(url)
    }
}

/// Cache statistics for monitoring and debugging
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq)]
pub struct CacheStats {
    /// Total number of cached entries (including expired)
    pub total_entries: usize,
    /// Number of valid (non-expired) entries
    pub valid_entries: usize,
    /// Number of expired entries
    pub expired_entries: usize,
    /// Whether TTL is configured for this cache
    pub ttl_configured: bool,
    /// Whether path validation is enabled
    pub validation_enabled: bool,
}

#[cfg(all(test, feature = "streaming"))]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::uninlined_format_args)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_string_representation() {
        assert_eq!(ContentType::Config.as_str(), "config");
        assert_eq!(ContentType::Data.as_str(), "data");
        assert_eq!(ContentType::Patch.as_str(), "patch");
    }

    #[test]
    fn test_path_cache_operations() {
        let mut cache = CdnPathCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.valid_len(), 0);
        assert!(!cache.has_valid_entries());

        cache.set("wow".to_string(), "tpr/wow".to_string());
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.valid_len(), 1);
        assert!(cache.has_valid_entries());
        assert_eq!(cache.get("wow"), Some("tpr/wow"));

        let removed = cache.remove("wow");
        assert_eq!(removed, Some("tpr/wow".to_string()));
        assert!(cache.is_empty());
    }

    #[test]
    fn test_url_construction() {
        let builder = CdnUrlBuilder::new();

        let url = builder
            .build_url(
                "level3.blizzard.com",
                "tpr/wow",
                ContentType::Config,
                "1234567890abcdef1234567890abcdef",
                true,
            )
            .expect("Operation should succeed");

        assert_eq!(
            url,
            "https://level3.blizzard.com/tpr/wow/config/12/34/1234567890abcdef1234567890abcdef"
        );
    }

    #[test]
    fn test_url_construction_http() {
        let builder = CdnUrlBuilder::new();

        let url = builder
            .build_url(
                "cdn.arctium.tools",
                "tpr/wow",
                ContentType::Data,
                "abcdef1234567890abcdef1234567890",
                false,
            )
            .expect("Operation should succeed");

        assert_eq!(
            url,
            "http://cdn.arctium.tools/tpr/wow/data/ab/cd/abcdef1234567890abcdef1234567890"
        );
    }

    #[test]
    fn test_real_world_examples() {
        let builder = CdnUrlBuilder::new();

        // Build configuration example from docs
        let build_config = builder
            .build_url(
                "cdn.arctium.tools",
                "tpr/wow",
                ContentType::Config,
                "ae66faee0ac786fdd7d8b4cf90a8d5b9",
                false,
            )
            .expect("Operation should succeed");

        assert_eq!(
            build_config,
            "http://cdn.arctium.tools/tpr/wow/config/ae/66/ae66faee0ac786fdd7d8b4cf90a8d5b9"
        );

        // Encoding file example (data type)
        let encoding_file = builder
            .build_url(
                "cdn.arctium.tools",
                "tpr/wow",
                ContentType::Data,
                "bbf06e7476382cfaa396cff0049d356b",
                false,
            )
            .expect("Operation should succeed");

        assert_eq!(
            encoding_file,
            "http://cdn.arctium.tools/tpr/wow/data/bb/f0/bbf06e7476382cfaa396cff0049d356b"
        );
    }

    #[test]
    fn test_product_config_url() {
        let builder = CdnUrlBuilder::new();

        let url = builder
            .build_product_config_url(
                "cdn.arctium.tools",
                "c9934edfc8f217a2e01c47e4deae8454",
                false,
            )
            .expect("Operation should succeed");

        assert_eq!(
            url,
            "http://cdn.arctium.tools/tpr/configs/data/c9/93/c9934edfc8f217a2e01c47e4deae8454"
        );
    }

    #[test]
    fn test_url_construction_with_cached_path() {
        let mut builder = CdnUrlBuilder::new();
        builder.cache_path("wow_classic".to_string(), "tpr/wow".to_string());

        let url = builder
            .build_url_for_product(
                "level3.blizzard.com",
                "wow_classic",
                ContentType::Data,
                "1234567890abcdef1234567890abcdef",
                true,
            )
            .expect("Operation should succeed");

        assert_eq!(
            url,
            "https://level3.blizzard.com/tpr/wow/data/12/34/1234567890abcdef1234567890abcdef"
        );
    }

    #[test]
    fn test_invalid_hash_validation() {
        let builder = CdnUrlBuilder::new();

        // Too short hash
        let result = builder.build_url("example.com", "tpr/wow", ContentType::Data, "short", true);
        assert!(result.is_err());

        // Non-hex characters
        let result = builder.build_url(
            "example.com",
            "tpr/wow",
            ContentType::Data,
            "ghijklmnopqrstuvwxyz123456789012",
            true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_directory_extraction() {
        let (dir1, dir2) =
            CdnUrlBuilder::hash_directories("1234567890abcdef").expect("Operation should succeed");
        assert_eq!(dir1, "12");
        assert_eq!(dir2, "34");

        let (dir1, dir2) =
            CdnUrlBuilder::hash_directories("ABCDEF1234567890").expect("Operation should succeed");
        assert_eq!(dir1, "ab");
        assert_eq!(dir2, "cd");

        // Test error cases
        let result = CdnUrlBuilder::hash_directories("123");
        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::panic)]
    fn test_missing_cached_path() {
        let builder = CdnUrlBuilder::new();

        let result = builder.build_url_for_product(
            "example.com",
            "unknown_product",
            ContentType::Data,
            "1234567890abcdef1234567890abcdef",
            true,
        );

        assert!(result.is_err());
        if let Err(StreamingError::Configuration { reason }) = result {
            assert!(reason.contains("No cached CDN path"));
        } else {
            unreachable!("Expected Configuration error");
        }
    }

    #[test]
    fn test_builder_with_existing_cache() {
        let mut cache = CdnPathCache::new();
        cache.set("wow".to_string(), "tpr/wow".to_string());

        let builder = CdnUrlBuilder::with_cache(cache);
        assert_eq!(builder.get_cached_path("wow"), Some("tpr/wow"));
    }

    #[test]
    fn test_cache_mutations() {
        let mut builder = CdnUrlBuilder::new();

        // Test mutable cache access
        builder
            .cache_mut()
            .set("test".to_string(), "tpr/test".to_string());
        assert_eq!(builder.cache().get("test"), Some("tpr/test"));

        // Test cache clearing
        builder.cache_mut().clear();
        assert!(builder.cache().is_empty());
    }

    #[test]
    fn test_path_cache_with_ttl() {
        let ttl = Duration::from_millis(100);
        let mut cache = CdnPathCache::with_ttl(ttl);

        cache.set("wow".to_string(), "tpr/wow".to_string());
        assert_eq!(cache.get("wow"), Some("tpr/wow"));

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(cache.get("wow"), None); // Should be expired

        // But still available without TTL check
        assert_eq!(cache.get_without_ttl_check("wow"), Some("tpr/wow"));
    }

    #[test]
    fn test_cache_cleanup_expired() {
        let ttl = Duration::from_millis(50);
        let mut cache = CdnPathCache::with_ttl(ttl);

        cache.set("wow".to_string(), "tpr/wow".to_string());
        cache.set("wowt".to_string(), "tpr/wowt".to_string());
        assert_eq!(cache.len(), 2);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(100));

        let removed = cache.cleanup_expired();
        assert_eq!(removed, 2);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_bootstrap_update() {
        let mut cache = CdnPathCache::new();
        cache.set("wow".to_string(), "old/path".to_string());

        let mut bootstrap = CdnBootstrap::new();
        bootstrap
            .paths
            .insert("wow".to_string(), "new/path".to_string());
        bootstrap
            .paths
            .insert("wowt".to_string(), "tpr/wowt".to_string());

        // Update without force - should not update existing path
        cache.update_from_bootstrap(&bootstrap, false);
        assert_eq!(cache.get("wow"), Some("old/path"));
        assert_eq!(cache.get("wowt"), Some("tpr/wowt"));

        // Update with force - should update existing path
        cache.update_from_bootstrap(&bootstrap, true);
        assert_eq!(cache.get("wow"), Some("new/path"));
        assert_eq!(cache.get("wowt"), Some("tpr/wowt"));
    }

    #[test]
    fn test_cache_stats() {
        let ttl = Duration::from_millis(100);
        let mut cache = CdnPathCache::with_ttl_and_validation(ttl, true);

        cache.set("wow".to_string(), "tpr/wow".to_string());

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.valid_entries, 1);
        assert_eq!(stats.expired_entries, 0);
        assert!(stats.ttl_configured);
        assert!(stats.validation_enabled);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.valid_entries, 0);
        assert_eq!(stats.expired_entries, 1);
    }

    #[test]
    fn test_url_builder_runtime_config() {
        let ttl = Duration::from_secs(3600);
        let mut builder = CdnUrlBuilder::with_runtime_config(Some(ttl), true);

        // Test bootstrap update
        let mut bootstrap = CdnBootstrap::new();
        bootstrap
            .paths
            .insert("wow".to_string(), "tpr/wow".to_string());

        builder.update_from_bootstrap(&bootstrap, false);
        assert_eq!(builder.get_cached_path("wow"), Some("tpr/wow"));

        // Test cleanup
        let removed = builder.cleanup_expired();
        assert_eq!(removed, 0); // Nothing should be expired yet
    }
}
