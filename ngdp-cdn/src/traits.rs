use bytes::Bytes;
use futures_util::{Sink, Stream};

/// Cache provider trait.
pub trait CacheProvider {
    /// Read an item from cache.
    ///
    /// Return `None` if the item is not in cache.
    ///
    /// This function assumes that all cache items are immutable.
    async fn read(&self, path: &str, hash: &str, suffix: &str)
    -> Option<impl Stream<Item = Bytes>>;

    /// Write an item to cache, by providing a [`Sink`][] where it can be
    /// written.
    ///
    /// Return `None` if the item should not be cached.
    async fn write(&self, path: &str, hash: &str, suffix: &str) -> Option<impl Sink<Bytes>>;
}
