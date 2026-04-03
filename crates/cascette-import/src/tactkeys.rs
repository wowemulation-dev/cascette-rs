//! WoWDev TACT Keys repository integration for encryption keys.
//!
//! The TACT keys file at `github.com/wowdev/TACTKeys` uses a whitespace-delimited
//! format (`key_id key_hex`) that is parsed by [`TactKeyStore::load_from_txt`] in
//! the `cascette-crypto` crate. This provider wraps that functionality with disk
//! caching and TTL-based invalidation.
//!
//! The [`fetch_github_tactkeys`] function provides a standalone way to fetch keys
//! without the full provider machinery.

use crate::error::{ImportError, ImportResult};
use crate::providers::{CacheStats, DataSource, ImportProvider, ImportProviderInfo};
use crate::types::{BuildInfo, ProviderCapabilities};
use async_trait::async_trait;
use cascette_crypto::{TactKey, TactKeyStore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Raw TACT keys download URL.
const TACTKEYS_RAW_URL: &str = "https://raw.githubusercontent.com/wowdev/TACTKeys/master/WoW.txt";

/// Disk cache format: hex key pairs with a Unix timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    /// `(key_id_hex, key_hex)` pairs.
    keys: Vec<(String, String)>,
    /// Unix timestamp seconds.
    timestamp: u64,
}

/// WoWDev TACT Keys provider for encryption keys.
pub struct TactKeysProvider {
    info: ImportProviderInfo,
    client: Client,
    cache_dir: PathBuf,
    tact_keys: TactKeyStore,
    /// Unix timestamp seconds of the last cache refresh.
    last_refresh: Option<u64>,
}

impl TactKeysProvider {
    /// Create a new TACT keys provider.
    ///
    /// `cache_dir` is the directory where keys are cached on disk.
    pub fn new(cache_dir: PathBuf) -> ImportResult<Self> {
        crate::ensure_crypto_provider();
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(format!("cascette-import/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ImportError::Network)?;

        Ok(Self {
            info: ImportProviderInfo {
                source: DataSource::TactKeys,
                name: "WoWDev TACT Keys".to_string(),
                description: "Community-maintained TACT encryption keys".to_string(),
                version: "1.0.0".to_string(),
                endpoint: Some(TACTKEYS_RAW_URL.to_string()),
                capabilities: ProviderCapabilities {
                    builds: false,
                    file_mappings: false,
                    real_time: false,
                    requires_auth: false,
                },
                rate_limit: Some(10),
                cache_ttl: Duration::from_secs(86400),
            },
            client,
            cache_dir,
            tact_keys: TactKeyStore::empty(),
            last_refresh: None,
        })
    }

    /// Download and parse the TACT keys file from GitHub.
    async fn fetch_tact_keys(&self) -> ImportResult<TactKeyStore> {
        let response = self
            .client
            .get(TACTKEYS_RAW_URL)
            .send()
            .await
            .map_err(ImportError::Network)?;

        if !response.status().is_success() {
            return Err(ImportError::HttpStatus {
                provider: "tactkeys".to_string(),
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let content = response.text().await.map_err(ImportError::Network)?;

        let mut store = TactKeyStore::empty();
        store.load_from_txt(&content);

        Ok(store)
    }

    /// Load keys from disk cache if valid.
    async fn load_from_cache(&mut self) -> ImportResult<()> {
        let cache_file = self.cache_dir.join("keys.json");
        if !cache_file.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&cache_file)
            .await
            .map_err(ImportError::Io)?;

        let entry: CacheEntry = serde_json::from_str(&content).map_err(ImportError::Json)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now.saturating_sub(entry.timestamp) < self.info.cache_ttl.as_secs() {
            let mut store = TactKeyStore::empty();
            for (id_hex, key_hex) in &entry.keys {
                if let Ok(id) = u64::from_str_radix(id_hex, 16)
                    && let Ok(tact_key) = TactKey::from_hex(id, key_hex)
                {
                    store.add(tact_key);
                }
            }
            self.tact_keys = store;
            self.last_refresh = Some(entry.timestamp);
        }

        Ok(())
    }

    /// Save keys to disk cache.
    async fn save_to_cache(&self) -> ImportResult<()> {
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(ImportError::Io)?;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let keys_vec: Vec<(String, String)> = self
            .tact_keys
            .iter()
            .map(|key| (format!("{:016X}", key.id), hex::encode_upper(key.key)))
            .collect();

        let entry = CacheEntry {
            keys: keys_vec,
            timestamp: now,
        };

        let json = serde_json::to_string_pretty(&entry).map_err(ImportError::Json)?;
        tokio::fs::write(self.cache_dir.join("keys.json"), json)
            .await
            .map_err(ImportError::Io)?;

        Ok(())
    }

    /// Check if the cache is stale or empty.
    fn cache_needs_refresh(&self) -> bool {
        if self.tact_keys.is_empty() {
            return true;
        }
        match self.last_refresh {
            Some(last) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now.saturating_sub(last) > self.info.cache_ttl.as_secs()
            }
            None => true,
        }
    }

    /// Get a TACT key by hex lookup string.
    pub fn get_tact_key(&self, lookup: &str) -> Option<TactKey> {
        let key_id = u64::from_str_radix(lookup.trim(), 16).ok()?;
        self.tact_keys
            .get(key_id)
            .map(|key_bytes| TactKey::new(key_id, *key_bytes))
    }

    /// Get all TACT keys currently loaded.
    pub fn get_all_tact_keys(&self) -> Vec<TactKey> {
        self.tact_keys.iter().collect()
    }

    /// Get a reference to the underlying key store.
    pub fn key_store(&self) -> &TactKeyStore {
        &self.tact_keys
    }
}

#[async_trait]
impl ImportProvider for TactKeysProvider {
    fn info(&self) -> &ImportProviderInfo {
        &self.info
    }

    async fn initialize(&mut self) -> ImportResult<()> {
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(ImportError::Io)?;
        }

        let _ = self.load_from_cache().await;

        if self.cache_needs_refresh()
            && self.is_available().await
            && let Ok(store) = self.fetch_tact_keys().await
        {
            self.tact_keys = store;
            self.last_refresh = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            let _ = self.save_to_cache().await;
        }

        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.client
            .head(TACTKEYS_RAW_URL)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn get_builds(&self, _product: &str) -> ImportResult<Vec<BuildInfo>> {
        Ok(Vec::new())
    }

    async fn refresh_cache(&mut self) -> ImportResult<()> {
        let store = self.fetch_tact_keys().await?;
        self.tact_keys = store;
        self.last_refresh = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.save_to_cache().await
    }

    async fn cache_stats(&self) -> ImportResult<CacheStats> {
        Ok(CacheStats {
            entries: self.tact_keys.len(),
            hits: 0,
            misses: 0,
            last_refresh: self.last_refresh,
            size_bytes: self.tact_keys.len() * std::mem::size_of::<TactKey>(),
        })
    }
}

/// Fetch TACT keys from the WoWDev GitHub repository.
///
/// This is a standalone function that does not require a [`TactKeysProvider`].
/// It creates a temporary HTTP client, downloads the keys file, and parses it
/// into a [`TactKeyStore`].
pub async fn fetch_github_tactkeys() -> ImportResult<TactKeyStore> {
    crate::ensure_crypto_provider();
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("cascette-import/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ImportError::Network)?;

    let response = client
        .get(TACTKEYS_RAW_URL)
        .send()
        .await
        .map_err(ImportError::Network)?;

    if !response.status().is_success() {
        return Err(ImportError::HttpStatus {
            provider: "github-tactkeys".to_string(),
            status: response.status().as_u16(),
            message: response.text().await.unwrap_or_default(),
        });
    }

    let content = response.text().await.map_err(ImportError::Network)?;

    let mut store = TactKeyStore::empty();
    store.load_from_txt(&content);

    Ok(store)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_tact_keys_parsing_via_store() {
        let content = r"# TACT Keys for World of Warcraft
// Comment line
0123456789ABCDEF 0123456789ABCDEF0123456789ABCDEF
FEDCBA9876543210 FEDCBA9876543210FEDCBA9876543210 Additional info

// Invalid lines
INVALID_HEX ABCD1234567890ABCDEF1234567890AB
0123456789ABCDEF INVALID_KEY
";

        let mut store = TactKeyStore::empty();
        let count = store.load_from_txt(content);
        assert_eq!(count, 2);

        let first = store
            .get(0x0123_4567_89AB_CDEF)
            .expect("first key should exist");
        assert_eq!(hex::encode_upper(first), "0123456789ABCDEF0123456789ABCDEF");

        let second = store
            .get(0xFEDC_BA98_7654_3210)
            .expect("second key should exist");
        assert_eq!(
            hex::encode_upper(second),
            "FEDCBA9876543210FEDCBA9876543210"
        );
    }

    #[test]
    fn test_provider_creation() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let provider = TactKeysProvider::new(dir.path().to_path_buf()).expect("create provider");
        assert_eq!(provider.info().source, DataSource::TactKeys);
        assert!(!provider.info().capabilities.builds);
        assert!(!provider.info().capabilities.file_mappings);
    }

    #[test]
    fn test_key_lookup() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let mut provider =
            TactKeysProvider::new(dir.path().to_path_buf()).expect("create provider");

        let key_id = 0x0123_4567_89AB_CDEF_u64;
        let test_key =
            TactKey::from_hex(key_id, "0123456789ABCDEF0123456789ABCDEF").expect("valid test key");
        provider.tact_keys.add(test_key);

        let result = provider.get_tact_key("0123456789abcdef");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, key_id);
    }

    #[tokio::test]
    async fn test_cache_roundtrip() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let cache_dir = dir.path().to_path_buf();

        // Save some keys.
        {
            let mut provider = TactKeysProvider::new(cache_dir.clone()).expect("create provider");
            let key =
                TactKey::from_hex(0x1234, "AABBCCDDAABBCCDDAABBCCDDAABBCCDD").expect("valid key");
            provider.tact_keys.add(key);
            provider.last_refresh = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            provider.save_to_cache().await.expect("save should succeed");
        }

        // Load them back.
        {
            let mut provider = TactKeysProvider::new(cache_dir).expect("create provider");
            provider
                .load_from_cache()
                .await
                .expect("load should succeed");
            assert_eq!(provider.tact_keys.len(), 1);
            assert!(provider.tact_keys.get(0x1234).is_some());
        }
    }
}
