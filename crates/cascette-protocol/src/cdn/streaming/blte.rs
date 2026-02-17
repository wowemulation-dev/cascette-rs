//! Streaming BLTE processor for efficient decompression from HTTP range responses
//!
//! This module provides progressive BLTE decompression capabilities without requiring
//! the entire BLTE file to be loaded into memory. It integrates with the existing
//! BLTE decompression code while supporting streaming operations.

use binrw::BinRead;
use bytes::Bytes;
use std::io::Cursor;

use crate::blte::{BlteError, BlteHeader, ChunkData};
use crate::cdn::streaming::{HttpClient, HttpRange, StreamingError};
use cascette_crypto::TactKeyStore;

/// Configuration for streaming BLTE operations
#[derive(Debug, Clone)]
pub struct StreamingBlteConfig {
    /// Maximum buffer size for decompression (default: 16MB)
    pub max_buffer_size: usize,
    /// Chunk read-ahead size for streaming (default: 4MB)
    pub chunk_read_ahead: usize,
    /// Whether to verify chunk checksums (default: true)
    pub verify_checksums: bool,
}

impl Default for StreamingBlteConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 16 * 1024 * 1024, // 16MB
            chunk_read_ahead: 4 * 1024 * 1024, // 4MB
            verify_checksums: true,
        }
    }
}

/// Progressive BLTE processor that can decompress content from HTTP range responses
#[derive(Debug)]
pub struct StreamingBlteProcessor<H: HttpClient> {
    http_client: H,
    #[allow(dead_code)]
    config: StreamingBlteConfig,
}

impl<H: HttpClient> StreamingBlteProcessor<H> {
    /// Create a new streaming BLTE processor
    pub fn new(http_client: H, config: StreamingBlteConfig) -> Self {
        Self {
            http_client,
            config,
        }
    }

    /// Create processor with default configuration
    pub fn with_defaults(http_client: H) -> Self {
        Self::new(http_client, StreamingBlteConfig::default())
    }

    /// Decompress BLTE content from URL without loading entire file
    ///
    /// # Arguments
    /// * `url` - URL of the BLTE content
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Decompressed content bytes
    ///
    /// # Errors
    /// Returns `StreamingError` for network failures or BLTE decompression errors
    pub async fn decompress_from_url(
        &self,
        url: &str,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        // First, read the BLTE header to determine structure
        let header_data = self.read_blte_header(url).await?;
        let header = self.parse_blte_header(&header_data)?;

        if header.is_single_chunk() {
            self.decompress_single_chunk(url, &header, key_store).await
        } else {
            self.decompress_multi_chunk(url, &header, key_store).await
        }
    }

    /// Decompress specific chunk range from BLTE content
    ///
    /// # Arguments
    /// * `url` - URL of the BLTE content
    /// * `chunk_start` - Starting chunk index (0-based)
    /// * `chunk_count` - Number of chunks to decompress
    /// * `key_store` - Optional TACT key store for decryption
    ///
    /// # Returns
    /// Decompressed content bytes for the specified chunk range
    pub async fn decompress_chunk_range(
        &self,
        url: &str,
        chunk_start: usize,
        chunk_count: usize,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        let header_data = self.read_blte_header(url).await?;
        let header = self.parse_blte_header(&header_data)?;

        if header.is_single_chunk() {
            if chunk_start == 0 && chunk_count >= 1 {
                self.decompress_single_chunk(url, &header, key_store).await
            } else {
                Ok(Vec::new()) // No chunks in range
            }
        } else {
            self.decompress_multi_chunk_range(url, &header, chunk_start, chunk_count, key_store)
                .await
        }
    }

    /// Read and parse BLTE header from URL
    async fn read_blte_header(&self, url: &str) -> Result<Bytes, StreamingError> {
        // BLTE header is at most 12 bytes (signature + size) for single chunk
        // For multi-chunk, we need to read up to extended header
        // Start with 12 bytes, then read more if needed
        let initial_data = self
            .http_client
            .get_range(url, Some(HttpRange::new(0, 11)))
            .await?;

        // Check if this is a multi-chunk file by examining the header
        if initial_data.len() >= 8 {
            let size_bytes = &initial_data[4..8];
            let size =
                u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]);

            // If size is 0, this indicates multi-chunk format
            if size == 0 {
                // Need to read extended header to get chunk count
                let extended_data = self
                    .http_client
                    .get_range(url, Some(HttpRange::new(0, 15))) // Read up to chunk count
                    .await?;

                if extended_data.len() >= 16 {
                    let chunk_count_bytes = &extended_data[12..16];
                    let chunk_count = u32::from_be_bytes([
                        chunk_count_bytes[0],
                        chunk_count_bytes[1],
                        chunk_count_bytes[2],
                        chunk_count_bytes[3],
                    ]);

                    // Calculate total header size: 16 + (chunk_count * 12)
                    let total_header_size = 16 + (chunk_count as usize * 12);

                    // Read complete header
                    let complete_header = self
                        .http_client
                        .get_range(url, Some(HttpRange::new(0, total_header_size as u64 - 1)))
                        .await?;

                    Ok(complete_header)
                } else {
                    Ok(extended_data)
                }
            } else {
                Ok(initial_data)
            }
        } else {
            Ok(initial_data)
        }
    }

    /// Parse BLTE header from bytes
    #[allow(clippy::unused_self)]
    fn parse_blte_header(&self, data: &[u8]) -> Result<BlteHeader, StreamingError> {
        let mut cursor = Cursor::new(data);
        BlteHeader::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| {
            StreamingError::BlteError {
                source: BlteError::InvalidHeader(format!("Header parse error: {e}")),
            }
        })
    }

    /// Decompress single-chunk BLTE content
    async fn decompress_single_chunk(
        &self,
        url: &str,
        _header: &BlteHeader,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        // Calculate header size
        let header_size = 8; // Single chunk header is always 8 bytes

        // Get content length to determine chunk size
        let content_length = self.http_client.get_content_length(url).await?;
        let chunk_size = content_length - header_size;

        // Read chunk data
        let chunk_data = self
            .http_client
            .get_range(url, Some(HttpRange::new(header_size, content_length - 1)))
            .await?;

        // Parse and decompress chunk
        let mut cursor = Cursor::new(&chunk_data[..]);
        let chunk =
            ChunkData::read_options(&mut cursor, binrw::Endian::Big, (chunk_size as usize,))
                .map_err(|e| StreamingError::BlteError {
                    source: BlteError::InvalidChunk(format!("Chunk parse error: {e}")),
                })?;

        // Decompress with optional decryption
        let decompressed = if let Some(key_store) = key_store {
            crate::blte::decrypt_chunk_with_keys(&chunk.data, key_store, 0)
                .map_err(|e| StreamingError::BlteError { source: e })?
        } else {
            chunk
                .decompress(0)
                .map_err(|e| StreamingError::BlteError { source: e })?
        };

        Ok(decompressed)
    }

    /// Decompress multi-chunk BLTE content
    async fn decompress_multi_chunk(
        &self,
        url: &str,
        header: &BlteHeader,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        let extended = header
            .extended
            .as_ref()
            .ok_or_else(|| StreamingError::BlteError {
                source: BlteError::InvalidHeader(
                    "Missing extended header for multi-chunk".to_string(),
                ),
            })?;

        let mut result = Vec::new();
        let mut current_offset = header.total_header_size() as u64;

        for (index, chunk_info) in extended.chunk_infos.iter().enumerate() {
            let chunk_end = current_offset + u64::from(chunk_info.compressed_size) - 1;

            // Read chunk data
            let chunk_data = self
                .http_client
                .get_range(url, Some(HttpRange::new(current_offset, chunk_end)))
                .await?;

            // Parse chunk
            let mut cursor = Cursor::new(&chunk_data[..]);
            let chunk = ChunkData::read_options(
                &mut cursor,
                binrw::Endian::Big,
                (chunk_info.compressed_size as usize,),
            )
            .map_err(|e| StreamingError::BlteError {
                source: BlteError::InvalidChunk(format!("Chunk {index} parse error: {e}")),
            })?;

            // Decompress with optional decryption
            let decompressed = if let Some(key_store) = key_store {
                crate::blte::decrypt_chunk_with_keys(&chunk.data, key_store, index)
                    .map_err(|e| StreamingError::BlteError { source: e })?
            } else {
                chunk
                    .decompress(index)
                    .map_err(|e| StreamingError::BlteError { source: e })?
            };

            result.extend_from_slice(&decompressed);
            current_offset += u64::from(chunk_info.compressed_size);
        }

        Ok(result)
    }

    /// Decompress specific range of chunks from multi-chunk BLTE content
    async fn decompress_multi_chunk_range(
        &self,
        url: &str,
        header: &BlteHeader,
        chunk_start: usize,
        chunk_count: usize,
        key_store: Option<&TactKeyStore>,
    ) -> Result<Vec<u8>, StreamingError> {
        let extended = header
            .extended
            .as_ref()
            .ok_or_else(|| StreamingError::BlteError {
                source: BlteError::InvalidHeader(
                    "Missing extended header for multi-chunk".to_string(),
                ),
            })?;

        if chunk_start >= extended.chunk_infos.len() {
            return Ok(Vec::new()); // No chunks in range
        }

        let end_chunk = (chunk_start + chunk_count).min(extended.chunk_infos.len());
        let mut result = Vec::new();

        // Calculate starting offset
        let mut current_offset = header.total_header_size() as u64;
        for i in 0..chunk_start {
            current_offset += u64::from(extended.chunk_infos[i].compressed_size);
        }

        // Process requested chunks
        for index in chunk_start..end_chunk {
            let chunk_info = &extended.chunk_infos[index];
            let chunk_end = current_offset + u64::from(chunk_info.compressed_size) - 1;

            // Read chunk data
            let chunk_data = self
                .http_client
                .get_range(url, Some(HttpRange::new(current_offset, chunk_end)))
                .await?;

            // Parse chunk
            let mut cursor = Cursor::new(&chunk_data[..]);
            let chunk = ChunkData::read_options(
                &mut cursor,
                binrw::Endian::Big,
                (chunk_info.compressed_size as usize,),
            )
            .map_err(|e| StreamingError::BlteError {
                source: BlteError::InvalidChunk(format!("Chunk {index} parse error: {e}")),
            })?;

            // Decompress with optional decryption
            let decompressed = if let Some(key_store) = key_store {
                crate::blte::decrypt_chunk_with_keys(&chunk.data, key_store, index)
                    .map_err(|e| StreamingError::BlteError { source: e })?
            } else {
                chunk
                    .decompress(index)
                    .map_err(|e| StreamingError::BlteError { source: e })?
            };

            result.extend_from_slice(&decompressed);
            current_offset += u64::from(chunk_info.compressed_size);
        }

        Ok(result)
    }

    /// Get BLTE header information without decompressing content
    pub async fn get_header_info(&self, url: &str) -> Result<BlteHeaderInfo, StreamingError> {
        let header_data = self.read_blte_header(url).await?;
        let header = self.parse_blte_header(&header_data)?;

        let chunk_count = if header.is_single_chunk() {
            1
        } else {
            header
                .extended
                .as_ref()
                .map_or(0, |ext| ext.chunk_infos.len())
        };

        let total_decompressed_size = if let Some(ref extended) = header.extended {
            extended
                .chunk_infos
                .iter()
                .map(|info| u64::from(info.decompressed_size))
                .sum()
        } else {
            // For single chunk, we need to get content length and estimate
            let content_length = self.http_client.get_content_length(url).await?;
            content_length - 8 // Subtract header size, rough estimate
        };

        Ok(BlteHeaderInfo {
            is_single_chunk: header.is_single_chunk(),
            chunk_count,
            total_decompressed_size,
            header_size: header.total_header_size(),
        })
    }
}

/// Information about BLTE header structure
#[derive(Debug, Clone)]
pub struct BlteHeaderInfo {
    /// Whether this is a single-chunk BLTE file
    pub is_single_chunk: bool,
    /// Number of chunks in the file
    pub chunk_count: usize,
    /// Total decompressed size of all chunks
    pub total_decompressed_size: u64,
    /// Size of the BLTE header in bytes
    pub header_size: usize,
}

// BLTE error integration is handled in the main streaming error module

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

        #[async_trait]
        impl HttpClient for TestHttpClient {
            async fn get_range(&self, url: &str, range: Option<HttpRange>) -> Result<Bytes, StreamingError>;
            async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;
            async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
        }
    }

    #[test]
    fn test_streaming_blte_config_defaults() {
        let config = StreamingBlteConfig::default();
        assert_eq!(config.max_buffer_size, 16 * 1024 * 1024);
        assert_eq!(config.chunk_read_ahead, 4 * 1024 * 1024);
        assert!(config.verify_checksums);
    }

    #[test]
    fn test_processor_creation() {
        let config = StreamingConfig::default();
        let http_client = ReqwestHttpClient::new(config).expect("Operation should succeed");
        let _processor = StreamingBlteProcessor::with_defaults(http_client);

        let blte_config = StreamingBlteConfig {
            max_buffer_size: 8 * 1024 * 1024,
            chunk_read_ahead: 2 * 1024 * 1024,
            verify_checksums: false,
        };
        let config = StreamingConfig::default();
        let http_client = ReqwestHttpClient::new(config).expect("Operation should succeed");
        let _processor = StreamingBlteProcessor::new(http_client, blte_config);
    }

    #[tokio::test]
    async fn test_single_chunk_header_detection() {
        let mut mock_client = MockTestHttpClient::new();

        // Mock single-chunk BLTE header (BLTE signature + non-zero size)
        let header_data = vec![
            0x42, 0x4C, 0x54, 0x45, // "BLTE" signature
            0x00, 0x00, 0x10, 0x00, // Size = 4096 (non-zero = single chunk)
        ];

        mock_client
            .expect_get_range()
            .with(
                mockall::predicate::eq("http://example.com/test.blte"),
                mockall::predicate::eq(Some(HttpRange::new(0, 11))),
            )
            .times(1)
            .returning(move |_, _| Ok(Bytes::from(header_data.clone())));

        let processor = StreamingBlteProcessor::with_defaults(mock_client);
        let header_data = processor
            .read_blte_header("http://example.com/test.blte")
            .await
            .expect("Operation should succeed");

        assert_eq!(header_data.len(), 8);
        assert_eq!(&header_data[0..4], b"BLTE");
    }

    #[tokio::test]
    async fn test_multi_chunk_header_detection() {
        let mut mock_client = MockTestHttpClient::new();

        // Mock multi-chunk BLTE header detection sequence
        let initial_header = vec![
            0x42, 0x4C, 0x54, 0x45, // "BLTE" signature
            0x00, 0x00, 0x00, 0x00, // Size = 0 (indicates multi-chunk)
            0x00, 0x00, 0x00, 0x00, // Padding
        ];

        let extended_header = vec![
            0x42, 0x4C, 0x54, 0x45, // "BLTE" signature
            0x00, 0x00, 0x00, 0x00, // Size = 0
            0x00, 0x00, 0x00, 0x00, // Flags
            0x00, 0x00, 0x00, 0x02, // Chunk count = 2
        ];

        let complete_header = vec![
            0x42, 0x4C, 0x54, 0x45, // "BLTE" signature
            0x00, 0x00, 0x00, 0x00, // Size = 0
            0x00, 0x00, 0x00, 0x00, // Flags
            0x00, 0x00, 0x00, 0x02, // Chunk count = 2
            // Chunk 1 info (12 bytes)
            0x00, 0x00, 0x10, 0x00, // Compressed size
            0x00, 0x00, 0x20, 0x00, // Decompressed size
            0x12, 0x34, 0x56, 0x78, // Hash prefix
            // Chunk 2 info (12 bytes)
            0x00, 0x00, 0x08, 0x00, // Compressed size
            0x00, 0x00, 0x10, 0x00, // Decompressed size
            0x9A, 0xBC, 0xDE, 0xF0, // Hash prefix
        ];

        // First call: initial header read
        mock_client
            .expect_get_range()
            .with(
                mockall::predicate::eq("http://example.com/multi.blte"),
                mockall::predicate::eq(Some(HttpRange::new(0, 11))),
            )
            .times(1)
            .returning(move |_, _| Ok(Bytes::from(initial_header.clone())));

        // Second call: extended header to get chunk count
        mock_client
            .expect_get_range()
            .with(
                mockall::predicate::eq("http://example.com/multi.blte"),
                mockall::predicate::eq(Some(HttpRange::new(0, 15))),
            )
            .times(1)
            .returning(move |_, _| Ok(Bytes::from(extended_header.clone())));

        // Third call: complete header with chunk info
        mock_client
            .expect_get_range()
            .with(
                mockall::predicate::eq("http://example.com/multi.blte"),
                mockall::predicate::eq(Some(HttpRange::new(0, 39))), // 16 + (2 * 12) - 1
            )
            .times(1)
            .returning(move |_, _| Ok(Bytes::from(complete_header.clone())));

        let processor = StreamingBlteProcessor::with_defaults(mock_client);
        let header_data = processor
            .read_blte_header("http://example.com/multi.blte")
            .await
            .expect("Operation should succeed");

        assert_eq!(header_data.len(), 40); // Complete multi-chunk header
        assert_eq!(&header_data[0..4], b"BLTE");
    }

    #[tokio::test]
    async fn test_header_info_extraction() {
        let mut mock_client = MockTestHttpClient::new();

        // Mock single-chunk header (header_size = 0 means single chunk)
        let header_data = vec![
            0x42, 0x4C, 0x54, 0x45, // "BLTE" signature
            0x00, 0x00, 0x00, 0x00, // Header size = 0 (single chunk format)
        ];

        mock_client
            .expect_get_range()
            .times(1..=3) // Allow multiple calls for header parsing
            .returning(move |_, _| Ok(Bytes::from(header_data.clone())));

        mock_client
            .expect_get_content_length()
            .times(1..=2) // May be called multiple times
            .returning(|_| Ok(4104)); // 8 bytes header + 4096 data

        let processor = StreamingBlteProcessor::with_defaults(mock_client);
        let info = processor
            .get_header_info("http://example.com/test.blte")
            .await
            .expect("Operation should succeed");

        assert!(info.is_single_chunk);
        assert_eq!(info.chunk_count, 1);
        assert_eq!(info.header_size, 8); // Single chunk = 8 bytes total header
        // Decompressed size is estimated for single chunk
        assert_eq!(info.total_decompressed_size, 4096);
    }
}
