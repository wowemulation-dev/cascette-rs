use std::future::Future;
use tokio::io::{AsyncBufRead, AsyncSeek, AsyncWrite};

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
