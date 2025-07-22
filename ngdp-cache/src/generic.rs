//! Generic cache implementation for arbitrary data

use crate::{Cache, Result};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

/// Generic cache for storing arbitrary data
pub struct GenericCache {
    c: Cache,
}

impl GenericCache {
    /// Create a new generic cache in
    /// [the user's cache directory][crate::get_cache_dir].
    pub async fn new() -> Result<Self> {
        let c = Cache::with_subdirectory("generic").await?;
        Ok(Self { c })
    }

    /// Create a new generic cache with a custom subdirectory
    pub async fn with_subdirectory(subdir: impl AsRef<Path>) -> Result<Self> {
        let subdir = PathBuf::from("generic").join(subdir);
        let c = Cache::with_subdirectory(subdir).await?;
        Ok(Self { c })
    }

    /// Create a CDN cache with a custom base directory
    pub async fn with_base_dir(base_dir: impl AsRef<Path>) -> Result<Self> {
        let path = base_dir.as_ref().join("cdn");
        let c = Cache::with_base_dir(path).await?;
        Ok(Self { c })
    }
}

impl Deref for GenericCache {
    type Target = Cache;

    fn deref(&self) -> &Self::Target {
        &self.c
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn test_generic_cache_operations() {
        let cache = GenericCache::with_subdirectory("test").await.unwrap();

        // Test write and read
        let key = "test_key";
        let data = b"test data";

        cache.write_buffer("", key, &data[..]).await.unwrap();
        assert_eq!(
            data.len() as u64,
            cache.object_size("", key).await.unwrap().unwrap()
        );

        let mut read_file = cache.read_object("", key).await.unwrap().unwrap();
        let mut read_buf = Vec::with_capacity(data.len());
        read_file.read_to_end(&mut read_buf).await.unwrap();
        assert_eq!(read_buf, data);

        // Test delete
        assert!(cache.delete_object("", key,).await.unwrap());
        assert!(cache.object_size("", key).await.unwrap().is_none());
    }
}
