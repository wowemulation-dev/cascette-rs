//! Cached wrapper for RibbitClient
//!
//! This module provides a caching layer for RibbitClient that stores responses
//! using the Blizzard MIME filename convention: command-argument(s)-sequencenumber.bmime

use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, trace};

use ribbit_client::{Endpoint, Region, RibbitClient};

use crate::{ensure_dir, get_cache_dir, Result};

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

    /// Check if a cached response is still valid
    async fn is_cache_valid(&self, endpoint: &Endpoint, sequence_number: Option<u64>) -> bool {
        if !self.enabled {
            return false;
        }

        let meta_path = self.get_metadata_path(endpoint, sequence_number);
        let ttl = self.get_ttl_for_endpoint(endpoint);

        if let Ok(metadata) = tokio::fs::read_to_string(&meta_path).await {
            if let Ok(timestamp) = metadata.trim().parse::<u64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                return (now - timestamp) < ttl.as_secs();
            }
        }

        false
    }

    /// Write raw response to cache
    async fn write_to_cache(&self, endpoint: &Endpoint, raw_data: &[u8]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let cache_path = self.get_cache_path(endpoint, None); // TODO: Extract sequence number
        let meta_path = self.get_metadata_path(endpoint, None);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            ensure_dir(parent).await?;
        }

        // Write the raw response data
        trace!("Writing {} bytes to cache: {:?}", raw_data.len(), cache_path);
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
        let cache_path = self.get_cache_path(endpoint, None);
        trace!("Reading from cache: {:?}", cache_path);
        Ok(tokio::fs::read(&cache_path).await?)
    }

    /// Make a raw request with caching
    ///
    /// This is the primary method for cached requests. It returns raw response bytes
    /// which can be parsed by the caller.
    pub async fn request_raw(&self, endpoint: &Endpoint) -> Result<Vec<u8>> {
        // Check cache first
        if self.enabled && self.is_cache_valid(endpoint, None).await {
            debug!("Cache hit for raw endpoint: {:?}", endpoint);
            if let Ok(cached_data) = self.read_from_cache(endpoint).await {
                return Ok(cached_data);
            }
        }

        // Cache miss or error - make actual request
        debug!("Cache miss for raw endpoint: {:?}, fetching from server", endpoint);
        let raw_data = self.client.request_raw(endpoint).await?;

        // Cache the successful response
        if let Err(e) = self.write_to_cache(endpoint, &raw_data).await {
            debug!("Failed to write to cache: {}", e);
        }

        Ok(raw_data)
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
                    || path.extension().and_then(|s| s.to_str()) == Some("meta") {
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
                        let ttl = if filename.starts_with("certs-") || filename.starts_with("ocsp-") {
                            CERT_CACHE_TTL
                        } else {
                            DEFAULT_CACHE_TTL
                        };
                        
                        if (now - timestamp) >= ttl.as_secs() {
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
                client.generate_cache_filename(&Endpoint::Custom("products/wow/config".to_string()), None),
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
}