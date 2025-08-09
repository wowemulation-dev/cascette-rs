//! Hybrid version discovery client that prioritizes HTTP over legacy Ribbit
//!
//! This module provides a client that follows modern NGDP best practices:
//! 1. Primary: HTTPS endpoints (<https://us.version.battle.net/wow/versions>)
//! 2. Fallback: Legacy Ribbit TCP protocol (:1119) for backward compatibility
//!
//! # Example
//!
//! ```no_run
//! use ngdp_cache::hybrid_version_client::HybridVersionClient;
//! use ribbit_client::Region;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a hybrid client that prioritizes HTTP
//! let client = HybridVersionClient::new(Region::US).await?;
//!
//! // This will try HTTPS first, fallback to Ribbit if needed
//! let versions = client.get_product_versions("wow").await?;
//! println!("Found {} versions", versions.entries.len());
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

use ribbit_client::{ProductCdnsResponse, ProductVersionsResponse, Region as RibbitRegion};
use tact_client::Region as TactRegion;
use tact_client::http::{HttpClient, ProtocolVersion};

use crate::{Result, ensure_dir, get_cache_dir};

/// Default TTL for HTTP responses (5 minutes)
#[allow(dead_code)]
const DEFAULT_HTTP_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// Default TTL for Ribbit fallback responses (2 minutes - shorter due to being fallback)
#[allow(dead_code)]
const DEFAULT_RIBBIT_CACHE_TTL: Duration = Duration::from_secs(2 * 60);

/// Hybrid version discovery client that prioritizes modern HTTP endpoints
pub struct HybridVersionClient {
    /// Primary HTTP client (V2 protocol)
    http_client: HttpClient,
    /// Fallback Ribbit client (legacy)
    ribbit_client: Option<crate::cached_ribbit_client::CachedRibbitClient>,
    /// Base directory for cache
    #[allow(dead_code)]
    cache_dir: PathBuf,
    /// Region for this client
    region: RibbitRegion,
    /// Whether to use Ribbit fallback
    enable_ribbit_fallback: bool,
}

impl HybridVersionClient {
    /// Create a new hybrid version discovery client
    pub async fn new(region: RibbitRegion) -> Result<Self> {
        // Convert ribbit region to tact region
        let tact_region = convert_ribbit_to_tact_region(region)?;
        // Primary HTTP client using modern HTTPS endpoints
        let http_client = HttpClient::new(tact_region, ProtocolVersion::V2)
            .map_err(crate::Error::TactClient)?
            .with_max_retries(2)
            .with_user_agent("cascette-rs/0.3.1");

        // Fallback Ribbit client
        let ribbit_client = match crate::cached_ribbit_client::CachedRibbitClient::new(region).await
        {
            Ok(client) => Some(client),
            Err(e) => {
                warn!("Failed to create Ribbit fallback client: {}", e);
                None
            }
        };

        let cache_dir = get_cache_dir()?.join("hybrid");
        ensure_dir(&cache_dir).await?;

        let has_ribbit_fallback = ribbit_client.is_some();
        info!(
            "Initialized hybrid version client for region {:?} (HTTP primary, Ribbit fallback: {})",
            region, has_ribbit_fallback
        );

        Ok(Self {
            http_client,
            ribbit_client,
            cache_dir,
            region,
            enable_ribbit_fallback: has_ribbit_fallback,
        })
    }

    /// Create a new hybrid client with HTTP-only (no Ribbit fallback)
    pub async fn http_only(region: RibbitRegion) -> Result<Self> {
        let tact_region = convert_ribbit_to_tact_region(region)?;
        let http_client = HttpClient::new(tact_region, ProtocolVersion::V2)
            .map_err(crate::Error::TactClient)?
            .with_max_retries(3)
            .with_user_agent("cascette-rs/0.3.1");

        let cache_dir = get_cache_dir()?.join("hybrid");
        ensure_dir(&cache_dir).await?;

        info!(
            "Initialized HTTP-only version client for region {:?}",
            region
        );

        Ok(Self {
            http_client,
            ribbit_client: None,
            cache_dir,
            region,
            enable_ribbit_fallback: false,
        })
    }

    /// Enable or disable Ribbit fallback
    pub fn set_ribbit_fallback(&mut self, enabled: bool) {
        self.enable_ribbit_fallback = enabled && self.ribbit_client.is_some();
    }

    /// Get product versions with HTTP-first, Ribbit-fallback strategy
    pub async fn get_product_versions(&self, product: &str) -> Result<ProductVersionsResponse> {
        debug!(
            "Getting product versions for '{}' using hybrid approach",
            product
        );

        // Try HTTP first (primary method)
        match self.try_http_versions(product).await {
            Ok(response) => {
                info!("✓ Product versions retrieved via HTTPS for '{}'", product);
                return Ok(response);
            }
            Err(e) => {
                warn!("✗ HTTP version discovery failed for '{}': {}", product, e);
                debug!("HTTP error details: {:?}", e);
            }
        }

        // Fallback to Ribbit if enabled
        if self.enable_ribbit_fallback {
            if let Some(ref ribbit_client) = self.ribbit_client {
                debug!("Falling back to Ribbit for product versions: '{}'", product);
                match ribbit_client.get_product_versions(product).await {
                    Ok(response) => {
                        info!(
                            "✓ Product versions retrieved via Ribbit fallback for '{}'",
                            product
                        );
                        return Ok(response);
                    }
                    Err(e) => {
                        warn!("✗ Ribbit fallback also failed for '{}': {}", product, e);
                    }
                }
            }
        }

        Err(crate::Error::Network(format!(
            "Both HTTP and Ribbit failed for product versions: {product}"
        )))
    }

    /// Get CDN configuration with HTTP-first, Ribbit-fallback strategy
    pub async fn get_product_cdns(&self, product: &str) -> Result<ProductCdnsResponse> {
        debug!(
            "Getting CDN configuration for '{}' using hybrid approach",
            product
        );

        // Try HTTP first (primary method)
        match self.try_http_cdns(product).await {
            Ok(response) => {
                info!("✓ CDN configuration retrieved via HTTPS for '{}'", product);
                return Ok(response);
            }
            Err(e) => {
                warn!("✗ HTTP CDN discovery failed for '{}': {}", product, e);
                debug!("HTTP error details: {:?}", e);
            }
        }

        // Fallback to Ribbit if enabled
        if self.enable_ribbit_fallback {
            if let Some(ref ribbit_client) = self.ribbit_client {
                debug!(
                    "Falling back to Ribbit for CDN configuration: '{}'",
                    product
                );
                match ribbit_client.get_product_cdns(product).await {
                    Ok(response) => {
                        info!(
                            "✓ CDN configuration retrieved via Ribbit fallback for '{}'",
                            product
                        );
                        return Ok(response);
                    }
                    Err(e) => {
                        warn!("✗ Ribbit fallback also failed for '{}': {}", product, e);
                    }
                }
            }
        }

        Err(crate::Error::Network(format!(
            "Both HTTP and Ribbit failed for CDN configuration: {product}"
        )))
    }

    /// Try to get versions via HTTP
    async fn try_http_versions(&self, product: &str) -> Result<ProductVersionsResponse> {
        let versions = self
            .http_client
            .get_product_versions_http_parsed(product)
            .await
            .map_err(crate::Error::TactClient)?;

        // Convert to ribbit_client format for compatibility
        Ok(ProductVersionsResponse {
            sequence_number: None, // HTTP responses don't have sequence numbers
            entries: versions
                .into_iter()
                .map(|v| ribbit_client::VersionEntry {
                    region: v.region,
                    build_config: v.build_config,
                    cdn_config: v.cdn_config,
                    key_ring: v.key_ring,
                    build_id: v.build_id,
                    versions_name: v.versions_name,
                    product_config: v.product_config,
                })
                .collect(),
        })
    }

    /// Try to get CDNs via HTTP
    async fn try_http_cdns(&self, product: &str) -> Result<ProductCdnsResponse> {
        let cdns = self
            .http_client
            .get_product_cdns_http_parsed(product)
            .await
            .map_err(crate::Error::TactClient)?;

        // Convert to ribbit_client format for compatibility
        Ok(ProductCdnsResponse {
            sequence_number: None, // HTTP responses don't have sequence numbers
            entries: cdns
                .into_iter()
                .map(|c| ribbit_client::CdnEntry {
                    name: c.name,
                    path: c.path,
                    hosts: c.hosts,
                    servers: Vec::new(), // HTTP CDN entries don't have separate servers field
                    config_path: c.config_path,
                })
                .collect(),
        })
    }

    /// Get the current region
    pub fn region(&self) -> RibbitRegion {
        self.region
    }

    /// Check if Ribbit fallback is available and enabled
    pub fn has_ribbit_fallback(&self) -> bool {
        self.enable_ribbit_fallback && self.ribbit_client.is_some()
    }
}

/// Convert ribbit region to tact region
fn convert_ribbit_to_tact_region(region: RibbitRegion) -> Result<TactRegion> {
    match region {
        RibbitRegion::US => Ok(TactRegion::US),
        RibbitRegion::EU => Ok(TactRegion::EU),
        RibbitRegion::CN => Ok(TactRegion::CN),
        RibbitRegion::KR => Ok(TactRegion::KR),
        RibbitRegion::TW => Ok(TactRegion::TW),
        RibbitRegion::SG => Err(crate::Error::Network(
            "Singapore region not supported by TACT client".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hybrid_client_creation() {
        let client = HybridVersionClient::new(RibbitRegion::US).await;
        assert!(client.is_ok(), "Should create hybrid client successfully");

        let client = client.unwrap();
        assert_eq!(client.region(), RibbitRegion::US);
    }

    #[tokio::test]
    async fn test_http_only_client() {
        let client = HybridVersionClient::http_only(RibbitRegion::EU).await;
        assert!(
            client.is_ok(),
            "Should create HTTP-only client successfully"
        );

        let client = client.unwrap();
        assert_eq!(client.region(), RibbitRegion::EU);
        assert!(
            !client.has_ribbit_fallback(),
            "HTTP-only client should not have Ribbit fallback"
        );
    }
}
