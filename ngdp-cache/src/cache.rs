use crate::{Result, ensure_dir, get_cache_dir};
use futures::StreamExt as _;
use reqwest::Response;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt as _, AsyncWriteExt as _},
};
use tracing::*;

/// A simple disk cache
pub struct Cache {
    /// Base directory for the cache
    base_dir: PathBuf,
}

impl Cache {
    /// Create a new CDN cache in [the user's cache directory][get_cache_dir].
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?;
        ensure_dir(&base_dir).await?;

        debug!("Initialized cache at: {:?}", base_dir);
        Ok(Self { base_dir })
    }

    /// Create a new CDN cache in a `subdir` of
    /// [the user's cache directory][get_cache_dir].
    pub async fn with_subdirectory(subdir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = get_cache_dir()?.join(subdir);
        ensure_dir(&base_dir).await?;
        debug!("Initialized cache at: {:?}", base_dir);
        Ok(Self { base_dir })
    }

    /// Create a CDN cache with a custom base directory
    pub async fn with_base_dir(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        ensure_dir(base_dir).await?;
        debug!("Initialized cache at: {base_dir:?}");
        Ok(Self {
            base_dir: base_dir.to_path_buf(),
        })
    }

    /// Get the base directory of this cache.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Path where `{path}/{hash}` should be cached to.
    ///
    /// `hash` has no formatting restrictions.
    pub fn cache_path(&self, path: impl AsRef<Path>, hash: &str) -> PathBuf {
        self.cache_path_with_suffix(path, hash, "")
    }

    /// Path where `{path}/{hash}{suffix}` should be cached to.
    ///
    /// `hash` has no formatting restrictions.
    pub fn cache_path_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> PathBuf {
        let mut path = self.base_dir().join(path);

        if hash.len() >= 4 {
            // abcdef -> ab/cd/abcdef
            path.push(&hash[..2]);
            path.push(&hash[2..4]);
        }
        path.push(format!("{hash}{suffix}"));
        path
    }

    /// Open a cache file for reading.
    ///    
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    ///
    /// Returns `Ok(None)` if the file does not exist. All other errors are
    /// propegated normally.
    pub async fn read_object(&self, path: impl AsRef<Path>, hash: &str) -> Result<Option<File>> {
        self.read_object_with_suffix(path, hash, "").await
    }

    /// Open a cache file for reading.
    ///    
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    ///
    /// Returns `Ok(None)` if the file does not exist. All other errors are
    /// propegated normally.
    pub async fn read_object_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<Option<File>> {
        let path = path.as_ref();
        debug!("Cache for {path:?} {hash:?} {suffix:?}");
        let path = self.cache_path_with_suffix(path, hash, suffix);

        match OpenOptions::new().read(true).open(&path).await {
            Ok(f) => Ok(Some(f)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => {
                error!("Read cache error for {path:?}: {e:?}");
                Err(e.into())
            }
        }
    }

    /// Writes a [`Response`] to a file, and then return a handle to that file,
    /// seeked to the start.
    ///
    /// The file will be open in read-write mode, but trait bounds will
    /// attempt to prevent write operations.
    ///
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    pub async fn write_response(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        response: Response,
    ) -> Result<File> {
        self.write_response_with_suffix(path, hash, "", response)
            .await
    }

    /// Writes a [`Response`] to a file, and then return a handle to that file,
    /// seeked to the start.
    ///
    /// The file will be open in read-write mode, but trait bounds will
    /// attempt to prevent write operations.
    ///
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    pub async fn write_response_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
        response: Response,
    ) -> Result<File> {
        let mut output = self.write_object_with_suffix(path, hash, suffix).await?;
        let len = response.content_length().unwrap_or(0);
        let mut stream = response.bytes_stream();

        let mut first = true;
        while let Some(buf) = stream.next().await {
            if first {
                first = false;
                // Only resize the file once the first chunk arrives.
                output.set_len(len).await?;
            }
            let buf = buf?;
            output.write_all(&buf).await?;
        }

        output.flush().await?;
        output.rewind().await?;
        Ok(output)
    }

    /// Write a buffer to the cache
    pub async fn write_buffer(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        buffer: impl AsyncBufReadExt + Unpin,
    ) -> Result<File> {
        self.write_buffer_with_suffix(path, hash, "", buffer).await
    }

    /// Write a buffer to the cache with a filename suffix
    pub async fn write_buffer_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
        mut buffer: impl AsyncBufReadExt + Unpin,
    ) -> Result<File> {
        let mut output = self.write_object_with_suffix(path, hash, suffix).await?;

        let mut b = [0; 8 << 10];
        while let Ok(len) = buffer.read(&mut b).await {
            if len == 0 {
                break;
            }
            output.write_all(&b[..len]).await?;
        }

        output.flush().await?;
        output.rewind().await?;
        Ok(output)
    }

    /// Open a cache file for writing.
    pub async fn write_object(&self, path: impl AsRef<Path>, hash: &str) -> Result<File> {
        self.write_object_with_suffix(path, hash, "").await
    }

    /// Open a cache file for writing.
    pub async fn write_object_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<File> {
        let path = self.cache_path_with_suffix(path, hash, suffix);
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        Ok(OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .await?)
    }

    /// Delete an item from the cache.
    ///
    /// Returns:
    ///
    /// * `Ok(true)` if a cache file existed and was deleted
    /// * `Ok(false)` if a cache file did not exist
    /// * `Err` on other errors
    pub async fn delete_object(&self, path: impl AsRef<Path>, hash: &str) -> Result<bool> {
        self.delete_object_with_suffix(path, hash, "").await
    }

    /// Delete an item from the cache.
    ///
    /// Returns:
    ///
    /// * `Ok(true)` if a cache file existed and was deleted
    /// * `Ok(false)` if a cache file did not exist
    /// * `Err` on other errors
    pub async fn delete_object_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<bool> {
        let path = self.cache_path_with_suffix(path, hash, suffix);

        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Get cached object size without reading it
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn object_size(&self, path: impl AsRef<Path>, hash: &str) -> Result<Option<u64>> {
        self.object_size_with_suffix(path, hash, "").await
    }

    /// Get cached object size without reading it
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn object_size_with_suffix(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<Option<u64>> {
        let path = self.cache_path_with_suffix(path, hash, suffix);
        match tokio::fs::metadata(&path).await {
            Ok(m) => Ok(Some(m.len())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
