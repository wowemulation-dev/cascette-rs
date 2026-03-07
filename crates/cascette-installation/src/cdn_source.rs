//! CDN source abstraction for testability.
//!
//! The `CdnSource` trait abstracts over `CdnClient` so that tests can inject
//! a mock with pre-recorded responses instead of hitting real CDN servers.

use async_trait::async_trait;

use cascette_protocol::{CdnClient, CdnEndpoint, ContentType};

use crate::error::{InstallationError, InstallationResult};

/// Abstraction over CDN download operations.
///
/// `CdnClient` implements this trait directly. Tests use `MockCdnSource`.
#[async_trait]
pub trait CdnSource: Send + Sync {
    /// Download a complete file from CDN.
    async fn download(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
    ) -> InstallationResult<Vec<u8>>;

    /// Download a byte range from CDN.
    async fn download_range(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
        offset: u64,
        length: u64,
    ) -> InstallationResult<Vec<u8>>;

    /// Download an archive `.index` file from CDN.
    async fn download_archive_index(
        &self,
        endpoint: &CdnEndpoint,
        archive_key: &str,
    ) -> InstallationResult<Vec<u8>>;
}

#[async_trait]
impl CdnSource for CdnClient {
    async fn download(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
    ) -> InstallationResult<Vec<u8>> {
        Self::download(self, endpoint, content_type, key)
            .await
            .map_err(InstallationError::from)
    }

    async fn download_range(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
        offset: u64,
        length: u64,
    ) -> InstallationResult<Vec<u8>> {
        Self::download_range(self, endpoint, content_type, key, offset, length)
            .await
            .map_err(InstallationError::from)
    }

    async fn download_archive_index(
        &self,
        endpoint: &CdnEndpoint,
        archive_key: &str,
    ) -> InstallationResult<Vec<u8>> {
        Self::download_archive_index(self, endpoint, archive_key)
            .await
            .map_err(InstallationError::from)
    }
}
