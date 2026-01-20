//! HTTP range request support for partial CDN archive downloads

use reqwest::header::{CONTENT_RANGE, RANGE};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use super::CdnEndpoint;

/// Cross-platform async sleep function
///
/// On native platforms, uses tokio::time::sleep.
/// On WASM, uses gloo_timers::future::TimeoutFuture.
#[cfg(not(target_arch = "wasm32"))]
async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    gloo_timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
}

/// Range request downloader for efficient partial archive downloads
pub struct RangeDownloader {
    client: Arc<reqwest::Client>,
    max_retries: u32,
    #[allow(dead_code)]
    chunk_size: usize, // Reserved for future chunked download implementation
}

/// Errors that can occur during range request operations
#[derive(Debug, Error)]
pub enum RangeError {
    /// Network request failed
    #[error("Network request failed: {0}")]
    Network(#[from] reqwest::Error),

    /// Server returned invalid response status
    #[error("Invalid response status: {0}")]
    InvalidResponse(reqwest::StatusCode),

    /// Content-Range header is invalid or missing
    #[error("Invalid or missing Content-Range header")]
    InvalidContentRange,

    /// Header value is not valid UTF-8
    #[error("Invalid header value: {0}")]
    InvalidHeader(#[from] reqwest::header::ToStrError),

    /// Received incomplete data
    #[error("Incomplete data: expected {expected} bytes, received {received}")]
    IncompleteData { expected: usize, received: usize },

    /// Maximum retry attempts exceeded
    #[error("Maximum retry attempts exceeded")]
    MaxRetriesExceeded,
}

impl RangeDownloader {
    /// Create a new range downloader with default configuration
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Result<Self, RangeError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;

        Ok(Self {
            client: Arc::new(client),
            max_retries: 3,
            chunk_size: 1024 * 1024, // 1MB default chunk size
        })
    }

    /// Create a new range downloader with default configuration (WASM version)
    ///
    /// On WASM, timeout is not supported by reqwest as the browser manages
    /// request timeouts via the Fetch API.
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Result<Self, RangeError> {
        let client = reqwest::Client::builder().build()?;

        Ok(Self {
            client: Arc::new(client),
            max_retries: 3,
            chunk_size: 1024 * 1024, // 1MB default chunk size
        })
    }

    /// Create a new range downloader with custom configuration
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_config(
        max_retries: u32,
        chunk_size: usize,
        timeout: Duration,
    ) -> Result<Self, RangeError> {
        let client = reqwest::Client::builder().timeout(timeout).build()?;

        Ok(Self {
            client: Arc::new(client),
            max_retries,
            chunk_size, // Reserved for future chunked download implementation
        })
    }

    /// Create a new range downloader with custom configuration (WASM version)
    ///
    /// On WASM, the timeout parameter is ignored as the browser manages
    /// request timeouts via the Fetch API.
    #[cfg(target_arch = "wasm32")]
    pub fn with_config(
        max_retries: u32,
        chunk_size: usize,
        _timeout: Duration, // Ignored on WASM
    ) -> Result<Self, RangeError> {
        let client = reqwest::Client::builder().build()?;

        Ok(Self {
            client: Arc::new(client),
            max_retries,
            chunk_size, // Reserved for future chunked download implementation
        })
    }

    /// Download a specific byte range from a URL
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    /// * `offset` - Starting byte offset (inclusive)
    /// * `length` - Number of bytes to download
    ///
    /// # Returns
    ///
    /// The downloaded bytes, or an error if the request fails
    pub async fn download_range(
        &self,
        url: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, RangeError> {
        let range_header = format!("bytes={}-{}", offset, offset + length - 1);

        for attempt in 0..self.max_retries {
            let response = self
                .client
                .get(url)
                .header(RANGE, &range_header)
                .send()
                .await?;

            // Verify we got a partial content response
            if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                if attempt < self.max_retries - 1 {
                    // Exponential backoff
                    sleep(Duration::from_secs(1_u64 << attempt)).await;
                    continue;
                }
                return Err(RangeError::InvalidResponse(response.status()));
            }

            // Verify content range header
            if let Some(content_range) = response.headers().get(CONTENT_RANGE) {
                let range_str = content_range.to_str()?;
                if !validate_content_range(range_str, offset, length) {
                    return Err(RangeError::InvalidContentRange);
                }
            }

            let data = response.bytes().await?.to_vec();

            // Verify we got the expected amount of data
            if data.len() != length as usize {
                if attempt < self.max_retries - 1 {
                    // Retry on incomplete data
                    sleep(Duration::from_secs(1_u64 << attempt)).await;
                    continue;
                }
                return Err(RangeError::IncompleteData {
                    expected: length as usize,
                    received: data.len(),
                });
            }

            return Ok(data);
        }

        Err(RangeError::MaxRetriesExceeded)
    }

    /// Download content from a CDN archive using range requests
    ///
    /// This method constructs the proper CDN URL for an archive and downloads
    /// the specified byte range.
    ///
    /// # Arguments
    ///
    /// * `cdn_endpoint` - CDN endpoint configuration
    /// * `archive_name` - Name of the archive file (e.g., "a1b2c3d4e5f67890")
    /// * `offset` - Starting byte offset within the archive
    /// * `size` - Number of bytes to download
    ///
    /// # Returns
    ///
    /// The downloaded archive content bytes
    pub async fn download_archive_content(
        &self,
        cdn_endpoint: &CdnEndpoint,
        archive_name: &str,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, RangeError> {
        // Construct archive URL with proper CDN path structure
        // Archives are stored in a two-level directory structure based on the first 4 characters
        let url = if let Some(product_path) = &cdn_endpoint.product_path {
            format!(
                "https://{}/{}/{}/data/{}/{}/{}",
                cdn_endpoint.host,
                cdn_endpoint.path,
                product_path,
                &archive_name[0..2],
                &archive_name[2..4],
                archive_name
            )
        } else {
            format!(
                "https://{}/{}/data/{}/{}/{}",
                cdn_endpoint.host,
                cdn_endpoint.path,
                &archive_name[0..2],
                &archive_name[2..4],
                archive_name
            )
        };

        self.download_range(&url, offset, size).await
    }

    /// Download multiple ranges from an archive efficiently
    ///
    /// This method can be used to download multiple non-contiguous ranges
    /// from the same archive in parallel.
    pub async fn download_archive_ranges(
        &self,
        cdn_endpoint: &CdnEndpoint,
        archive_name: &str,
        ranges: &[(u64, u64)], // (offset, size) pairs
    ) -> Result<Vec<Vec<u8>>, RangeError> {
        let mut results = Vec::with_capacity(ranges.len());

        // Download ranges in parallel
        let futures: Vec<_> = ranges
            .iter()
            .map(|(offset, size)| {
                self.download_archive_content(cdn_endpoint, archive_name, *offset, *size)
            })
            .collect();

        for future in futures {
            results.push(future.await?);
        }

        Ok(results)
    }
}

impl Default for RangeDownloader {
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        // expect_used: RangeDownloader::new() only fails if reqwest Client::builder().build()
        // fails, which should never happen with default settings.
        Self::new().expect("RangeDownloader creation failed")
    }
}

/// Validate that a Content-Range header matches the expected range
///
/// Parses headers like "bytes 200-1023/2048" and validates against expected values.
fn validate_content_range(range_str: &str, expected_start: u64, expected_length: u64) -> bool {
    // Parse "bytes START-END/TOTAL" format
    if let Some(bytes_part) = range_str.strip_prefix("bytes ")
        && let Some((range, _total)) = bytes_part.split_once('/')
        && let Some((start_str, end_str)) = range.split_once('-')
        && let (Ok(start), Ok(end)) = (start_str.parse::<u64>(), end_str.parse::<u64>())
    {
        return start == expected_start && (end - start + 1) == expected_length;
    }
    false
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_content_range() {
        // Valid range
        assert!(validate_content_range("bytes 200-1023/2048", 200, 824));

        // Invalid start
        assert!(!validate_content_range("bytes 100-1023/2048", 200, 824));

        // Invalid length
        assert!(!validate_content_range("bytes 200-999/2048", 200, 824));

        // Invalid format
        assert!(!validate_content_range("invalid", 200, 824));
    }

    #[test]
    fn test_range_header_calculation() {
        // Offset 100, length 50 should request bytes 100-149
        let offset = 100_u64;
        let length = 50_u64;
        let range_header = format!("bytes={}-{}", offset, offset + length - 1);
        assert_eq!(range_header, "bytes=100-149");
    }
}
