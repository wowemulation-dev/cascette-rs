//! Fallback client that tries Ribbit first, then TACT
//!
//! This module provides a client that attempts to use the Ribbit protocol
//! first (as it's the primary protocol) and falls back to TACT HTTP if
//! Ribbit fails. Both protocols return identical BPSV data.

use ngdp_cache::{cached_ribbit_client::CachedRibbitClient, cached_tact_client::CachedTactClient};
use ribbit_client::{Endpoint, Region};
use std::fmt;
use tact_client::error::Error as TactError;
use thiserror::Error;
use tracing::{debug, warn};

/// Error type for fallback client operations
#[derive(Error, Debug)]
pub enum FallbackError {
    /// Both Ribbit and TACT failed
    #[error("Both Ribbit and TACT failed: Ribbit: {ribbit_error}, TACT: {tact_error}")]
    BothFailed {
        ribbit_error: String,
        tact_error: String,
    },
    /// Failed to create clients
    #[error("Failed to create clients: {0}")]
    ClientCreation(String),
}

/// Client that provides automatic fallback from Ribbit to TACT
pub struct FallbackClient {
    ribbit_client: CachedRibbitClient,
    tact_client: CachedTactClient,
    region: Region,
    caching_enabled: bool,
}

impl FallbackClient {
    /// Create a new fallback client for the specified region
    pub async fn new(region: Region) -> Result<Self, FallbackError> {
        let ribbit_client = CachedRibbitClient::new(region)
            .await
            .map_err(|e| FallbackError::ClientCreation(format!("Ribbit: {}", e)))?;

        // Convert Ribbit region to TACT region if possible
        let tact_region = match region {
            Region::US => tact_client::Region::US,
            Region::EU => tact_client::Region::EU,
            Region::CN => tact_client::Region::CN,
            Region::KR => tact_client::Region::KR,
            Region::TW => tact_client::Region::TW,
            Region::SG => {
                // TACT doesn't support SG, fall back to US
                tact_client::Region::US
            }
        };

        let tact_client = CachedTactClient::new(tact_region, tact_client::ProtocolVersion::V1)
            .await
            .map_err(|e| FallbackError::ClientCreation(format!("TACT: {}", e)))?;

        Ok(Self {
            ribbit_client,
            tact_client,
            region,
            caching_enabled: true,
        })
    }

    /// Set whether caching is enabled
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.caching_enabled = enabled;
        self.ribbit_client.set_caching_enabled(enabled);
        self.tact_client.set_caching_enabled(enabled);
    }

    /// Make a request using fallback logic
    ///
    /// Attempts to use Ribbit first (primary protocol), falls back to TACT on failure
    pub async fn request(
        &self,
        endpoint: &Endpoint,
    ) -> Result<ribbit_client::Response, FallbackError> {
        // Convert Ribbit endpoint to TACT endpoint string
        let tact_endpoint = match endpoint {
            Endpoint::Summary => {
                // TACT doesn't have a summary endpoint
                return self.ribbit_request(endpoint).await;
            }
            Endpoint::ProductVersions(product) => format!("{}/versions", product),
            Endpoint::ProductCdns(product) => format!("{}/cdns", product),
            Endpoint::ProductBgdl(product) => format!("{}/bgdl", product),
            Endpoint::Cert(_) | Endpoint::Ocsp(_) => {
                // TACT doesn't support certificates
                return self.ribbit_request(endpoint).await;
            }
            Endpoint::Custom(path) => path.clone(),
        };

        // Try Ribbit first
        match self.ribbit_client.request(endpoint).await {
            Ok(response) => {
                debug!("Successfully retrieved data from Ribbit for {:?}", endpoint);
                Ok(response)
            }
            Err(ribbit_err) => {
                warn!(
                    "Ribbit request failed for {:?}: {}, trying TACT fallback",
                    endpoint, ribbit_err
                );

                // Try TACT fallback
                match self.tact_request(&tact_endpoint).await {
                    Ok(data) => {
                        debug!(
                            "Successfully retrieved data from TACT for {}",
                            tact_endpoint
                        );
                        // Convert TACT response to Ribbit Response format
                        Ok(ribbit_client::Response {
                            raw: data.as_bytes().to_vec(),
                            data: Some(data),
                            mime_parts: None,
                        })
                    }
                    Err(tact_err) => {
                        warn!(
                            "TACT request also failed for {}: {}",
                            tact_endpoint, tact_err
                        );
                        Err(FallbackError::BothFailed {
                            ribbit_error: ribbit_err.to_string(),
                            tact_error: tact_err.to_string(),
                        })
                    }
                }
            }
        }
    }

    /// Make a typed request using fallback logic
    pub async fn request_typed<T: ribbit_client::TypedResponse>(
        &self,
        endpoint: &Endpoint,
    ) -> Result<T, FallbackError> {
        let response = self.request(endpoint).await?;
        T::from_response(&response).map_err(|e| FallbackError::BothFailed {
            ribbit_error: format!("Failed to parse response: {}", e),
            tact_error: "Not attempted".to_string(),
        })
    }

    /// Direct Ribbit request (no fallback)
    async fn ribbit_request(
        &self,
        endpoint: &Endpoint,
    ) -> Result<ribbit_client::Response, FallbackError> {
        self.ribbit_client
            .request(endpoint)
            .await
            .map_err(|e| FallbackError::BothFailed {
                ribbit_error: e.to_string(),
                tact_error: "Not applicable for this endpoint".to_string(),
            })
    }

    /// Direct TACT request
    async fn tact_request(&self, endpoint: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Extract product from endpoint (format: "product/endpoint")
        let parts: Vec<&str> = endpoint.split('/').collect();
        if parts.len() != 2 {
            return Err(Box::new(TactError::InvalidManifest {
                line: 0,
                reason: format!("Invalid endpoint format: {}", endpoint),
            }));
        }

        let product = parts[0];
        let endpoint_type = parts[1];

        // Get the raw response and extract text
        let response = match endpoint_type {
            "versions" => self.tact_client.get_versions(product).await?,
            "cdns" => self.tact_client.get_cdns(product).await?,
            "bgdl" => self.tact_client.get_bgdl(product).await?,
            _ => {
                return Err(Box::new(TactError::InvalidManifest {
                    line: 0,
                    reason: format!("Unknown endpoint type: {}", endpoint_type),
                }));
            }
        };

        Ok(response.text().await?)
    }

    /// Clear expired cache entries for both clients
    pub async fn clear_expired(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.ribbit_client.clear_expired().await?;
        self.tact_client.clear_expired().await?;
        Ok(())
    }

    /// Clear all cache entries for both clients
    pub async fn clear_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.ribbit_client.clear_cache().await?;
        self.tact_client.clear_cache().await?;
        Ok(())
    }
}

impl fmt::Debug for FallbackClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FallbackClient")
            .field("region", &self.region)
            .field("caching_enabled", &self.caching_enabled)
            .finish()
    }
}
