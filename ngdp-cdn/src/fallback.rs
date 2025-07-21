//! CDN client with automatic fallback support
//!
//! This module provides a CDN client that supports multiple CDN hosts
//! with automatic fallback when a primary CDN fails.

use crate::{
    CdnClient, CdnClientBuilderTrait, CdnClientTrait, Error, FallbackError,
    traits::{CdnClientTrait as _, FallbackCdnClientTrait},
};
use reqwest::Response;
use std::{fmt::Debug, marker::PhantomData, sync::Arc};
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
pub struct CdnClientWithFallback<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    /// Base CDN client for making requests
    client: Arc<T>,
    /// List of CDN hosts to try in order
    cdn_hosts: Arc<parking_lot::RwLock<Vec<String>>>,
    /// Whether to use default backup CDNs
    use_default_backups: bool,
    /// Custom CDN hosts to use after community CDNs
    custom_cdn_hosts: Arc<parking_lot::RwLock<Vec<String>>>,
}

impl<T> CdnClientWithFallback<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    /// Create a new CDN client with default backup CDNs
    pub async fn new() -> Result<Self, <T as CdnClientTrait>::Error> {
        let client = T::new().await?;
        Ok(Self::with_client(client))
    }

    /// Create a new CDN client with a custom base client
    pub fn with_client(client: T) -> Self {
        Self {
            client: Arc::new(client),
            cdn_hosts: Arc::new(parking_lot::RwLock::new(Vec::new())),
            use_default_backups: true,
            custom_cdn_hosts: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    // /// Create a builder for configuring the CDN client
    // pub fn builder<U>() -> CdnClientWithFallbackBuilder<U, T>
    // where
    //     U: CdnClientBuilderTrait<Client = CdnClientWithFallback<T>, Error = Error>,
    // {
    //     CdnClientWithFallbackBuilder::new()
    // }

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

    // Removed: using a temporary file works better
    // /// Download content and stream it to a writer
    // ///
    // /// Note: Due to the nature of fallback retries, this method downloads to a temporary
    // /// buffer first, then writes to the provided writer. For true streaming without
    // /// buffering, use the base `CdnClient` directly with a specific CDN host.
    // pub async fn download_streaming<W>(
    //     &self,
    //     path: &str,
    //     hash: &str,
    //     suffix: &str,
    //     mut writer: W,
    // ) -> Result<u64, T::Error>
    // where
    //     W: tokio::io::AsyncWrite + Unpin,
    // {
    //     use tokio::io::AsyncWriteExt;

    //     // Download to memory first since we need to retry with different CDNs
    //     let response = self.download(path, hash, suffix).await?;
    //     let bytes = response.bytes().await?;

    //     writer
    //         .write_all(&bytes)
    //         .await
    //         .map_err(|e| FallbackError::invalid_response(format!("Write error: {e}")))?;
    //     writer
    //         .flush()
    //         .await
    //         .map_err(|e| FallbackError::invalid_response(format!("Write error: {e}")))?;

    //     Ok(bytes.len() as u64)
    // }
}

// impl<T> Default for CdnClientWithFallback<T>
// where
//     T: CdnClientTrait,
//     <T as CdnClientTrait>::Error: FallbackError,
// {
//     fn default() -> Self {
//         Self::new().expect("Failed to create default CDN client")
//     }
// }

impl<T> FallbackCdnClientTrait for CdnClientWithFallback<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    type Response = T::Response;
    type Error = T::Error;
    type Builder = CdnClientWithFallbackBuilder<T>;

    /// Create a new CDN client with default backup CDNs
    async fn new() -> Result<Self, Self::Error> {
        let client = T::new().await?;
        Ok(Self::with_client(client))
    }

    /// Download content from CDN by hash, trying each CDN host in order
    async fn download(
        &self,
        path: &str,
        hash: &str,
        suffix: &str,
    ) -> Result<Self::Response, Self::Error> {
        let hosts = self.get_all_cdn_hosts();

        if hosts.is_empty() {
            return Err(FallbackError::invalid_host("No CDN hosts configured"));
        }

        let mut last_error: Option<<T as CdnClientTrait>::Error> = None;

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
        Err(last_error.unwrap_or_else(FallbackError::cdn_exhausted))
    }
}

/// Builder for configuring CDN client with fallback
// #[derive(Debug, Clone)]
pub struct CdnClientWithFallbackBuilder<T>
where
    // T: CdnClientBuilderTrait<Client = U, Error = <U as CdnClientTrait>::Error>,
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    base_client_builder: <T as CdnClientTrait>::Builder,
    primary_cdns: Vec<String>,
    use_default_backups: bool,
    custom_cdns: Vec<String>,
    // _phantom: PhantomData<U>,
}

impl<T> Clone for CdnClientWithFallbackBuilder<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    fn clone(&self) -> Self {
        Self {
            base_client_builder: self.base_client_builder.clone(),
            primary_cdns: self.primary_cdns.clone(),
            use_default_backups: self.use_default_backups.clone(),
            custom_cdns: self.custom_cdns.clone(),
        }
    }
}

impl<T> Debug for CdnClientWithFallbackBuilder<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdnClientWithFallbackBuilder")
            .field("base_client_builder", &self.base_client_builder)
            .field("primary_cdns", &self.primary_cdns)
            .field("use_default_backups", &self.use_default_backups)
            .field("custom_cdns", &self.custom_cdns)
            .finish()
    }
}

impl<T> CdnClientBuilderTrait for CdnClientWithFallbackBuilder<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
    // T: CdnClientBuilderTrait<Client = U, Error = <U as CdnClientTrait>::Error>,
    // U: CdnClientTrait,
    // <U as CdnClientTrait>::Error: FallbackError + std::error::Error,
{
    type Client = CdnClientWithFallback<T>;
    type Error = <<T as CdnClientTrait>::Builder as CdnClientBuilderTrait>::Error;

    fn new() -> Self {
        Self {
            base_client_builder: <T as CdnClientTrait>::Builder::new(),
            primary_cdns: Vec::new(),
            use_default_backups: true,
            custom_cdns: Vec::new(),
            // _phantom: PhantomData,
        }
    }

    async fn build(self) -> Result<Self::Client, Self::Error> {
        let base_client: <<T as CdnClientTrait>::Builder as CdnClientBuilderTrait>::Client =
            match self.base_client_builder.build().await {
                Ok(o) => o,
                Err(e) => return Err(e),
            };
        let mut client = CdnClientWithFallback::with_client(base_client);

        client.set_use_default_backups(self.use_default_backups);
        client.set_primary_cdns(self.primary_cdns);
        client.set_custom_cdns(self.custom_cdns);

        Ok(client)
    }
}

impl<T> CdnClientWithFallbackBuilder<T>
where
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
    // T: CdnClientBuilderTrait<Client = U, Error = <U as CdnClientTrait>::Error>,
    // U: CdnClientTrait,
    // <U as CdnClientTrait>::Error: FallbackError,
    // <T as CdnClientBuilderTrait>::Client: CdnClientTrait,
    // <<T as CdnClientBuilderTrait>::Client as CdnClientTrait>::Error: FallbackError,
{
    /// Configure the base CDN client
    pub fn configure_base_client<F>(mut self, f: F) -> Self
    where
        F: FnOnce(T::Builder) -> T::Builder,
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
}

impl<T> Default for CdnClientWithFallbackBuilder<T>
where
    // T: CdnClientBuilderTrait<Client = U, Error = <U as CdnClientTrait>::Error>,
    // U: CdnClientTrait,
    // <U as CdnClientTrait>::Error: FallbackError,
    T: CdnClientTrait,
    <T as CdnClientTrait>::Error: FallbackError,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]

    async fn test_default_backup_cdns() {
        let client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
        let hosts = client.get_all_cdn_hosts();

        // Should have the default backup CDNs
        assert_eq!(hosts.len(), 2);
        assert!(hosts.contains(&"cdn.arctium.tools".to_string()));
        assert!(hosts.contains(&"tact.mirror.reliquaryhq.com".to_string()));
    }

    #[tokio::test]
    async fn test_add_primary_cdn() {
        let client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
        client.add_primary_cdn("primary.example.com");

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 3);
        // Primary CDN should be first
        assert_eq!(hosts[0], "primary.example.com");
        // Backup CDNs should be at the end
        assert_eq!(hosts[1], "cdn.arctium.tools");
        assert_eq!(hosts[2], "tact.mirror.reliquaryhq.com");
    }

    #[tokio::test]

    async fn test_primary_cdns_before_backups() {
        let client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();

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

    #[tokio::test]
    async fn test_disable_default_backups() {
        let mut client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
        client.set_use_default_backups(false);
        client.add_primary_cdn("primary.example.com");

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0], "primary.example.com");
    }

    #[tokio::test]
    async fn test_builder_configuration() {
        let client = CdnClientWithFallbackBuilder::<CdnClient>::new()
            .add_primary_cdn("cdn1.example.com")
            .add_primary_cdn("cdn2.example.com")
            .use_default_backups(false)
            .configure_base_client(|builder| builder.max_retries(5).initial_backoff_ms(200))
            .build()
            .await
            .unwrap();

        let hosts = client.get_all_cdn_hosts();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0], "cdn1.example.com");
        assert_eq!(hosts[1], "cdn2.example.com");
    }

    #[tokio::test]
    async fn test_no_duplicate_hosts() {
        let client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
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
