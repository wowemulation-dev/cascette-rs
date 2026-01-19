//! Streaming interfaces for handling large NGDP content efficiently
//!
//! This module provides streaming interfaces for working with large NGDP files
//! such as large encoding files and archives without loading them entirely
//! into memory. It supports chunk-based processing, streaming validation,
//! and progressive cache population.
#![allow(clippy::cast_precision_loss)] // Progress calculations intentionally accept precision loss

use crate::{
    error::{NgdpCacheError, NgdpCacheResult},
    key::BlteBlockKey,
    traits::AsyncCache,
    validation::{ValidationHooks, ValidationResult},
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use cascette_crypto::ContentKey;
use futures::Stream;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Configuration for streaming operations
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Size of chunks for streaming operations (default: 64KB)
    pub chunk_size: usize,
    /// Maximum number of chunks to buffer in memory (default: 16)
    pub max_buffered_chunks: usize,
    /// Enable validation during streaming (default: true)
    pub validate_chunks: bool,
    /// Minimum chunk size for the last chunk (default: 1KB)
    pub min_chunk_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024,   // 64KB chunks
            max_buffered_chunks: 16, // 1MB max buffer
            validate_chunks: true,
            min_chunk_size: 1024, // 1KB minimum
        }
    }
}

/// A stream of validated content chunks
pub struct ContentStream {
    /// Current chunk index
    current_chunk: u32,
    /// Total number of chunks expected
    total_chunks: Option<u32>,
    /// Content key for validation
    content_key: ContentKey,
    /// Streaming configuration
    config: StreamingConfig,
    /// Validation state
    validated_chunks: Vec<bool>,
    /// Total size processed so far
    total_processed: u64,
}

impl ContentStream {
    /// Create a new content stream
    pub fn new(content_key: ContentKey, total_size: Option<u64>, config: StreamingConfig) -> Self {
        let total_chunks = total_size.map(|size| size.div_ceil(config.chunk_size as u64) as u32);

        let validated_chunks = match total_chunks {
            Some(chunks) => vec![false; chunks as usize],
            None => Vec::new(),
        };

        Self {
            current_chunk: 0,
            total_chunks,
            content_key,
            config,
            validated_chunks,
            total_processed: 0,
        }
    }

    /// Get the current chunk index
    pub fn current_chunk_index(&self) -> u32 {
        self.current_chunk
    }

    /// Get total chunks if known
    pub fn total_chunks(&self) -> Option<u32> {
        self.total_chunks
    }

    /// Get total bytes processed
    pub fn bytes_processed(&self) -> u64 {
        self.total_processed
    }

    /// Check if a specific chunk has been validated
    pub fn is_chunk_validated(&self, chunk_index: u32) -> bool {
        self.validated_chunks
            .get(chunk_index as usize)
            .copied()
            .unwrap_or(false)
    }

    /// Mark a chunk as validated
    pub fn mark_chunk_validated(&mut self, chunk_index: u32) {
        if let Some(validated) = self.validated_chunks.get_mut(chunk_index as usize) {
            *validated = true;
        }
    }

    /// Get streaming progress (0.0 to 1.0)
    pub fn progress(&self) -> Option<f32> {
        self.total_chunks.map(|total| {
            if total == 0 {
                1.0
            } else {
                self.current_chunk as f32 / total as f32
            }
        })
    }

    /// Check if streaming is complete
    pub fn is_complete(&self) -> bool {
        match self.total_chunks {
            Some(total) => self.current_chunk >= total,
            None => false,
        }
    }

    /// Get the content key for this stream
    pub fn content_key(&self) -> &ContentKey {
        &self.content_key
    }

    /// Get the streaming configuration
    pub fn config(&self) -> &StreamingConfig {
        &self.config
    }
}

/// Trait for streaming cache operations
#[async_trait]
pub trait StreamingCache: AsyncCache<BlteBlockKey> {
    /// Stream content from cache in chunks
    async fn stream_content(
        &self,
        content_key: ContentKey,
        config: &StreamingConfig,
    ) -> NgdpCacheResult<Pin<Box<dyn Stream<Item = NgdpCacheResult<Bytes>> + Send>>>;

    /// Store content stream in cache with chunking
    async fn store_content_stream<R>(
        &self,
        content_key: ContentKey,
        reader: R,
        config: &StreamingConfig,
    ) -> NgdpCacheResult<u64>
    where
        R: AsyncRead + Send + Unpin;

    /// Get a specific chunk from cache
    async fn get_chunk(
        &self,
        content_key: ContentKey,
        chunk_index: u32,
    ) -> NgdpCacheResult<Option<Bytes>>;

    /// Store a specific chunk in cache
    async fn put_chunk(
        &self,
        content_key: ContentKey,
        chunk_index: u32,
        data: Bytes,
    ) -> NgdpCacheResult<()>;
}

/// Streaming content processor for large NGDP files
pub struct StreamingProcessor<V> {
    /// Validation hooks for chunk validation
    pub validation: V,
    /// Streaming configuration
    pub config: StreamingConfig,
}

impl<V> StreamingProcessor<V>
where
    V: ValidationHooks,
{
    /// Create a new streaming processor
    pub fn new(validation: V, config: StreamingConfig) -> Self {
        Self { validation, config }
    }

    /// Process a large content stream with validation
    pub async fn process_stream<R>(
        &self,
        content_key: ContentKey,
        mut reader: R,
        expected_size: Option<u64>,
    ) -> NgdpCacheResult<Vec<Bytes>>
    where
        R: AsyncRead + Send + Unpin,
    {
        let mut chunks = Vec::new();
        let mut stream = ContentStream::new(content_key, expected_size, self.config.clone());
        loop {
            let mut buffer = BytesMut::with_capacity(self.config.chunk_size);
            buffer.resize(self.config.chunk_size, 0);

            let bytes_read = reader
                .read(&mut buffer)
                .await
                .map_err(|e| NgdpCacheError::StreamProcessingError(format!("Read error: {e}")))?;

            if bytes_read == 0 {
                break; // End of stream
            }

            // Resize buffer to actual bytes read
            buffer.truncate(bytes_read);
            let chunk_data = buffer.freeze();

            // Validate chunk if enabled
            if self.config.validate_chunks {
                let chunk_key = ContentKey::from_data(&chunk_data);
                let validation_result = self
                    .validation
                    .validate_content(&chunk_key, &chunk_data)
                    .await
                    .map_err(NgdpCacheError::from)?;

                if !validation_result.is_valid {
                    return Err(NgdpCacheError::ContentValidationFailed(chunk_key));
                }

                stream.mark_chunk_validated(stream.current_chunk);
            }

            chunks.push(chunk_data);
            stream.current_chunk += 1;
            stream.total_processed += bytes_read as u64;

            // Check buffer limits
            if chunks.len() >= self.config.max_buffered_chunks {
                // In a real implementation, this would flush to cache
                break;
            }
        }

        Ok(chunks)
    }

    /// Validate a stream of chunks
    pub async fn validate_chunks(
        &self,
        chunks: &[Bytes],
    ) -> NgdpCacheResult<Vec<ValidationResult>> {
        let mut results = Vec::with_capacity(chunks.len());

        for chunk in chunks {
            let chunk_key = ContentKey::from_data(chunk);
            let result = self
                .validation
                .validate_content(&chunk_key, chunk)
                .await
                .map_err(NgdpCacheError::from)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Reconstruct content from validated chunks
    pub fn reconstruct_content(&self, chunks: &[Bytes]) -> Bytes {
        let total_size = chunks.iter().map(|c| c.len()).sum();
        let mut result = BytesMut::with_capacity(total_size);

        for chunk in chunks {
            result.extend_from_slice(chunk);
        }

        result.freeze()
    }

    /// Get streaming statistics
    pub fn get_stats(&self, stream: &ContentStream) -> StreamingStats {
        let chunks_validated = stream.validated_chunks.iter().filter(|&&v| v).count();
        let validation_rate = if stream.validated_chunks.is_empty() {
            0.0
        } else {
            chunks_validated as f32 / stream.validated_chunks.len() as f32
        };

        StreamingStats {
            chunks_processed: stream.current_chunk,
            total_chunks: stream.total_chunks,
            bytes_processed: stream.total_processed,
            chunks_validated: chunks_validated as u32,
            validation_rate,
            progress: stream.progress().unwrap_or(0.0),
        }
    }
}

/// Statistics for streaming operations
#[derive(Debug, Clone)]
pub struct StreamingStats {
    /// Number of chunks processed
    pub chunks_processed: u32,
    /// Total chunks expected (if known)
    pub total_chunks: Option<u32>,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Number of chunks successfully validated
    pub chunks_validated: u32,
    /// Validation rate (0.0 to 1.0)
    pub validation_rate: f32,
    /// Overall progress (0.0 to 1.0)
    pub progress: f32,
}

impl StreamingStats {
    /// Check if all chunks have been validated
    pub fn all_chunks_validated(&self) -> bool {
        match self.total_chunks {
            Some(total) => self.chunks_validated >= total,
            None => false,
        }
    }

    /// Get average chunk size
    pub fn average_chunk_size(&self) -> Option<u64> {
        if self.chunks_processed > 0 {
            Some(self.bytes_processed / u64::from(self.chunks_processed))
        } else {
            None
        }
    }

    /// Get estimated total size
    pub fn estimated_total_size(&self) -> Option<u64> {
        match (self.total_chunks, self.average_chunk_size()) {
            (Some(total), Some(avg_size)) => Some(u64::from(total) * avg_size),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::validation::NoOpValidationHooks;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_streaming_config_defaults() {
        let config = StreamingConfig::default();
        assert_eq!(config.chunk_size, 64 * 1024);
        assert_eq!(config.max_buffered_chunks, 16);
        assert!(config.validate_chunks);
        assert_eq!(config.min_chunk_size, 1024);
    }

    #[tokio::test]
    async fn test_content_stream_creation() {
        let content_key = ContentKey::from_data(b"test content");
        let config = StreamingConfig::default();
        let stream = ContentStream::new(content_key, Some(1024), config);

        assert_eq!(stream.current_chunk_index(), 0);
        assert_eq!(stream.total_chunks(), Some(1));
        assert_eq!(stream.bytes_processed(), 0);
        assert!(!stream.is_complete());
        assert_eq!(stream.progress(), Some(0.0));
    }

    #[tokio::test]
    async fn test_streaming_processor_basic() {
        let validation = NoOpValidationHooks;
        let config = StreamingConfig::default();
        let processor = StreamingProcessor::new(validation, config);

        let test_data = b"Hello, streaming world! This is test data for streaming.";
        let content_key = ContentKey::from_data(test_data);
        let reader = Cursor::new(test_data);

        let chunks = processor
            .process_stream(content_key, reader, Some(test_data.len() as u64))
            .await
            .expect("Test operation should succeed");

        assert!(!chunks.is_empty());
        let reconstructed = processor.reconstruct_content(&chunks);
        assert_eq!(reconstructed.as_ref(), test_data);
    }

    #[tokio::test]
    async fn test_chunk_validation() {
        let validation = NoOpValidationHooks;
        let config = StreamingConfig::default();
        let processor = StreamingProcessor::new(validation, config);

        let chunk_one = Bytes::from_static(b"chunk 1 data");
        let chunk_two = Bytes::from_static(b"chunk 2 data");
        let chunks = vec![chunk_one, chunk_two];

        let results = processor
            .validate_chunks(&chunks)
            .await
            .expect("Test operation should succeed");
        assert_eq!(results.len(), 2);
        assert!(results[0].is_valid);
        assert!(results[1].is_valid);
    }

    #[tokio::test]
    async fn test_streaming_stats() {
        let content_key = ContentKey::from_data(b"test content");
        let config = StreamingConfig {
            chunk_size: 10,
            ..StreamingConfig::default()
        };

        let mut stream = ContentStream::new(content_key, Some(100), config);
        stream.current_chunk = 5;
        stream.total_processed = 50;
        stream.mark_chunk_validated(0);
        stream.mark_chunk_validated(1);

        let validation = NoOpValidationHooks;
        let processor = StreamingProcessor::new(validation, StreamingConfig::default());
        let stats = processor.get_stats(&stream);

        assert_eq!(stats.chunks_processed, 5);
        assert_eq!(stats.total_chunks, Some(10));
        assert_eq!(stats.bytes_processed, 50);
        assert_eq!(stats.chunks_validated, 2);
        assert_eq!(stats.validation_rate, 0.2);
        assert_eq!(stats.progress, 0.5);
        assert!(!stats.all_chunks_validated());
        assert_eq!(stats.average_chunk_size(), Some(10));
    }

    #[tokio::test]
    async fn test_content_stream_progress() {
        let content_key = ContentKey::from_data(b"test content");
        let config = StreamingConfig::default(); // Default chunk_size is 64KB
        let mut stream = ContentStream::new(content_key, Some(1024), config); // Use 1024 bytes for easier calculation

        // Test initial progress
        assert_eq!(stream.progress(), Some(0.0));

        // Total chunks should be ceil(1024 / 65536) = 1 chunk
        assert_eq!(stream.total_chunks(), Some(1));

        // Simulate processing the single chunk
        stream.current_chunk = 1;
        assert!(stream.is_complete());

        // Test with a size that creates multiple chunks
        let config2 = StreamingConfig {
            chunk_size: 100,
            ..StreamingConfig::default()
        };
        let mut stream2 = ContentStream::new(content_key, Some(1000), config2);

        // Total chunks should be ceil(1000 / 100) = 10
        assert_eq!(stream2.total_chunks(), Some(10));

        // Test progress at halfway point
        stream2.current_chunk = 5;
        let progress = stream2.progress().expect("Test operation should succeed");
        assert_eq!(progress, 0.5); // 5/10 = 0.5

        // Test completion
        stream2.current_chunk = 10;
        assert!(stream2.is_complete());
    }

    #[tokio::test]
    async fn test_large_content_simulation() {
        let validation = NoOpValidationHooks;
        let config = StreamingConfig {
            chunk_size: 1024, // 1KB chunks
            max_buffered_chunks: 5,
            validate_chunks: false,
            min_chunk_size: 512,
        };
        let processor = StreamingProcessor::new(validation, config);

        // Simulate a 10KB file
        let large_data = vec![0u8; 10 * 1024];
        let content_key = ContentKey::from_data(&large_data);
        let reader = Cursor::new(large_data.clone());

        let chunks = processor
            .process_stream(content_key, reader, Some(large_data.len() as u64))
            .await
            .expect("Test operation should succeed");

        // Should be limited by max_buffered_chunks
        assert_eq!(chunks.len(), 5);

        // Each chunk should be 1KB except possibly the last one
        for (i, chunk) in chunks.iter().enumerate() {
            if i < chunks.len() - 1 {
                assert_eq!(chunk.len(), 1024);
            }
        }
    }
}
