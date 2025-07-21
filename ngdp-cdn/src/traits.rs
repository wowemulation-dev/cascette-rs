use std::{error::Error, fmt::Debug, ops::RangeInclusive};

/// CDN client which takes a hostname parameter
pub trait CdnClientTrait: Sized {
    /// Response type
    type Response;

    /// Error type
    type Error: Error;

    /// Client builder type
    type Builder: CdnClientBuilderTrait<Client = Self>;

    /// Create a new CDN client instance with default options.
    async fn new() -> Result<Self, Self::Error>;

    /// Create a builder for this CDN client.
    fn builder() -> Self::Builder {
        Self::Builder::new()
    }

    /// Download content from CDN by hash
    async fn download(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        suffix: &str,
    ) -> Result<Self::Response, Self::Error>;

    /// Download content from CDN by hash, with a HTTP `Range` header.
    async fn download_range(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        cache_hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Self::Response, Self::Error>;

    /// Download BuildConfig from CDN
    ///
    /// BuildConfig files are stored at `{path}/config/{hash}`
    async fn download_build_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash, "").await
    }

    /// Download CDNConfig from CDN
    ///
    /// CDNConfig files are stored at `{path}/config/{hash}`
    async fn download_cdn_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash, "").await
    }

    /// Download ProductConfig from CDN
    ///
    /// ProductConfig files are stored at `{config_path}/{hash}`
    /// Note: This uses the config_path from CDN response, not the regular path
    async fn download_product_config(
        &self,
        cdn_host: &str,
        config_path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        self.download(cdn_host, config_path, hash, "").await
    }

    /// Download KeyRing from CDN
    ///
    /// KeyRing files are stored at `{path}/config/{hash}`
    async fn download_key_ring(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash, "").await
    }

    /// Download data file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}`
    async fn download_data(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(cdn_host, &data_path, hash, "").await
    }

    /// Download data index file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}.index`
    async fn download_data_index(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(cdn_host, &data_path, hash, ".index").await
    }

    /// Download partial range of a data file from the CDN.
    ///
    /// Data files are stored at `{path}/data/{hash}`
    async fn download_data_range(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        cache_hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Self::Response, Self::Error> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download_range(cdn_host, &data_path, hash, cache_hash, range)
            .await
    }

    /// Download patch file from CDN
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    async fn download_patch(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(cdn_host, &patch_path, hash, "").await
    }

    /// Download partial range of a patch file from the CDN.
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    async fn download_patch_range(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        cache_hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Self::Response, Self::Error> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download_range(cdn_host, &patch_path, hash, cache_hash, range)
            .await
    }
}

/// Trait for a [`CdnClientTrait`]'s builder
pub trait CdnClientBuilderTrait: Debug + Clone {
    /// The type of the CDN client generated by this builder.
    type Client;

    /// The type of errors returned by this builder.
    type Error: Error;

    /// Create a new builder with default values.
    fn new() -> Self;

    /// Build the CDN client.
    async fn build(self) -> Result<Self::Client, Self::Error>;
}

/// Trait for a fallback CDN client.
/// 
/// This rotates between multiple CDNs.
pub trait FallbackCdnClientTrait: Sized {
    /// Response type
    type Response;

    /// Error type
    type Error: FallbackError + Error;

    /// Client builder type
    type Builder: CdnClientBuilderTrait<Client = Self>;

    /// Create a new fallback CDN client instance with default options.
    async fn new() -> Result<Self, Self::Error>;

    /// Create a builder instance for this CDN client.
    fn builder() -> Self::Builder {
        Self::Builder::new()
    }

    async fn download(
        &self,
        path: &str,
        hash: &str,
        suffix: &str,
    ) -> Result<Self::Response, Self::Error>;

    /// Download BuildConfig from CDN
    async fn download_build_config(
        &self,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download CDNConfig from CDN
    async fn download_cdn_config(
        &self,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download ProductConfig from CDN
    async fn download_product_config(
        &self,
        config_path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        self.download(config_path, hash, "").await
    }

    /// Download KeyRing from CDN
    async fn download_key_ring(
        &self,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download data file from CDN
    async fn download_data(&self, path: &str, hash: &str) -> Result<Self::Response, Self::Error> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, "").await
    }

    /// Download data index file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}.index`
    async fn download_data_index(
        &self,
        path: &str,
        hash: &str,
    ) -> Result<Self::Response, Self::Error> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, ".index").await
    }

    /// Download patch file from CDN
    async fn download_patch(&self, path: &str, hash: &str) -> Result<Self::Response, Self::Error> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(&patch_path, hash, "").await
    }
}

/// Specialised error types for the fallback CDN client.
pub trait FallbackError {
    fn invalid_host(host: impl Into<String>) -> Self;
    fn cdn_exhausted() -> Self;
    fn invalid_response(reason: impl Into<String>) -> Self;
}
