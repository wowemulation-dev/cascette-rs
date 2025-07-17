//! CDN client with automatic fallback support
//!
//! This module provides a CDN client that supports multiple CDN hosts
//! with automatic fallback when a primary CDN fails.

use crate::{CdnClient, Error, Result};
use reqwest::Response;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Default backup CDN servers
const DEFAULT_BACKUP_CDNS: &[&str] = &["cdn.arctium.tools", "tact.mirror.reliquaryhq.com"];

/// CDN client with automatic fallback to backup servers
///
/// This client wraps the base `CdnClient` and adds support for multiple
/// CDN hosts with automatic fallback. When a download fails from one CDN,
/// it automatically tries the next one in the list.
///
/// # CDN Priority Order
///
/// 1. **Primary CDNs** (Blizzard servers) - All primary CDNs are tried first
/// 2. **Backup CDNs** (Community mirrors) - Only tried after all primary CDNs fail
///
/// # Example
///
/// ```no_run
/// use ngdp_cdn::CdnClientWithFallback;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create with default backup CDNs
/// let client = CdnClientWithFallback::new()?;
///
/// // Add Blizzard CDNs (these will be tried first)
/// client.add_primary_cdns(vec![
///     "blzddist1-a.akamaihd.net",
///     "level3.blizzard.com",
///     "blzddist2-a.akamaihd.net",
/// ]);
///
/// // Download will try all Blizzard CDNs first, then community backups
/// let response = client.download(
///     "tpr/wow",
///     "2e9c1e3b5f5a0c9d9e8f1234567890ab",
///     "",
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CdnClientWithFallback {
    /// Base CDN client for making requests
    client: Arc<CdnClient>,
    /// List of CDN hosts to try in order
    cdn_hosts: Arc<parking_lot::RwLock<Vec<String>>>,
    /// Whether to use default backup CDNs
    use_default_backups: bool,
    /// Custom CDN hosts to use after community CDNs
    custom_cdn_hosts: Arc<parking_lot::RwLock<Vec<String>>>,
}

impl CdnClientWithFallback {
    /// Create a new CDN client with default backup CDNs
    pub fn new() -> Result<Self> {
        let client = CdnClient::new()?;
        Ok(Self::with_client(client))
    }

    /// Create a new CDN client with a custom base client
    pub fn with_client(client: CdnClient) -> Self {
        Self {
            client: Arc::new(client),
            cdn_hosts: Arc::new(parking_lot::RwLock::new(Vec::new())),
            use_default_backups: true,
            custom_cdn_hosts: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    /// Create a builder for configuring the CDN client
    pub fn builder() -> CdnClientWithFallbackBuilder {
        CdnClientWithFallbackBuilder::new()
    }

    /// Add a primary CDN host
    ///
    /// Primary CDNs are tried before backup CDNs
    pub fn add_primary_cdn(&self, host: impl Into<String>) {
        let mut hosts = self.cdn_hosts.write();
        let host = host.into();
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }

    /// Add multiple primary CDN hosts
    pub fn add_primary_cdns(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        let mut cdn_hosts = self.cdn_hosts.write();
        for host in hosts {
            let host = host.into();
            if !cdn_hosts.contains(&host) {
                cdn_hosts.push(host);
            }
        }
    }

    /// Set primary CDN hosts, replacing any existing ones
    pub fn set_primary_cdns(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        let mut cdn_hosts = self.cdn_hosts.write();
        cdn_hosts.clear();
        for host in hosts {
            cdn_hosts.push(host.into());
        }
    }

    /// Clear all CDN hosts (primary, backup, and custom)
    pub fn clear_cdns(&self) {
        self.cdn_hosts.write().clear();
        self.custom_cdn_hosts.write().clear();
    }

    /// Get the list of all CDN hosts with Blizzard CDNs first, then backups, then custom
    pub fn get_all_cdn_hosts(&self) -> Vec<String> {
        let primary_hosts = self.cdn_hosts.read().clone();
        let mut all_hosts = primary_hosts;

        // Add backup CDNs after all primary CDNs
        if self.use_default_backups {
            for backup in DEFAULT_BACKUP_CDNS {
                if !all_hosts.contains(&backup.to_string()) {
                    all_hosts.push(backup.to_string());
                }
            }
        }

        // Add custom CDNs last, after community CDNs
        let custom_hosts = self.custom_cdn_hosts.read();
        for custom_host in custom_hosts.iter() {
            if !all_hosts.contains(custom_host) {
                all_hosts.push(custom_host.clone());
            }
        }

        all_hosts
    }

    /// Set whether to use default backup CDNs
    pub fn set_use_default_backups(&mut self, use_backups: bool) {
        self.use_default_backups = use_backups;
    }

    /// Add a custom CDN host
    ///
    /// Custom CDNs are tried after primary and community CDNs
    pub fn add_custom_cdn(&self, host: impl Into<String>) {
        let mut hosts = self.custom_cdn_hosts.write();
        let host = host.into();
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }

    /// Add multiple custom CDN hosts
    pub fn add_custom_cdns(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        let mut cdn_hosts = self.custom_cdn_hosts.write();
        for host in hosts {
            let host = host.into();
            if !cdn_hosts.contains(&host) {
                cdn_hosts.push(host);
            }
        }
    }

    /// Set custom CDN hosts, replacing any existing ones
    pub fn set_custom_cdns(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        let mut cdn_hosts = self.custom_cdn_hosts.write();
        cdn_hosts.clear();
        for host in hosts {
            cdn_hosts.push(host.into());
        }
    }

    /// Clear all custom CDN hosts
    pub fn clear_custom_cdns(&self) {
        self.custom_cdn_hosts.write().clear();
    }

    /// Download content from CDN by hash, trying each CDN host in order
    pub async fn download(&self, path: &str, hash: &str, suffix: &str) -> Result<Response> {
        let hosts = self.get_all_cdn_hosts();

        if hosts.is_empty() {
            return Err(Error::invalid_host("No CDN hosts configured"));
        }

        let mut last_error = None;

        for (index, cdn_host) in hosts.iter().enumerate() {
            debug!(
                "Attempting download from CDN {} of {}: {}",
                index + 1,
                hosts.len(),
                cdn_host
            );

            match self.client.download(cdn_host, path, hash, suffix).await {
                Ok(response) => {
                    if index > 0 {
                        info!(
                            "Successfully downloaded from backup CDN: {} (attempt {})",
                            cdn_host,
                            index + 1
                        );
                    }
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Failed to download from CDN {}: {}", cdn_host, e);
                    last_error = Some(e);
                }
            }
        }

        // All CDNs failed
        Err(last_error.unwrap_or_else(Error::cdn_exhausted))
    }

    /// Download BuildConfig from CDN
    pub async fn download_build_config(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download CDNConfig from CDN
    pub async fn download_cdn_config(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download ProductConfig from CDN
    pub async fn download_product_config(&self, config_path: &str, hash: &str) -> Result<Response> {
        self.download(config_path, hash, "").await
    }

    /// Download KeyRing from CDN
    pub async fn download_key_ring(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download data file from CDN
    pub async fn download_data(&self, path: &str, hash: &str) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, "").await
    }

    /// Download data index file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}.index`
    pub async fn download_data_index(&self, path: &str, hash: &str) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, ".index").await
    }

    /// Download patch file from CDN
    pub async fn download_patch(&self, path: &str, hash: &str) -> Result<Response> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(&patch_path, hash, "").await
    }

    /// Download content and stream it to a writer
    ///
    /// Note: Due to the nature of fallback retries, this method downloads to a temporary
    /// buffer first, then writes to the provided writer. For true streaming without
    /// buffering, use the base `CdnClient` directly with a specific CDN host.
    pub async fn download_streaming<W>(
        &self,
        path: &str,
        hash: &str,
        suffix: &str,
        mut writer: W,
    ) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;

        // Download to memory first since we need to retry with different CDNs
        let response = self.download(path, hash, suffix).await?;
        let bytes = response.bytes().await?;

        writer
            .write_all(&bytes)
            .await
            .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;
        writer
            .flush()
            .await
            .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;

        Ok(bytes.len() as u64)
    }
}

impl Default for CdnClientWithFallback {
    fn default() -> Self {
        Self::new().expect("Failed to create default CDN client")
    }
}

/// Builder for configuring CDN client with fallback
#[derive(Debug, Clone)]
pub struct CdnClientWithFallbackBuilder {
    base_client_builder: crate::CdnClientBuilder,
    primary_cdns: Vec<String>,
    use_default_backups: bool,
    custom_cdns: Vec<String>,
}

impl CdnClientWithFallbackBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            base_client_builder: crate::CdnClient::builder(),
            primary_cdns: Vec::new(),
            use_default_backups: true,
            custom_cdns: Vec::new(),
        }
    }

    /// Configure the base CDN client
    pub fn configure_base_client<F>(mut self, f: F) -> Self
    where
        F: FnOnce(crate::CdnClientBuilder) -> crate::CdnClientBuilder,
    {
        self.base_client_builder = f(self.base_client_builder);
        self
    }

    /// Add a primary CDN host
    pub fn add_primary_cdn(mut self, host: impl Into<String>) -> Self {
        self.primary_cdns.push(host.into());
        self
    }

    /// Add multiple primary CDN hosts
    pub fn add_primary_cdns(mut self, hosts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for host in hosts {
            self.primary_cdns.push(host.into());
        }
        self
    }

    /// Set whether to use default backup CDNs
    pub fn use_default_backups(mut self, use_backups: bool) -> Self {
        self.use_default_backups = use_backups;
        self
    }

    /// Add a custom CDN host
    pub fn add_custom_cdn(mut self, host: impl Into<String>) -> Self {
        self.custom_cdns.push(host.into());
        self
    }

    /// Add multiple custom CDN hosts
    pub fn add_custom_cdns(mut self, hosts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for host in hosts {
            self.custom_cdns.push(host.into());
        }
        self
    }

    /// Build the CDN client
    pub fn build(self) -> Result<CdnClientWithFallback> {
        let base_client = self.base_client_builder.build()?;
        let mut client = CdnClientWithFallback::with_client(base_client);

        client.set_use_default_backups(self.use_default_backups);
        client.set_primary_cdns(self.primary_cdns);
        client.set_custom_cdns(self.custom_cdns);

        Ok(client)
    }
}

impl Default for CdnClientWithFallbackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_backup_cdns() {
        let client = CdnClientWithFallback::new().unwrap();
        let hosts = client.get_all_cdn_hosts();

        // Should have the default backup CDNs
        assert_eq!(hosts.len(), 2);
        assert!(hosts.contains(&"cdn.arctium.tools".to_string()));
        assert!(hosts.contains(&"tact.mirror.reliquaryhq.com".to_string()));
    }

    #[test]
    fn test_add_primary_cdn() {
        let client = CdnClientWithFallback::new().unwrap();
        client.add_primary_cdn("primary.example.com");

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 3);
        // Primary CDN should be first
        assert_eq!(hosts[0], "primary.example.com");
        // Backup CDNs should be at the end
        assert_eq!(hosts[1], "cdn.arctium.tools");
        assert_eq!(hosts[2], "tact.mirror.reliquaryhq.com");
    }

    #[test]
    fn test_primary_cdns_before_backups() {
        let client = CdnClientWithFallback::new().unwrap();

        // Add multiple primary CDNs
        client.add_primary_cdns(vec![
            "blzddist1-a.akamaihd.net",
            "level3.blizzard.com",
            "blzddist2-a.akamaihd.net",
        ]);

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 5);

        // All Blizzard CDNs should come first
        assert_eq!(hosts[0], "blzddist1-a.akamaihd.net");
        assert_eq!(hosts[1], "level3.blizzard.com");
        assert_eq!(hosts[2], "blzddist2-a.akamaihd.net");

        // Community backups should come last
        assert_eq!(hosts[3], "cdn.arctium.tools");
        assert_eq!(hosts[4], "tact.mirror.reliquaryhq.com");
    }

    #[test]
    fn test_disable_default_backups() {
        let mut client = CdnClientWithFallback::new().unwrap();
        client.set_use_default_backups(false);
        client.add_primary_cdn("primary.example.com");

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0], "primary.example.com");
    }

    #[test]
    fn test_builder_configuration() {
        let client = CdnClientWithFallback::builder()
            .add_primary_cdn("cdn1.example.com")
            .add_primary_cdn("cdn2.example.com")
            .use_default_backups(false)
            .configure_base_client(|builder| builder.max_retries(5).initial_backoff_ms(200))
            .build()
            .unwrap();

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0], "cdn1.example.com");
        assert_eq!(hosts[1], "cdn2.example.com");
    }

    #[test]
    fn test_no_duplicate_hosts() {
        let client = CdnClientWithFallback::new().unwrap();
        client.add_primary_cdn("cdn.arctium.tools");
        client.add_primary_cdn("other.example.com");

        let hosts = client.get_all_cdn_hosts();
        // Should not duplicate cdn.arctium.tools
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0], "cdn.arctium.tools");
        assert_eq!(hosts[1], "other.example.com");
        assert_eq!(hosts[2], "tact.mirror.reliquaryhq.com");
    }
}
