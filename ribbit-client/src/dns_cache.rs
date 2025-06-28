//! DNS caching for Ribbit client connections

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default DNS cache TTL in seconds
const DEFAULT_DNS_TTL_SECS: u64 = 300; // 5 minutes

/// A cached DNS entry
#[derive(Debug, Clone)]
struct DnsEntry {
    /// Resolved IP addresses
    addresses: Vec<IpAddr>,
    /// When this entry expires
    expires_at: Instant,
}

/// DNS cache for resolving hostnames to IP addresses
#[derive(Debug, Clone)]
pub struct DnsCache {
    /// Cache storage
    cache: Arc<RwLock<HashMap<String, DnsEntry>>>,
    /// TTL for cache entries
    ttl: Duration,
}

impl DnsCache {
    /// Create a new DNS cache with default TTL
    #[must_use]
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(DEFAULT_DNS_TTL_SECS))
    }

    /// Create a new DNS cache with specified TTL
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Resolve a hostname, using cache if available
    /// Resolve hostname and port to socket addresses with caching
    ///
    /// # Errors
    /// Returns an error if DNS resolution fails
    pub async fn resolve(&self, hostname: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(hostname) {
                if entry.expires_at > Instant::now() {
                    // Cache hit - return cached addresses
                    let socket_addrs: Vec<SocketAddr> = entry
                        .addresses
                        .iter()
                        .map(|&ip| SocketAddr::new(ip, port))
                        .collect();
                    return Ok(socket_addrs);
                }
            }
        }

        // Cache miss or expired - resolve and update cache
        self.resolve_and_cache(hostname, port).await
    }

    /// Resolve hostname and update cache
    async fn resolve_and_cache(
        &self,
        hostname: &str,
        port: u16,
    ) -> std::io::Result<Vec<SocketAddr>> {
        // Perform DNS resolution synchronously in a blocking task
        let hostname_string = hostname.to_string();
        let addrs = tokio::task::spawn_blocking(move || {
            format!("{hostname_string}:{port}").to_socket_addrs()
        })
        .await
        .map_err(std::io::Error::other)??;

        let socket_addrs: Vec<SocketAddr> = addrs.collect();

        if !socket_addrs.is_empty() {
            // Extract IP addresses and cache them
            let ip_addrs: Vec<IpAddr> = socket_addrs.iter().map(std::net::SocketAddr::ip).collect();

            let entry = DnsEntry {
                addresses: ip_addrs.clone(),
                expires_at: Instant::now() + self.ttl,
            };

            let mut cache = self.cache.write().await;
            cache.insert(hostname.to_string(), entry);
        }

        Ok(socket_addrs)
    }

    /// Clear the DNS cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Remove expired entries from the cache
    pub async fn remove_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// Get the number of cached entries
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dns_cache_creation() {
        let cache = DnsCache::new();
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_dns_cache_with_ttl() {
        let cache = DnsCache::with_ttl(Duration::from_secs(60));
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_dns_resolution() {
        let cache = DnsCache::new();

        // Resolve localhost - should always work
        let addrs = cache.resolve("localhost", 80).await.unwrap();
        assert!(!addrs.is_empty());

        // Cache should now contain the entry
        assert_eq!(cache.size().await, 1);

        // Second resolution should use cache
        let addrs2 = cache.resolve("localhost", 80).await.unwrap();
        assert_eq!(addrs, addrs2);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = DnsCache::with_ttl(Duration::from_millis(10));

        // Resolve and cache
        let _addrs = cache.resolve("localhost", 80).await.unwrap();
        assert_eq!(cache.size().await, 1);

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Remove expired entries
        cache.remove_expired().await;
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let cache = DnsCache::new();

        // Add some entries
        let _addrs1 = cache.resolve("localhost", 80).await.unwrap();
        let _addrs2 = cache.resolve("127.0.0.1", 80).await.unwrap();
        assert!(cache.size().await >= 1);

        // Clear cache
        cache.clear().await;
        assert_eq!(cache.size().await, 0);
    }
}
