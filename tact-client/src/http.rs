//! HTTP client for TACT protocol

use crate::{CdnEntry, Error, Region, Result, VersionEntry, response_types};
use reqwest::{Client, Response};
use std::time::Duration;
use tracing::{debug, trace};

/// TACT protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    /// Version 1: TCP-based protocol on port 1119
    V1,
    /// Version 2: HTTPS-based REST API
    V2,
}

/// HTTP client for TACT protocol
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    region: Region,
    version: ProtocolVersion,
}

impl HttpClient {
    /// Create a new HTTP client for the specified region and protocol version
    pub fn new(region: Region, version: ProtocolVersion) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            region,
            version,
        })
    }

    /// Create a new HTTP client with custom reqwest client
    pub fn with_client(client: Client, region: Region, version: ProtocolVersion) -> Self {
        Self {
            client,
            region,
            version,
        }
    }

    /// Get the base URL for the current configuration
    pub fn base_url(&self) -> String {
        match self.version {
            ProtocolVersion::V1 => {
                format!("http://{}.patch.battle.net:1119", self.region)
            }
            ProtocolVersion::V2 => {
                format!("https://{}.version.battle.net/v2/products", self.region)
            }
        }
    }

    /// Get the current region
    pub fn region(&self) -> Region {
        self.region
    }

    /// Get the current protocol version
    pub fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Set the region
    pub fn set_region(&mut self, region: Region) {
        self.region = region;
    }

    /// Get versions manifest for a product (V1 protocol)
    pub async fn get_versions(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/versions", self.base_url(), product);
        debug!("Fetching versions from: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Get CDN configuration for a product (V1 protocol)
    pub async fn get_cdns(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/cdns", self.base_url(), product);
        debug!("Fetching CDNs from: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Get BGDL manifest for a product (V1 protocol)
    pub async fn get_bgdl(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/bgdl", self.base_url(), product);
        debug!("Fetching BGDL from: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Get product summary (V2 protocol)
    pub async fn get_summary(&self) -> Result<Response> {
        if self.version != ProtocolVersion::V2 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = self.base_url();
        debug!("Fetching summary from: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Get product details (V2 protocol)
    pub async fn get_product(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V2 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}", self.base_url(), product);
        debug!("Fetching product details from: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Make a raw GET request to a path
    pub async fn get(&self, path: &str) -> Result<Response> {
        let url = if path.starts_with('/') {
            format!("{}{}", self.base_url(), path)
        } else {
            format!("{}/{}", self.base_url(), path)
        };

        debug!("GET request to: {}", url);

        let response = self.client.get(&url).send().await?;
        trace!("Response status: {}", response.status());

        Ok(response)
    }

    /// Download a file from CDN
    pub async fn download_file(&self, cdn_host: &str, path: &str, hash: &str) -> Result<Response> {
        let url = format!(
            "http://{}/{}/{}/{}/{}",
            cdn_host,
            path,
            &hash[0..2],
            &hash[2..4],
            hash
        );

        debug!("Downloading file from: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status() == 404 {
            return Err(Error::file_not_found(hash));
        }

        trace!("Download response status: {}", response.status());

        Ok(response)
    }

    /// Get parsed versions manifest for a product
    pub async fn get_versions_parsed(&self, product: &str) -> Result<Vec<VersionEntry>> {
        let response = self.get_versions(product).await?;
        let text = response.text().await?;
        response_types::parse_versions(&text)
    }

    /// Get parsed CDN manifest for a product
    pub async fn get_cdns_parsed(&self, product: &str) -> Result<Vec<CdnEntry>> {
        let response = self.get_cdns(product).await?;
        let text = response.text().await?;
        response_types::parse_cdns(&text)
    }

    /// Get parsed BGDL manifest for a product
    pub async fn get_bgdl_parsed(&self, product: &str) -> Result<Vec<response_types::BgdlEntry>> {
        let response = self.get_bgdl(product).await?;
        let text = response.text().await?;
        response_types::parse_bgdl(&text)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(Region::US, ProtocolVersion::V1).expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_v1() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert_eq!(client.base_url(), "http://us.patch.battle.net:1119");

        let client = HttpClient::new(Region::EU, ProtocolVersion::V1).unwrap();
        assert_eq!(client.base_url(), "http://eu.patch.battle.net:1119");
    }

    #[test]
    fn test_base_url_v2() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V2).unwrap();
        assert_eq!(
            client.base_url(),
            "https://us.version.battle.net/v2/products"
        );

        let client = HttpClient::new(Region::EU, ProtocolVersion::V2).unwrap();
        assert_eq!(
            client.base_url(),
            "https://eu.version.battle.net/v2/products"
        );
    }

    #[test]
    fn test_region_setting() {
        let mut client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert_eq!(client.region(), Region::US);

        client.set_region(Region::EU);
        assert_eq!(client.region(), Region::EU);
        assert_eq!(client.base_url(), "http://eu.patch.battle.net:1119");
    }
}
