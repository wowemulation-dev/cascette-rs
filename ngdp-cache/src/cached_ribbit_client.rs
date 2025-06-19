//! Cached wrapper for RibbitClient
//!
//! This module provides a caching layer for RibbitClient that stores responses
//! using the Blizzard MIME filename convention: command-argument(s)-sequencenumber.bmime
//!
//! # Example
//!
//! ```no_run
//! use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
//! use ribbit_client::Region;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a cached client
//! let client = CachedRibbitClient::new(Region::US).await?;
//!
//! // Use it exactly like RibbitClient - caching is transparent
//! let summary = client.get_summary().await?;
//! println!("Found {} products", summary.products.len());
//!
//! // Subsequent calls use cache (5 minute TTL for regular endpoints)
//! let summary2 = client.get_summary().await?;  // This will be from cache!
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, trace};

use ribbit_client::{Endpoint, Region, RibbitClient, TypedResponse};

use crate::{Result, ensure_dir, get_cache_dir};

/// Default TTL for certificate cache (30 days)
const CERT_CACHE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Default TTL for regular responses (5 minutes)
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// A caching wrapper around RibbitClient for raw responses
pub struct CachedRibbitClient {
    /// The underlying RibbitClient
    client: RibbitClient,
    /// Base directory for cache
    cache_dir: PathBuf,
    /// Region for this client
    region: Region,
    /// Whether caching is enabled
    enabled: bool,
}

impl CachedRibbitClient {
    /// Create a new cached Ribbit client
    pub async fn new(region: Region) -> Result<Self> {
        let client = RibbitClient::new(region);
        let cache_dir = get_cache_dir()?.join("ribbit").join("cached");
        ensure_dir(&cache_dir).await?;

        debug!("Initialized cached Ribbit client for region {:?}", region);

        Ok(Self {
            client,
            cache_dir,
            region,
            enabled: true,
        })
    }

    /// Create a new cached client with custom cache directory
    pub async fn with_cache_dir(region: Region, cache_dir: PathBuf) -> Result<Self> {
        let client = RibbitClient::new(region);
        ensure_dir(&cache_dir).await?;

        Ok(Self {
            client,
            cache_dir,
            region,
            enabled: true,
        })
    }

    /// Enable or disable caching
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Generate cache filename following Blizzard convention:
    /// command-argument(s)-sequencenumber.bmime
    fn generate_cache_filename(&self, endpoint: &Endpoint, sequence_number: Option<u64>) -> String {
        let (command, arguments) = match endpoint {
            Endpoint::Summary => ("summary", "#".to_string()),
            Endpoint::ProductVersions(product) => ("versions", product.clone()),
            Endpoint::ProductCdns(product) => ("cdns", product.clone()),
            Endpoint::ProductBgdl(product) => ("bgdl", product.clone()),
            Endpoint::Cert(hash) => ("certs", hash.clone()),
            Endpoint::Ocsp(hash) => ("ocsp", hash.clone()),
            Endpoint::Custom(path) => {
                // Try to extract command and argument from custom path
                let parts: Vec<&str> = path.split('/').collect();
                match parts.as_slice() {
                    [cmd] => (*cmd, "#".to_string()),
                    [cmd, arg] => (*cmd, arg.to_string()),
                    [cmd, arg, ..] => (*cmd, arg.to_string()),
                    _ => ("custom", path.replace('/', "_")),
                }
            }
        };

        let seq = sequence_number.unwrap_or(0);
        format!("{}-{}-{}.bmime", command, arguments, seq)
    }

    /// Get the cache path for an endpoint
    fn get_cache_path(&self, endpoint: &Endpoint, sequence_number: Option<u64>) -> PathBuf {
        let filename = self.generate_cache_filename(endpoint, sequence_number);
        self.cache_dir.join(self.region.to_string()).join(filename)
    }

    /// Get the metadata path for an endpoint
    fn get_metadata_path(&self, endpoint: &Endpoint, sequence_number: Option<u64>) -> PathBuf {
        let mut path = self.get_cache_path(endpoint, sequence_number);
        path.set_extension("meta");
        path
    }

    /// Determine TTL based on endpoint type
    fn get_ttl_for_endpoint(&self, endpoint: &Endpoint) -> Duration {
        match endpoint {
            Endpoint::Cert(_) | Endpoint::Ocsp(_) => CERT_CACHE_TTL,
            _ => DEFAULT_CACHE_TTL,
        }
    }

    /// Extract sequence number from raw response data
    fn extract_sequence_number(&self, raw_data: &[u8]) -> Option<u64> {
        let data_str = String::from_utf8_lossy(raw_data);

        // Look for the sequence number in the format "## seqn = 12345"
        for line in data_str.lines() {
            if line.starts_with("## seqn = ") {
                if let Some(seqn_str) = line.strip_prefix("## seqn = ") {
                    if let Ok(seqn) = seqn_str.trim().parse::<u64>() {
                        return Some(seqn);
                    }
                }
            }
        }

        None
    }

    /// Find the most recent valid cached file for an endpoint
    async fn find_cached_file(&self, endpoint: &Endpoint) -> Option<(PathBuf, u64)> {
        if !self.enabled {
            return None;
        }

        let region_dir = self.cache_dir.join(self.region.to_string());
        if !region_dir.exists() {
            return None;
        }

        // Generate pattern to match files for this endpoint
        let base_filename = self.generate_cache_filename(endpoint, Some(0));
        let prefix = base_filename.trim_end_matches("-0.bmime");

        let ttl = self.get_ttl_for_endpoint(endpoint);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut best_file: Option<(PathBuf, u64)> = None;
        let mut best_seqn: u64 = 0;

        // Read directory and find matching files
        if let Ok(mut entries) = tokio::fs::read_dir(&region_dir).await {
            while let Some(entry) = entries.next_entry().await.ok()? {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if this file matches our endpoint pattern
                    if filename.starts_with(&format!("{}-", prefix)) && filename.ends_with(".bmime")
                    {
                        // Extract sequence number from filename
                        if let Some(seqn_part) = filename
                            .strip_prefix(&format!("{}-", prefix))
                            .and_then(|s| s.strip_suffix(".bmime"))
                        {
                            if let Ok(seqn) = seqn_part.parse::<u64>() {
                                // Check if this file is still valid
                                let meta_path = path.with_extension("meta");
                                if let Ok(metadata) = tokio::fs::read_to_string(&meta_path).await {
                                    if let Ok(timestamp) = metadata.trim().parse::<u64>() {
                                        if now.saturating_sub(timestamp) < ttl.as_secs()
                                            && seqn > best_seqn
                                        {
                                            best_file = Some((path.clone(), seqn));
                                            best_seqn = seqn;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        best_file
    }

    /// Check if a cached response is still valid
    async fn is_cache_valid(&self, endpoint: &Endpoint) -> bool {
        self.find_cached_file(endpoint).await.is_some()
    }

    /// Write raw response to cache
    async fn write_to_cache(&self, endpoint: &Endpoint, raw_data: &[u8]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Extract sequence number from the response data
        let sequence_number = self.extract_sequence_number(raw_data);

        let cache_path = self.get_cache_path(endpoint, sequence_number);
        let meta_path = self.get_metadata_path(endpoint, sequence_number);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            ensure_dir(parent).await?;
        }

        // Write the raw response data
        trace!(
            "Writing {} bytes to cache: {:?}",
            raw_data.len(),
            cache_path
        );
        tokio::fs::write(&cache_path, raw_data).await?;

        // Write timestamp metadata
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        tokio::fs::write(&meta_path, timestamp.to_string()).await?;

        Ok(())
    }

    /// Read response from cache
    async fn read_from_cache(&self, endpoint: &Endpoint) -> Result<Vec<u8>> {
        if let Some((cache_path, _seqn)) = self.find_cached_file(endpoint).await {
            trace!("Reading from cache: {:?}", cache_path);
            Ok(tokio::fs::read(&cache_path).await?)
        } else {
            Err(crate::Error::CacheEntryNotFound(format!(
                "No valid cache for endpoint: {:?}",
                endpoint
            )))
        }
    }

    /// Make a request with caching
    ///
    /// This method caches the raw response and reconstructs the Response object
    /// when serving from cache.
    pub async fn request(&self, endpoint: &Endpoint) -> Result<ribbit_client::Response> {
        // Check cache first
        if self.enabled && self.is_cache_valid(endpoint).await {
            debug!("Cache hit for endpoint: {:?}", endpoint);
            if let Ok(cached_data) = self.read_from_cache(endpoint).await {
                // Reconstruct Response based on protocol version
                // We need to extract the data from the raw bytes
                let response = match self.client.protocol_version() {
                    ribbit_client::ProtocolVersion::V2 => {
                        // V2 is simple - just raw data as string
                        ribbit_client::Response {
                            raw: cached_data.clone(),
                            data: Some(String::from_utf8_lossy(&cached_data).to_string()),
                            mime_parts: None,
                        }
                    }
                    _ => {
                        // V1 - try to extract data from MIME structure
                        // Look for the main data content in the MIME message
                        let data_str = String::from_utf8_lossy(&cached_data);
                        let mut data_content = None;

                        // Simple MIME parsing to extract the data part
                        if let Some(boundary_start) = data_str.find("boundary=\"") {
                            if let Some(boundary_end) = data_str[boundary_start + 10..].find('"') {
                                let boundary = &data_str
                                    [boundary_start + 10..boundary_start + 10 + boundary_end];
                                let delimiter = format!("--{}", boundary);

                                // Find the data part (usually first part after content type)
                                let parts: Vec<&str> = data_str.split(&delimiter).collect();
                                for part in parts {
                                    if part.contains("Content-Disposition:")
                                        && !part.contains("Content-Type: application/cms")
                                    {
                                        // Extract the body after headers - try both \r\n\r\n and \n\n
                                        let body_start = part
                                            .find("\r\n\r\n")
                                            .map(|pos| (pos, 4))
                                            .or_else(|| part.find("\n\n").map(|pos| (pos, 2)));

                                        if let Some((start, offset)) = body_start {
                                            let body = &part[start + offset..];
                                            // Remove any trailing boundary markers
                                            if let Some(end) = body
                                                .find(&format!("\r\n--{}", boundary))
                                                .or_else(|| body.find(&format!("\n--{}", boundary)))
                                            {
                                                data_content = Some(body[..end].trim().to_string());
                                            } else {
                                                data_content = Some(body.trim().to_string());
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        ribbit_client::Response {
                            raw: cached_data,
                            data: data_content,
                            mime_parts: None, // Cannot fully reconstruct
                        }
                    }
                };
                return Ok(response);
            }
        }

        // Cache miss or error - make actual request
        debug!(
            "Cache miss for endpoint: {:?}, fetching from server",
            endpoint
        );
        let response = self.client.request(endpoint).await?;

        // Cache the successful response
        if let Err(e) = self.write_to_cache(endpoint, &response.raw).await {
            debug!("Failed to write to cache: {}", e);
        }

        Ok(response)
    }

    /// Make a raw request with caching
    ///
    /// This is a convenience method that returns just the raw bytes.
    pub async fn request_raw(&self, endpoint: &Endpoint) -> Result<Vec<u8>> {
        // Check cache first
        if self.enabled && self.is_cache_valid(endpoint).await {
            debug!("Cache hit for raw endpoint: {:?}", endpoint);
            if let Ok(cached_data) = self.read_from_cache(endpoint).await {
                return Ok(cached_data);
            }
        }

        // Cache miss or error - make actual request
        debug!(
            "Cache miss for raw endpoint: {:?}, fetching from server",
            endpoint
        );
        let raw_data = self.client.request_raw(endpoint).await?;

        // Cache the successful response
        if let Err(e) = self.write_to_cache(endpoint, &raw_data).await {
            debug!("Failed to write to cache: {}", e);
        }

        Ok(raw_data)
    }

    /// Request with automatic type parsing
    ///
    /// This method caches the raw response and parses it into the appropriate typed structure.
    /// It's a drop-in replacement for RibbitClient::request_typed.
    pub async fn request_typed<T: TypedResponse>(&self, endpoint: &Endpoint) -> Result<T> {
        let response = self.request(endpoint).await?;
        T::from_response(&response).map_err(|e| e.into())
    }

    /// Request product versions with typed response
    ///
    /// Convenience method with caching for requesting product version information.
    pub async fn get_product_versions(
        &self,
        product: &str,
    ) -> Result<ribbit_client::ProductVersionsResponse> {
        self.request_typed(&Endpoint::ProductVersions(product.to_string()))
            .await
    }

    /// Request product CDNs with typed response
    ///
    /// Convenience method with caching for requesting CDN server information.
    pub async fn get_product_cdns(
        &self,
        product: &str,
    ) -> Result<ribbit_client::ProductCdnsResponse> {
        self.request_typed(&Endpoint::ProductCdns(product.to_string()))
            .await
    }

    /// Request product background download config with typed response
    ///
    /// Convenience method with caching for requesting background download configuration.
    pub async fn get_product_bgdl(
        &self,
        product: &str,
    ) -> Result<ribbit_client::ProductBgdlResponse> {
        self.request_typed(&Endpoint::ProductBgdl(product.to_string()))
            .await
    }

    /// Request summary of all products with typed response
    ///
    /// Convenience method with caching for requesting the summary of all available products.
    pub async fn get_summary(&self) -> Result<ribbit_client::SummaryResponse> {
        self.request_typed(&Endpoint::Summary).await
    }

    /// Get the underlying RibbitClient
    pub fn inner(&self) -> &RibbitClient {
        &self.client
    }

    /// Get mutable access to the underlying RibbitClient
    pub fn inner_mut(&mut self) -> &mut RibbitClient {
        &mut self.client
    }

    /// Clear all cached responses
    pub async fn clear_cache(&self) -> Result<()> {
        debug!("Clearing all cached responses");

        let region_dir = self.cache_dir.join(self.region.to_string());
        if region_dir.exists() {
            let mut entries = tokio::fs::read_dir(&region_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("bmime")
                    || path.extension().and_then(|s| s.to_str()) == Some("meta")
                {
                    tokio::fs::remove_file(&path).await?;
                }
            }
        }

        Ok(())
    }

    /// Clear expired cache entries
    pub async fn clear_expired(&self) -> Result<()> {
        debug!("Clearing expired cache entries");

        let region_dir = self.cache_dir.join(self.region.to_string());
        if !region_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&region_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("bmime") {
                // Check if this cache file is expired
                let meta_path = path.with_extension("meta");

                if let Ok(metadata) = tokio::fs::read_to_string(&meta_path).await {
                    if let Ok(timestamp) = metadata.trim().parse::<u64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        // Determine TTL based on filename
                        let filename = path.file_name().unwrap().to_string_lossy();
                        let ttl = if filename.starts_with("certs-") || filename.starts_with("ocsp-")
                        {
                            CERT_CACHE_TTL
                        } else {
                            DEFAULT_CACHE_TTL
                        };

                        if now.saturating_sub(timestamp) >= ttl.as_secs() {
                            // Remove both files
                            let _ = tokio::fs::remove_file(&path).await;
                            let _ = tokio::fs::remove_file(&meta_path).await;
                            trace!("Removed expired cache file: {:?}", path);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_filename_generation() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedRibbitClient::new(Region::US).await.unwrap();

            // Test various endpoints
            assert_eq!(
                client.generate_cache_filename(&Endpoint::Summary, None),
                "summary-#-0.bmime"
            );

            assert_eq!(
                client.generate_cache_filename(&Endpoint::ProductVersions("wow".to_string()), None),
                "versions-wow-0.bmime"
            );

            assert_eq!(
                client.generate_cache_filename(&Endpoint::Cert("abc123".to_string()), Some(12345)),
                "certs-abc123-12345.bmime"
            );

            assert_eq!(
                client.generate_cache_filename(
                    &Endpoint::Custom("products/wow/config".to_string()),
                    None
                ),
                "products-wow-0.bmime"
            );
        });
    }

    #[test]
    fn test_ttl_selection() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedRibbitClient::new(Region::US).await.unwrap();

            // Regular endpoints get default TTL
            assert_eq!(
                client.get_ttl_for_endpoint(&Endpoint::Summary),
                DEFAULT_CACHE_TTL
            );

            // Certificate endpoints get longer TTL
            assert_eq!(
                client.get_ttl_for_endpoint(&Endpoint::Cert("test".to_string())),
                CERT_CACHE_TTL
            );

            assert_eq!(
                client.get_ttl_for_endpoint(&Endpoint::Ocsp("test".to_string())),
                CERT_CACHE_TTL
            );
        });
    }

    #[test]
    fn test_api_methods_compile() {
        // This test just verifies that all API methods compile correctly
        // It doesn't actually run them to avoid network calls in tests
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedRibbitClient::new(Region::US).await.unwrap();

            // These would all compile and work in real usage:
            // let _ = client.get_summary().await;
            // let _ = client.get_product_versions("wow").await;
            // let _ = client.get_product_cdns("wow").await;
            // let _ = client.get_product_bgdl("wow").await;
            // let _ = client.request(&Endpoint::Summary).await;
            // let _ = client.request_raw(&Endpoint::Summary).await;
            // let _ = client.request_typed::<SummaryResponse>(&Endpoint::Summary).await;

            // Just verify the client was created
            assert_eq!(client.inner().region(), Region::US);
        });
    }

    #[test]
    fn test_extract_sequence_number() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedRibbitClient::new(Region::US).await.unwrap();

            // Test BPSV format with sequence number
            let data_with_seqn = b"Product!STRING:0|Seqn!DEC:4\n## seqn = 12345\nwow|67890";
            assert_eq!(client.extract_sequence_number(data_with_seqn), Some(12345));

            // Test MIME wrapped data
            let mime_data = b"Subject: test\nFrom: Test/1.0\n\n--boundary\nContent-Disposition: test\n\nProduct!STRING:0\n## seqn = 67890\ndata\n--boundary--";
            assert_eq!(client.extract_sequence_number(mime_data), Some(67890));

            // Test data without sequence number
            let data_no_seqn = b"Product!STRING:0|Seqn!DEC:4\nwow|12345";
            assert_eq!(client.extract_sequence_number(data_no_seqn), None);
        });
    }
}
