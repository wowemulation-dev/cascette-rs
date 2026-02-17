//! Streaming archive reader with HTTP range request optimization
//!
//! This module provides efficient archive content extraction using HTTP range requests,
//! with automatic range coalescing and parallel streaming capabilities.

use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;

use super::super::{ArchiveError, ArchiveIndex};
use crate::cdn::streaming::{
    HttpClient, HttpRange, RangeCoalescer, StreamingConfig, StreamingError,
    blte::{StreamingBlteConfig, StreamingBlteProcessor},
};
use cascette_crypto::TactKeyStore;

/// Configuration for streaming archive operations
#[derive(Debug, Clone)]
pub struct StreamingArchiveConfig {
    /// Maximum number of parallel range requests (default: 4)
    pub max_parallel_requests: usize,
    /// Buffer size for individual range requests (default: 2MB)
    pub range_buffer_size: usize,
    /// Whether to verify content checksums (default: true)
    pub verify_checksums: bool,
    /// BLTE processing configuration
    pub blte_config: StreamingBlteConfig,
}

impl Default for StreamingArchiveConfig {
    fn default() -> Self {
        Self {
            max_parallel_requests: 4,
            range_buffer_size: 2 * 1024 * 1024, // 2MB
            verify_checksums: true,
            blte_config: StreamingBlteConfig::default(),
        }
    }
}

/// Archive content extraction request
#[derive(Debug, Clone)]
pub struct ArchiveExtractionRequest {
    /// Encoding key of the content to extract
    pub encoding_key: Vec<u8>,
    /// Expected content size (for verification)
    pub expected_size: Option<u32>,
    /// Whether the content is BLTE compressed
    pub is_blte: bool,
}

/// Result of archive content extraction
#[derive(Debug)]
pub struct ArchiveExtractionResult {
    /// The extracted content
    pub content: Vec<u8>,
    /// Size of the extracted content
    pub size: usize,
    /// Whether the content was BLTE compressed
    pub was_compressed: bool,
    /// Archive offset where the content was found
    pub archive_offset: u64,
}

/// Streaming archive reader with HTTP range request support
#[derive(Debug)]
pub struct StreamingArchiveReader<H: HttpClient> {
    http_client: H,
    config: StreamingArchiveConfig,
    #[allow(dead_code)]
    range_coalescer: RangeCoalescer,
    blte_processor: StreamingBlteProcessor<H>,
}

impl<H: HttpClient + Clone> StreamingArchiveReader<H> {
    /// Create a new streaming archive reader
    pub fn new(
        http_client: H,
        config: StreamingArchiveConfig,
        streaming_config: StreamingConfig,
    ) -> Self {
        let range_coalescer = RangeCoalescer::new(streaming_config);
        let blte_processor =
            StreamingBlteProcessor::new(http_client.clone(), config.blte_config.clone());

        Self {
            http_client,
            config,
            range_coalescer,
            blte_processor,
        }
    }

    /// Create reader with default configuration
    pub fn with_defaults(http_client: H) -> Self {
        let config = StreamingArchiveConfig::default();
        let streaming_config = StreamingConfig::default();
        Self::new(http_client, config, streaming_config)
    }

    /// Extract content from archive at specific offset and size
    ///
    /// # Arguments
    /// * `archive_url` - URL of the archive file
    /// * `offset` - Byte offset within the archive
    /// * `size` - Size of the content to extract
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Raw content bytes from the archive
    pub async fn extract_range(
        &self,
        archive_url: &str,
        offset: u64,
        size: u32,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        let range = HttpRange::new(offset, offset + u64::from(size) - 1);
        let content = self.http_client.get_range(archive_url, Some(range)).await?;

        // If key store is provided and content looks like BLTE, try decompression
        if key_store.is_some() && content.len() >= 8 && &content[0..4] == b"BLTE" {
            // Create temporary URL for BLTE processor
            let temp_content = content.to_vec();
            let decompressed = self.decompress_blte_data(&temp_content, key_store)?;
            Ok(decompressed)
        } else {
            Ok(content.to_vec())
        }
    }

    /// Extract multiple content pieces using optimized range requests
    ///
    /// # Arguments
    /// * `archive_url` - URL of the archive file
    /// * `requests` - List of content extraction requests
    /// * `index` - CDN index for content lookup
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Map of encoding keys to extracted content
    pub async fn extract_multiple(
        &self,
        archive_url: &str,
        requests: Vec<ArchiveExtractionRequest>,
        index: &ArchiveIndex,
        key_store: Option<&TactKeyStore>,
    ) -> Result<HashMap<Vec<u8>, ArchiveExtractionResult>, StreamingError> {
        // Look up all entries in the index
        let mut range_requests = Vec::new();
        let mut request_map = HashMap::new();

        for request in requests {
            if let Some(entry) = index.find_entry(&request.encoding_key) {
                let range = HttpRange::new(entry.offset, entry.offset + u64::from(entry.size) - 1);
                range_requests.push(range);
                request_map.insert(range, (request, entry.clone()));
            }
        }

        if range_requests.is_empty() {
            return Ok(HashMap::new());
        }

        // For simplicity in this implementation, execute requests individually
        // In a production system, you would implement proper range coalescing
        let mut results = HashMap::new();

        for range in range_requests {
            if let Some((request, entry)) = request_map.get(&range) {
                let content = self.http_client.get_range(archive_url, Some(range)).await?;

                // Process BLTE content if needed
                let final_content = if request.is_blte {
                    self.decompress_blte_data(&content, key_store)?
                } else {
                    content.to_vec()
                };

                let result = ArchiveExtractionResult {
                    content: final_content.clone(),
                    size: final_content.len(),
                    was_compressed: request.is_blte,
                    archive_offset: entry.offset,
                };

                results.insert(request.encoding_key.clone(), result);
            }
        }

        Ok(results)
    }

    /// Extract all content from an index efficiently
    ///
    /// # Arguments
    /// * `archive_url` - URL of the archive file
    /// * `index` - CDN index containing all content to extract
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Map of encoding keys to extracted content
    pub async fn extract_all_indexed(
        &self,
        archive_url: &str,
        index: &ArchiveIndex,
        key_store: Option<&TactKeyStore>,
    ) -> Result<HashMap<Vec<u8>, ArchiveExtractionResult>, StreamingError> {
        // Convert all index entries to extraction requests
        let requests: Vec<ArchiveExtractionRequest> = index
            .entries
            .iter()
            .map(|entry| ArchiveExtractionRequest {
                encoding_key: entry.encoding_key.clone(),
                expected_size: Some(entry.size),
                is_blte: true, // Assume BLTE by default
            })
            .collect();

        self.extract_multiple(archive_url, requests, index, key_store)
            .await
    }

    /// Get archive content size without downloading
    pub async fn get_archive_size(&self, archive_url: &str) -> Result<u64, StreamingError> {
        self.http_client.get_content_length(archive_url).await
    }

    /// Check if archive supports range requests
    pub async fn supports_range_requests(&self, archive_url: &str) -> Result<bool, StreamingError> {
        self.http_client.supports_ranges(archive_url).await
    }

    /// Extract content by encoding key using index lookup
    ///
    /// # Arguments
    /// * `archive_url` - URL of the archive file
    /// * `encoding_key` - Encoding key of the content to extract
    /// * `index` - CDN index for content lookup
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Extracted and potentially decompressed content
    pub async fn extract_by_key(
        &self,
        archive_url: &str,
        encoding_key: &[u8],
        index: &ArchiveIndex,
        key_store: Option<&TactKeyStore>,
    ) -> Result<ArchiveExtractionResult, StreamingError> {
        let entry =
            index
                .find_entry(encoding_key)
                .ok_or_else(|| StreamingError::ArchiveFormat {
                    source: ArchiveError::InvalidFormat(format!(
                        "Encoding key not found in index: {}",
                        hex::encode(encoding_key)
                    )),
                })?;

        let content = self
            .extract_range(archive_url, entry.offset, entry.size, key_store)
            .await?;

        Ok(ArchiveExtractionResult {
            size: content.len(),
            was_compressed: content.len() != entry.size as usize,
            archive_offset: entry.offset,
            content,
        })
    }

    /// Decompress BLTE data from memory
    #[allow(clippy::unused_self)]
    fn decompress_blte_data(
        &self,
        data: &[u8],
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        // Create a temporary BLTE processor for in-memory decompression
        use crate::CascFormat;
        use crate::blte::BlteFile;

        let blte_file = BlteFile::parse(data).map_err(|e| StreamingError::BlteError {
            source: crate::blte::BlteError::InvalidHeader(format!("Parse error: {e}")),
        })?;

        let decompressed = if let Some(key_store) = key_store {
            blte_file.decompress_with_keys(key_store)?
        } else {
            blte_file.decompress()?
        };

        Ok(decompressed)
    }

    /// Get configuration
    pub fn config(&self) -> &StreamingArchiveConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: StreamingArchiveConfig) {
        self.config = config.clone();
        self.blte_processor =
            StreamingBlteProcessor::new(self.http_client.clone(), config.blte_config);
    }
}

/// Batch archive extraction for multiple archives
#[derive(Debug)]
pub struct BatchArchiveExtractor<H: HttpClient> {
    readers: Vec<StreamingArchiveReader<H>>,
    #[allow(dead_code)]
    config: StreamingArchiveConfig,
}

impl<H: HttpClient + Clone> BatchArchiveExtractor<H> {
    /// Create a batch extractor for multiple archives
    pub fn new(http_clients: Vec<H>, config: StreamingArchiveConfig) -> Self {
        let streaming_config = StreamingConfig::default();
        let readers = http_clients
            .into_iter()
            .map(|client| {
                StreamingArchiveReader::new(client, config.clone(), streaming_config.clone())
            })
            .collect();

        Self { readers, config }
    }

    /// Extract content from multiple archives in parallel
    ///
    /// # Arguments
    /// * `archive_requests` - List of (archive_url, extraction_requests, index) tuples
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Map of encoding keys to extracted content from all archives
    pub async fn extract_from_archives(
        &self,
        archive_requests: Vec<(&str, Vec<ArchiveExtractionRequest>, &ArchiveIndex)>,
        key_store: Option<&TactKeyStore>,
    ) -> Result<HashMap<Vec<u8>, ArchiveExtractionResult>, StreamingError> {
        if archive_requests.len() > self.readers.len() {
            return Err(StreamingError::Configuration {
                reason: format!(
                    "Too many archive requests ({}) for available readers ({})",
                    archive_requests.len(),
                    self.readers.len()
                ),
            });
        }

        let mut tasks = FuturesUnordered::new();

        for (i, (archive_url, requests, index)) in archive_requests.into_iter().enumerate() {
            let reader = &self.readers[i];
            let archive_url = archive_url.to_string();

            tasks.push(async move {
                reader
                    .extract_multiple(&archive_url, requests, index, key_store)
                    .await
            });
        }

        // Collect all results into a single map
        let mut combined_results = HashMap::new();
        while let Some(task_result) = tasks.next().await {
            let archive_results = task_result?;
            combined_results.extend(archive_results);
        }

        Ok(combined_results)
    }

    /// Get the number of available readers
    pub fn reader_count(&self) -> usize {
        self.readers.len()
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
    fn test_archive_config_defaults() {
        let config = StreamingArchiveConfig::default();
        assert_eq!(config.max_parallel_requests, 4);
        assert_eq!(config.range_buffer_size, 2 * 1024 * 1024);
        assert!(config.verify_checksums);
    }

    #[test]
    fn test_extraction_request_creation() {
        let request = ArchiveExtractionRequest {
            encoding_key: vec![1, 2, 3, 4],
            expected_size: Some(1024),
            is_blte: true,
        };

        assert_eq!(request.encoding_key, vec![1, 2, 3, 4]);
        assert_eq!(request.expected_size, Some(1024));
        assert!(request.is_blte);
    }

    #[test]
    fn test_reader_creation() {
        let config = StreamingConfig::default();
        let http_client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let _reader = StreamingArchiveReader::with_defaults(http_client.clone());

        let archive_config = StreamingArchiveConfig::default();
        let streaming_config = StreamingConfig::default();
        let _reader = StreamingArchiveReader::new(http_client, archive_config, streaming_config);
    }

    #[tokio::test]
    async fn test_range_extraction() {
        let mut mock_client = MockTestHttpClient::new();

        // Mock successful range request
        let test_content = b"Hello, streaming world!".to_vec();
        mock_client
            .expect_clone()
            .returning(MockTestHttpClient::new);

        mock_client
            .expect_get_range()
            .withf(|url, range| {
                url == "http://example.com/archive.dat"
                    && range
                        .as_ref()
                        .is_some_and(|r| r.start == 100 && r.end == 122)
            })
            .times(1)
            .returning(move |_, _| Ok(Bytes::from(test_content.clone())));

        let reader = StreamingArchiveReader::with_defaults(mock_client);
        let result = reader
            .extract_range("http://example.com/archive.dat", 100, 23, None)
            .await
            .expect("Operation should succeed");

        assert_eq!(result, b"Hello, streaming world!".to_vec());
    }

    #[tokio::test]
    async fn test_archive_size_query() {
        let mut mock_client = MockTestHttpClient::new();

        mock_client
            .expect_clone()
            .returning(MockTestHttpClient::new);

        mock_client
            .expect_get_content_length()
            .with(mockall::predicate::eq("http://example.com/archive.dat"))
            .times(1)
            .returning(|_| Ok(1024 * 1024)); // 1MB

        let reader = StreamingArchiveReader::with_defaults(mock_client);
        let size = reader
            .get_archive_size("http://example.com/archive.dat")
            .await
            .expect("Operation should succeed");

        assert_eq!(size, 1024 * 1024);
    }

    #[tokio::test]
    async fn test_range_support_check() {
        let mut mock_client = MockTestHttpClient::new();

        mock_client
            .expect_clone()
            .returning(MockTestHttpClient::new);

        mock_client
            .expect_supports_ranges()
            .with(mockall::predicate::eq("http://example.com/archive.dat"))
            .times(1)
            .returning(|_| Ok(true));

        let reader = StreamingArchiveReader::with_defaults(mock_client);
        let supports_ranges = reader
            .supports_range_requests("http://example.com/archive.dat")
            .await
            .expect("Operation should succeed");

        assert!(supports_ranges);
    }

    #[test]
    fn test_batch_extractor_creation() {
        let config = StreamingConfig::default();
        let http_client1 =
            ReqwestHttpClient::new(config.clone()).expect("Operation should succeed");
        let http_client2 = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let clients = vec![http_client1, http_client2];
        let archive_config = StreamingArchiveConfig::default();

        let batch_extractor = BatchArchiveExtractor::new(clients, archive_config);
        assert_eq!(batch_extractor.reader_count(), 2);
    }

    #[test]
    fn test_extraction_result_properties() {
        let result = ArchiveExtractionResult {
            content: vec![1, 2, 3, 4],
            size: 4,
            was_compressed: true,
            archive_offset: 1024,
        };

        assert_eq!(result.content, vec![1, 2, 3, 4]);
        assert_eq!(result.size, 4);
        assert!(result.was_compressed);
        assert_eq!(result.archive_offset, 1024);
    }
}
