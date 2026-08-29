//! In-memory size estimate cache with async polling.
//!
//! Size estimation is transient (no SQLite persistence needed). The cache
//! stores per-UID results that background tasks populate after a POST
//! request starts the estimation.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Status of a size estimation request.
#[derive(Debug, Clone)]
pub enum SizeEstimateStatus {
    /// Estimation is in progress.
    Pending,
    /// Estimation completed with a byte count.
    Ready(u64),
    /// Estimation failed.
    Failed,
}

/// A cached size estimation entry.
#[derive(Debug, Clone)]
pub struct SizeEstimateEntry {
    /// Product unique identifier.
    pub uid: String,
    /// Current status of the estimation.
    pub status: SizeEstimateStatus,
}

/// In-memory cache for size estimation results.
#[derive(Debug, Clone)]
pub struct SizeEstimateCache {
    entries: Arc<RwLock<HashMap<String, SizeEstimateEntry>>>,
}

impl SizeEstimateCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert a pending entry for the given UID.
    ///
    /// Returns `false` if an entry already exists (no overwrite).
    pub async fn insert_pending(&self, uid: &str) -> bool {
        let mut entries = self.entries.write().await;
        if entries.contains_key(uid) {
            return false;
        }
        entries.insert(
            uid.to_string(),
            SizeEstimateEntry {
                uid: uid.to_string(),
                status: SizeEstimateStatus::Pending,
            },
        );
        true
    }

    /// Mark an entry as ready with the estimated byte count.
    pub async fn set_ready(&self, uid: &str, bytes: u64) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(uid) {
            entry.status = SizeEstimateStatus::Ready(bytes);
        }
    }

    /// Mark an entry as failed.
    pub async fn set_failed(&self, uid: &str) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(uid) {
            entry.status = SizeEstimateStatus::Failed;
        }
    }

    /// Look up a cached entry.
    pub async fn get(&self, uid: &str) -> Option<SizeEstimateEntry> {
        self.entries.read().await.get(uid).cloned()
    }
}

impl Default for SizeEstimateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_pending_returns_false_if_exists() {
        let cache = SizeEstimateCache::new();
        assert!(cache.insert_pending("wow").await);
        assert!(!cache.insert_pending("wow").await);
    }

    #[tokio::test]
    async fn test_lifecycle() {
        let cache = SizeEstimateCache::new();
        assert!(cache.get("wow").await.is_none());

        cache.insert_pending("wow").await;
        let entry = cache.get("wow").await.unwrap();
        assert!(matches!(entry.status, SizeEstimateStatus::Pending));

        cache.set_ready("wow", 42_000).await;
        let entry = cache.get("wow").await.unwrap();
        assert!(matches!(entry.status, SizeEstimateStatus::Ready(42_000)));
    }

    #[tokio::test]
    async fn test_set_failed() {
        let cache = SizeEstimateCache::new();
        cache.insert_pending("wow").await;
        cache.set_failed("wow").await;
        let entry = cache.get("wow").await.unwrap();
        assert!(matches!(entry.status, SizeEstimateStatus::Failed));
    }
}
