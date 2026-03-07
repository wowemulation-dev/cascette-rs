//! High-level integration layer for complete CDN streaming operations
//!
//! This module provides the `StreamingCdnResolver` that combines CDN index lookup,
//! streaming archive reading, and HTTP range requests into a unified API for
//! content resolution and extraction.

use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

/// Simple cancellation token implementation
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Create a new cancellation token
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Cancel the operation
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if the operation has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Wait until the operation is cancelled
    pub async fn cancelled(&self) {
        // Simple polling approach - check every 10ms
        while !self.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

use super::super::{ArchiveError, ArchiveIndex};
use crate::cdn::streaming::{
    HttpClient, StreamingConfig, StreamingError,
    archive::{ArchiveExtractionRequest, StreamingArchiveConfig, StreamingArchiveReader},
    error::InputValidator,
    path::{CdnUrlBuilder, ContentType},
};
use cascette_crypto::{TactKeyProvider, TactKeyStore};
use cascette_formats::archive::ArchiveGroup;

use tracing::{debug, info};

/// Content resolution request
#[derive(Debug, Clone)]
pub struct ContentResolutionRequest {
    /// Encoding key of the content to resolve
    pub encoding_key: Vec<u8>,
    /// Expected content size for verification
    pub expected_size: Option<u32>,
    /// Whether to decompress BLTE content
    pub decompress: bool,
}

/// Result of content resolution
#[derive(Debug)]
pub struct ContentResolutionResult {
    /// The resolved content
    pub content: Vec<u8>,
    /// Size of the content
    pub size: usize,
    /// Archive where the content was found
    pub archive_url: String,
    /// Offset within the archive
    pub archive_offset: u64,
    /// Whether the content was decompressed
    pub was_decompressed: bool,
}

/// CDN configuration for content resolution
#[derive(Debug, Clone)]
pub struct CdnResolutionConfig {
    /// Product name (e.g., "wow", "wow_classic")
    pub product: String,
    /// CDN path (e.g., "tpr/wow")
    pub cdn_path: String,
    /// CDN host to use for requests
    pub cdn_host: String,
    /// Archive configuration
    pub archive_config: StreamingArchiveConfig,
    /// Streaming configuration
    pub streaming_config: StreamingConfig,
    /// Whether to prefer HTTPS when available
    pub prefer_https: bool,
}

impl Default for CdnResolutionConfig {
    fn default() -> Self {
        Self {
            product: "wow".to_string(),
            cdn_path: "tpr/wow".to_string(),
            cdn_host: "level3.blizzard.com".to_string(),
            archive_config: StreamingArchiveConfig::default(),
            streaming_config: StreamingConfig::default(),
            prefer_https: true,
        }
    }
}

impl CdnResolutionConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), StreamingError> {
        // Validate CDN host
        InputValidator::validate_hostname(&self.cdn_host)?;

        // Validate product name
        if self.product.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "Product name cannot be empty".to_string(),
            });
        }

        if self.product.len() > 50 {
            return Err(StreamingError::Configuration {
                reason: "Product name too long".to_string(),
            });
        }

        // Validate CDN path
        if self.cdn_path.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "CDN path cannot be empty".to_string(),
            });
        }

        if self.cdn_path.len() > 100 {
            return Err(StreamingError::Configuration {
                reason: "CDN path too long".to_string(),
            });
        }

        // Validate that the product and path contain only safe characters
        if !self
            .product
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(StreamingError::Configuration {
                reason: "Product name contains invalid characters".to_string(),
            });
        }

        if !self
            .cdn_path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '_' || c == '-')
        {
            return Err(StreamingError::Configuration {
                reason: "CDN path contains invalid characters".to_string(),
            });
        }

        Ok(())
    }

    /// Create a new validated configuration
    pub fn new(
        product: String,
        cdn_path: String,
        cdn_host: String,
        prefer_https: bool,
    ) -> Result<Self, StreamingError> {
        let config = Self {
            product,
            cdn_path,
            cdn_host,
            archive_config: StreamingArchiveConfig::default(),
            streaming_config: StreamingConfig::default(),
            prefer_https,
        };

        config.validate()?;
        Ok(config)
    }
}

/// High-level CDN streaming resolver
#[derive(Debug)]
pub struct StreamingCdnResolver<H: HttpClient> {
    http_client: H,
    config: CdnResolutionConfig,
    url_builder: CdnUrlBuilder,
    archive_reader: StreamingArchiveReader<H>,
    // Cached CDN indices for performance
    cached_indices: HashMap<String, Arc<ArchiveIndex>>,
    // Cached archive group indices
    cached_archive_groups: HashMap<String, Arc<ArchiveIndex>>,
    /// Ordered archive hashes from CDN config (position = archive_index)
    archive_hashes: Vec<String>,
    /// Archive group index for encoding key -> archive routing
    archive_group: Option<Arc<ArchiveGroup>>,
}

impl<H: HttpClient + Clone> StreamingCdnResolver<H> {
    /// Create a new CDN resolver
    pub fn new(http_client: H, config: CdnResolutionConfig) -> Self {
        let archive_reader = StreamingArchiveReader::new(
            http_client.clone(),
            config.archive_config.clone(),
            config.streaming_config.clone(),
        );

        let mut url_builder = CdnUrlBuilder::new();
        url_builder.cache_path(config.product.clone(), config.cdn_path.clone());

        Self {
            http_client,
            config,
            url_builder,
            archive_reader,
            cached_indices: HashMap::new(),
            cached_archive_groups: HashMap::new(),
            archive_hashes: Vec::new(),
            archive_group: None,
        }
    }

    /// Create resolver with default configuration
    #[must_use = "Resolver must be used to access CDN content"]
    pub fn with_defaults(http_client: H) -> Self {
        Self::new(http_client, CdnResolutionConfig::default())
    }

    /// Create resolver with specific CDN host (with validation)
    #[must_use = "Resolver must be used to access CDN content"]
    pub fn with_cdn_host(http_client: H, cdn_host: String) -> Result<Self, StreamingError> {
        // Validate the CDN host
        InputValidator::validate_hostname(&cdn_host)?;

        let config = CdnResolutionConfig {
            cdn_host,
            ..Default::default()
        };
        Ok(Self::new(http_client, config))
    }

    /// Resolve single content by encoding key
    ///
    /// This method handles the complete resolution flow:
    /// 1. Load and cache CDN indices as needed
    /// 2. Look up content location in indices
    /// 3. Extract content using streaming archive reader
    /// 4. Decompress BLTE content if requested
    ///
    /// # Arguments
    /// * `encoding_key` - The encoding key of the content to resolve
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Resolved and optionally decompressed content
    pub async fn resolve_content(
        &mut self,
        encoding_key: &[u8],
        key_store: Option<&TactKeyStore>,
    ) -> Result<ContentResolutionResult, StreamingError> {
        let request = ContentResolutionRequest {
            encoding_key: encoding_key.to_vec(),
            expected_size: None,
            decompress: true,
        };

        let mut results = self.resolve_multiple(vec![request], key_store).await?;

        results.remove(encoding_key).ok_or_else(|| {
            let context = format!("Resolving content with key: {}", hex::encode(encoding_key));
            StreamingError::archive_format_with_context(
                ArchiveError::InvalidFormat("Content not found".to_string()),
                &context,
            )
        })
    }

    /// Resolve multiple content pieces efficiently
    ///
    /// # Arguments
    /// * `requests` - List of content resolution requests
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Map of encoding keys to resolved content
    pub async fn resolve_multiple(
        &mut self,
        requests: Vec<ContentResolutionRequest>,
        key_store: Option<&TactKeyStore>,
    ) -> Result<HashMap<Vec<u8>, ContentResolutionResult>, StreamingError> {
        if requests.is_empty() {
            return Ok(HashMap::new());
        }

        // Group requests by archive using archive group index
        let archive_groups = self.group_requests_by_archive(&requests);

        let mut final_results = HashMap::new();

        // Process each archive group sequentially to avoid borrow checker issues
        for (archive_hash, grouped_requests) in archive_groups {
            let archive_url = self.build_archive_url(&archive_hash)?;
            let index = self.get_archive_index(&archive_hash).await?;

            // Convert requests to archive extraction requests
            let extraction_requests: Vec<ArchiveExtractionRequest> = grouped_requests
                .into_iter()
                .map(|req| ArchiveExtractionRequest {
                    encoding_key: req.encoding_key,
                    expected_size: req.expected_size,
                    is_blte: req.decompress,
                })
                .collect();

            // Extract content from this archive
            let archive_results = self
                .archive_reader
                .extract_multiple(
                    &archive_url,
                    extraction_requests,
                    &index,
                    key_store.map(|ks| ks as &dyn TactKeyProvider),
                )
                .await?;

            // Convert to final result format
            for (encoding_key, extraction_result) in archive_results {
                let content_result = ContentResolutionResult {
                    content: extraction_result.content,
                    size: extraction_result.size,
                    archive_url: archive_url.clone(),
                    archive_offset: extraction_result.archive_offset,
                    was_decompressed: extraction_result.was_compressed,
                };

                final_results.insert(encoding_key, content_result);
            }
        }

        Ok(final_results)
    }

    /// Resolve content from specific archive
    ///
    /// # Arguments
    /// * `archive_hash` - Hash of the archive to search
    /// * `encoding_key` - Encoding key of the content
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Resolved content from the specified archive
    pub async fn resolve_from_archive(
        &mut self,
        archive_hash: &str,
        encoding_key: &[u8],
        key_store: Option<&TactKeyStore>,
    ) -> Result<ContentResolutionResult, StreamingError> {
        let archive_url = self.build_archive_url(archive_hash)?;
        let index = self.get_archive_index(archive_hash).await?;

        let extraction_result = self
            .archive_reader
            .extract_by_key(
                &archive_url,
                encoding_key,
                &index,
                key_store.map(|ks| ks as &dyn TactKeyProvider),
            )
            .await?;

        Ok(ContentResolutionResult {
            content: extraction_result.content,
            size: extraction_result.size,
            archive_url,
            archive_offset: extraction_result.archive_offset,
            was_decompressed: extraction_result.was_compressed,
        })
    }

    /// Load and cache CDN index for an archive
    async fn get_archive_index(
        &mut self,
        archive_hash: &str,
    ) -> Result<Arc<ArchiveIndex>, StreamingError> {
        // Check cache first
        if let Some(cached_index) = self.cached_indices.get(archive_hash) {
            return Ok(cached_index.clone());
        }

        // Validate archive hash before building URL
        InputValidator::validate_content_hash(archive_hash)?;

        // Build index URL
        let index_url = self.url_builder.build_url_for_product(
            &self.config.cdn_host,
            &self.config.product,
            ContentType::Data, // Archives use data path
            &format!("{archive_hash}.index"),
            self.config.prefer_https,
        )?;

        // Validate the final URL for security
        InputValidator::validate_url(&index_url)?;

        // Download and parse index
        let index_data = self
            .http_client
            .get_range(&index_url, None)
            .await
            .map_err(|e| {
                // Add context to existing streaming error
                match e {
                    StreamingError::NetworkRequest { source } => {
                        StreamingError::network_with_context(
                            source,
                            &format!("Downloading index for archive: {archive_hash}"),
                        )
                    }
                    _ => e,
                }
            })?;

        // Parse index from bytes
        let mut cursor = Cursor::new(index_data.as_ref());
        let index = ArchiveIndex::parse(&mut cursor).map_err(|e| {
            let context = format!("Parsing index for archive: {archive_hash}");
            StreamingError::archive_format_with_context(e, &context)
        })?;

        let arc_index = Arc::new(index);
        self.cached_indices
            .insert(archive_hash.to_string(), arc_index.clone());

        Ok(arc_index)
    }

    /// Load archive hashes and an optional pre-built archive group index.
    ///
    /// The archive hashes are the ordered list from `CdnConfig::archives()`.
    /// The positional index of each hash corresponds to the `archive_index`
    /// in archive group entries.
    ///
    /// If `group` is None, the resolver falls back to the first archive hash
    /// for all requests.
    pub fn set_archive_group(&mut self, archive_hashes: Vec<String>, group: Option<ArchiveGroup>) {
        self.archive_hashes = archive_hashes;
        self.archive_group = group.map(Arc::new);
    }

    /// Group requests by archive using archive group index
    fn group_requests_by_archive(
        &self,
        requests: &[ContentResolutionRequest],
    ) -> HashMap<String, Vec<ContentResolutionRequest>> {
        let mut groups: HashMap<String, Vec<ContentResolutionRequest>> = HashMap::new();

        for request in requests {
            let archive_hash = self.resolve_archive_for_key(&request.encoding_key);
            groups
                .entry(archive_hash)
                .or_default()
                .push(request.clone());
        }

        groups
    }

    /// Determine which archive contains the given encoding key.
    ///
    /// Looks up the key in the archive group index to find the archive_index,
    /// then maps it to the corresponding archive hash. Falls back to the first
    /// archive hash if no group index is loaded or the key is not found.
    fn resolve_archive_for_key(&self, encoding_key: &[u8]) -> String {
        if let Some(group) = &self.archive_group
            && let Some(entry) = group.find_entry(encoding_key)
        {
            let idx = entry.archive_index as usize;
            if idx < self.archive_hashes.len() {
                return self.archive_hashes[idx].clone();
            }
        }

        // Fallback: first archive hash or placeholder
        self.archive_hashes
            .first()
            .cloned()
            .unwrap_or_else(|| "default_archive".to_string())
    }

    /// Build archive URL from hash with input validation
    fn build_archive_url(&self, archive_hash: &str) -> Result<String, StreamingError> {
        // Validate the archive hash format
        InputValidator::validate_content_hash(archive_hash)?;

        // Validate CDN host
        InputValidator::validate_hostname(&self.config.cdn_host)?;

        let url = self.url_builder.build_url_for_product(
            &self.config.cdn_host,
            &self.config.product,
            ContentType::Data,
            archive_hash,
            self.config.prefer_https,
        )?;

        // Validate the final URL
        InputValidator::validate_url(&url)?;

        Ok(url)
    }

    /// Update CDN configuration
    pub fn update_config(&mut self, config: CdnResolutionConfig) {
        self.config = config.clone();
        self.url_builder
            .cache_path(config.product.clone(), config.cdn_path.clone());
        self.archive_reader.update_config(config.archive_config);
        // Clear caches when configuration changes
        self.cached_indices.clear();
        self.cached_archive_groups.clear();
        self.archive_hashes.clear();
        self.archive_group = None;
    }

    /// Update CDN host without clearing all caches (with validation)
    pub fn update_cdn_host(&mut self, new_host: String) -> Result<(), StreamingError> {
        // Validate the new hostname
        InputValidator::validate_hostname(&new_host)?;

        if self.config.cdn_host != new_host {
            self.config.cdn_host = new_host;
            // Clear caches since URLs will change
            self.cached_indices.clear();
            self.cached_archive_groups.clear();
            self.archive_hashes.clear();
            self.archive_group = None;
        }

        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &CdnResolutionConfig {
        &self.config
    }

    /// Clear all cached indices (useful for memory management)
    pub fn clear_caches(&mut self) {
        self.cached_indices.clear();
        self.cached_archive_groups.clear();
        self.archive_hashes.clear();
        self.archive_group = None;
    }

    /// Perform resource cleanup and prepare for shutdown
    pub fn prepare_for_shutdown(&mut self) {
        // Clear all caches to free memory
        self.clear_caches();

        // Log resource usage for debugging
        debug!(
            "Resolver shutdown: cleared {} cached indices and {} archive groups",
            self.cached_indices.len(),
            self.cached_archive_groups.len()
        );
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cached_indices_count: self.cached_indices.len(),
            cached_archive_groups_count: self.cached_archive_groups.len(),
        }
    }

    /// Preload indices for better performance with cancellation support
    ///
    /// # Arguments
    /// * `archive_hashes` - List of archive hashes to preload
    /// * `cancellation_token` - Optional cancellation signal
    ///
    /// # Returns
    /// Number of successfully preloaded indices
    pub async fn preload_indices(
        &mut self,
        archive_hashes: Vec<String>,
        cancellation_token: Option<CancellationToken>,
    ) -> Result<usize, StreamingError> {
        let mut tasks = FuturesUnordered::new();

        for archive_hash in archive_hashes {
            if !self.cached_indices.contains_key(&archive_hash) {
                let token = cancellation_token.clone();
                let hash_copy = archive_hash.clone();

                tasks.push(async move {
                    // Check for cancellation
                    if let Some(token) = &token
                        && token.is_cancelled()
                    {
                        return Err(StreamingError::Configuration {
                            reason: "Preload cancelled".to_string(),
                        });
                    }

                    Ok::<String, StreamingError>(hash_copy)
                });
            }
        }

        let mut loaded_count = 0;

        loop {
            let task_result_option = tokio::select! {
                result = tasks.next() => result,
                () = async {
                    if let Some(token) = &cancellation_token {
                        token.cancelled().await;
                    } else {
                        // If no cancellation token, wait forever
                        std::future::pending::<()>().await;
                    }
                } => {
                    // Cancelled - break out of loop
                    None
                }
            };

            let Some(task_result) = task_result_option else {
                break; // Cancellation or no more tasks
            };
            // Check cancellation before processing result
            if let Some(token) = &cancellation_token
                && token.is_cancelled()
            {
                break;
            }

            if let Ok(archive_hash) = task_result {
                if self.get_archive_index(&archive_hash).await.is_ok() {
                    loaded_count += 1;
                }
            } else {
                // Skip failed tasks but continue with others
            }
        }

        Ok(loaded_count)
    }

    /// Preload indices without cancellation (backward compatibility)
    pub async fn preload_indices_simple(
        &mut self,
        archive_hashes: Vec<String>,
    ) -> Result<usize, StreamingError> {
        self.preload_indices(archive_hashes, None).await
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached archive indices
    pub cached_indices_count: usize,
    /// Number of cached archive group indices
    pub cached_archive_groups_count: usize,
}

/// Batch content resolver for handling large-scale operations
#[derive(Debug)]
pub struct BatchContentResolver<H: HttpClient> {
    resolvers: Vec<StreamingCdnResolver<H>>,
    #[allow(dead_code)]
    config: CdnResolutionConfig,
}

impl<H: HttpClient + Clone> BatchContentResolver<H> {
    /// Create batch resolver with multiple CDN resolvers
    pub fn new(http_clients: Vec<H>, config: CdnResolutionConfig) -> Self {
        let resolvers = http_clients
            .into_iter()
            .map(|client| StreamingCdnResolver::new(client, config.clone()))
            .collect();

        Self { resolvers, config }
    }

    /// Resolve content using multiple resolvers in parallel
    ///
    /// # Arguments
    /// * `requests` - List of content resolution requests
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Map of encoding keys to resolved content
    pub async fn resolve_batch(
        &mut self,
        requests: Vec<ContentResolutionRequest>,
        key_store: Option<&TactKeyStore>,
    ) -> Result<HashMap<Vec<u8>, ContentResolutionResult>, StreamingError> {
        if requests.is_empty() {
            return Ok(HashMap::new());
        }

        // Distribute requests across resolvers
        let chunk_size = requests.len().div_ceil(self.resolvers.len());
        let mut tasks = FuturesUnordered::new();

        for (i, resolver) in self.resolvers.iter_mut().enumerate() {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(requests.len());

            if start < requests.len() {
                let chunk_requests = requests[start..end].to_vec();
                tasks.push(
                    async move { resolver.resolve_multiple(chunk_requests, key_store).await },
                );
            }
        }

        // Collect all results
        let mut combined_results = HashMap::new();
        while let Some(task_result) = tasks.next().await {
            let resolver_results = task_result?;
            combined_results.extend(resolver_results);
        }

        Ok(combined_results)
    }

    /// Get the number of available resolvers
    pub fn resolver_count(&self) -> usize {
        self.resolvers.len()
    }

    /// Clear caches on all resolvers
    pub fn clear_all_caches(&mut self) {
        for resolver in &mut self.resolvers {
            resolver.clear_caches();
        }
    }

    /// Prepare all resolvers for shutdown
    pub fn prepare_for_shutdown(&mut self) {
        for resolver in &mut self.resolvers {
            resolver.prepare_for_shutdown();
        }

        info!(
            "Batch resolver shutdown prepared for {} resolvers",
            self.resolvers.len()
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use crate::cdn::streaming::HttpRange;
    use crate::cdn::streaming::config::StreamingConfig;
    use crate::cdn::streaming::http::ReqwestHttpClient;
    use async_trait::async_trait;
    use bytes::Bytes;
    use mockall::mock;

    mock! {
        TestHttpClient {}

        impl Clone for TestHttpClient {
            fn clone(&self) -> Self;
        }

        #[async_trait]
        impl HttpClient for TestHttpClient {
            async fn get_range(&self, url: &str, range: Option<HttpRange>) -> Result<Bytes, StreamingError>;
            async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;
            async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
        }
    }

    #[test]
    fn test_resolution_config_defaults() {
        let config = CdnResolutionConfig::default();
        assert_eq!(config.product, "wow");
        assert_eq!(config.cdn_path, "tpr/wow");
        assert_eq!(config.cdn_host, "level3.blizzard.com");
        assert!(config.prefer_https);
    }

    #[test]
    fn test_content_resolution_request() {
        let request = ContentResolutionRequest {
            encoding_key: vec![1, 2, 3, 4],
            expected_size: Some(1024),
            decompress: true,
        };

        assert_eq!(request.encoding_key, vec![1, 2, 3, 4]);
        assert_eq!(request.expected_size, Some(1024));
        assert!(request.decompress);
    }

    #[test]
    fn test_resolver_creation() {
        let config = StreamingConfig::default();
        let http_client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let _resolver = StreamingCdnResolver::with_defaults(http_client.clone());

        let resolution_config = CdnResolutionConfig::default();
        let _resolver = StreamingCdnResolver::new(http_client, resolution_config);
    }

    #[test]
    fn test_batch_resolver_creation() {
        let config = StreamingConfig::default();
        let http_client1 =
            ReqwestHttpClient::new(config.clone()).expect("Operation should succeed");
        let http_client2 = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let clients = vec![http_client1, http_client2];
        let resolution_config = CdnResolutionConfig::default();

        let batch_resolver = BatchContentResolver::new(clients, resolution_config);
        assert_eq!(batch_resolver.resolver_count(), 2);
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            cached_indices_count: 5,
            cached_archive_groups_count: 2,
        };

        assert_eq!(stats.cached_indices_count, 5);
        assert_eq!(stats.cached_archive_groups_count, 2);
    }

    // Helper to build an ArchiveIndex for testing
    fn make_test_index(entries: &[(&[u8], u64, u32)]) -> ArchiveIndex {
        use cascette_formats::archive::ArchiveIndexBuilder;
        let mut builder = ArchiveIndexBuilder::new();
        for &(key, offset, size) in entries {
            builder.add_entry(key.to_vec(), size, offset);
        }
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        builder.build(&mut cursor).expect("build index")
    }

    // Helper to build an ArchiveGroup from archive indices
    fn make_test_archive_group(archives: &[(u16, &ArchiveIndex)]) -> ArchiveGroup {
        use cascette_formats::archive::ArchiveGroupBuilder;
        let mut builder = ArchiveGroupBuilder::new();
        for &(idx, index) in archives {
            builder.add_archive(idx, index);
        }
        let mut buf = Vec::new();
        builder
            .build(std::io::Cursor::new(&mut buf))
            .expect("build archive group")
    }

    fn make_mock_resolver() -> StreamingCdnResolver<MockTestHttpClient> {
        let mut mock_client = MockTestHttpClient::new();
        mock_client.expect_clone().returning(|| {
            let mut m = MockTestHttpClient::new();
            m.expect_clone().returning(MockTestHttpClient::new);
            m
        });
        StreamingCdnResolver::with_defaults(mock_client)
    }

    #[test]
    fn test_group_requests_without_archive_group() {
        let mut resolver = make_mock_resolver();
        resolver.set_archive_group(vec!["aabbccdd".to_string(), "eeff0011".to_string()], None);

        let requests = vec![
            ContentResolutionRequest {
                encoding_key: vec![1, 2, 3, 4],
                expected_size: None,
                decompress: true,
            },
            ContentResolutionRequest {
                encoding_key: vec![5, 6, 7, 8],
                expected_size: None,
                decompress: true,
            },
        ];

        let groups = resolver.group_requests_by_archive(&requests);
        // All requests should go to the first archive hash
        assert_eq!(groups.len(), 1);
        assert!(groups.contains_key("aabbccdd"));
        assert_eq!(groups["aabbccdd"].len(), 2);
    }

    #[test]
    fn test_group_requests_with_archive_group() {
        let mut resolver = make_mock_resolver();

        let key_a: Vec<u8> = vec![0x10; 16];
        let key_b: Vec<u8> = vec![0x20; 16];

        let idx0 = make_test_index(&[(&key_a, 0, 100)]);
        let idx1 = make_test_index(&[(&key_b, 0, 200)]);
        let group = make_test_archive_group(&[(0, &idx0), (1, &idx1)]);

        resolver.set_archive_group(
            vec!["archive_zero".to_string(), "archive_one".to_string()],
            Some(group),
        );

        let requests = vec![
            ContentResolutionRequest {
                encoding_key: key_a.clone(),
                expected_size: None,
                decompress: true,
            },
            ContentResolutionRequest {
                encoding_key: key_b.clone(),
                expected_size: None,
                decompress: true,
            },
        ];

        let groups = resolver.group_requests_by_archive(&requests);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["archive_zero"].len(), 1);
        assert_eq!(groups["archive_zero"][0].encoding_key, key_a);
        assert_eq!(groups["archive_one"].len(), 1);
        assert_eq!(groups["archive_one"][0].encoding_key, key_b);
    }

    #[test]
    fn test_resolve_archive_for_key_default() {
        let resolver = make_mock_resolver();
        // No archive_hashes, no group — should return "default_archive"
        let result = resolver.resolve_archive_for_key(&[0x01, 0x02]);
        assert_eq!(result, "default_archive");
    }

    #[test]
    fn test_set_archive_group_cleared_on_config_change() {
        let mut resolver = make_mock_resolver();

        let key: Vec<u8> = vec![0x10; 16];
        let idx = make_test_index(&[(&key, 0, 100)]);
        let group = make_test_archive_group(&[(0, &idx)]);

        resolver.set_archive_group(vec!["some_hash".to_string()], Some(group));
        assert!(resolver.archive_group.is_some());
        assert!(!resolver.archive_hashes.is_empty());

        // Config change should clear archive group and hashes
        resolver.update_config(CdnResolutionConfig::default());
        assert!(resolver.archive_group.is_none());
        assert!(resolver.archive_hashes.is_empty());
    }

    #[test]
    fn test_content_resolution_result() {
        let result = ContentResolutionResult {
            content: vec![1, 2, 3, 4],
            size: 4,
            archive_url: "http://example.com/archive.dat".to_string(),
            archive_offset: 1024,
            was_decompressed: true,
        };

        assert_eq!(result.content, vec![1, 2, 3, 4]);
        assert_eq!(result.size, 4);
        assert_eq!(result.archive_url, "http://example.com/archive.dat");
        assert_eq!(result.archive_offset, 1024);
        assert!(result.was_decompressed);
    }
}
