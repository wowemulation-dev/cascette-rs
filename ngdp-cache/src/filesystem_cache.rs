use crate::Result;
use ngdp_cdn::CacheProvider;
use std::path::{Path, PathBuf};
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufRead, AsyncSeek, AsyncWrite, BufReader},
};

/// A caching wrapper around CdnClient
pub struct FilesystemCache {
    /// Base cache directory
    cache_base_dir: PathBuf,
    /// Whether caching is enabled
    enabled: bool,
}

impl FilesystemCache {
    /// Create a new cached CDN client
    pub async fn new() -> Result<Self> {
        let cache_base_dir = crate::get_cache_dir()?.join("cdn-fs");
        crate::ensure_dir(&cache_base_dir).await?;

        Ok(Self {
            cache_base_dir,
            enabled: true,
        })
    }

    /// Create a new cached client with custom cache directory
    pub async fn with_cache_dir(cache_dir: impl AsRef<Path>) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        crate::ensure_dir(&cache_dir).await?;

        Ok(Self {
            cache_base_dir: cache_dir,
            enabled: true,
        })
    }

    /// Is caching currently enabled?
    pub fn caching_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable caching
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        self.cache_base_dir.as_path()
    }
}

impl CacheProvider for FilesystemCache {
    async fn read(&self, full_path: &str) -> Option<impl AsyncBufRead + AsyncSeek> {
        if !self.enabled {
            return None;
        }

        let cache_path = self.cache_base_dir.join(full_path);
        // Read errors => cache miss
        let f = OpenOptions::new().read(true).open(cache_path).await.ok()?;
        Some(BufReader::new(f))
    }

    async fn write(&self, full_path: &str) -> Option<impl AsyncWrite> {
        if !self.enabled {
            return None;
        }

        let cache_path = self.cache_base_dir.join(full_path);
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(cache_path)
            .await
            .ok()
    }
}
