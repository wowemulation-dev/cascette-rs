use std::future::Future;
use tokio::io::{AsyncBufRead, AsyncSeek, AsyncWrite, Empty};

/// Cache provider trait.
pub trait CacheProvider {
    /// Read an item from cache.
    ///
    /// Return `None` if the item is not in cache.
    ///
    /// This function assumes that all cache items are immutable.
    fn read(&self, full_path: &str) -> impl Future<Output = Option<impl AsyncBufRead + AsyncSeek>>;

    /// Write an item to cache, by providing a [`Sink`][] where it can be
    /// written.
    ///
    /// Return `None` if the item should not be cached.
    fn write(&self, full_path: &str) -> impl Future<Output = Option<impl AsyncWrite>>;
}

/// No-op cache provider.
pub struct DummyCacheProvider;

impl CacheProvider for DummyCacheProvider {
    async fn read(&self, full_path: &str) -> Option<impl AsyncBufRead + AsyncSeek> {
        let _ = full_path;
        None::<Empty>
    }

    async fn write(&self, full_path: &str) -> Option<impl AsyncWrite> {
        let _ = full_path;
        None::<Empty>
    }
}
