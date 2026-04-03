//! wago.tools integration for World of Warcraft build information.
//!
//! The wago.tools API returns build history as a JSON map of `product -> [build, ...]`.
//! This provider fetches that data, caches it to disk as JSON with Unix timestamps
//! for TTL checking, and converts builds into the crate's [`BuildInfo`] format.

use crate::error::{ImportError, ImportResult};
use crate::providers::{CacheStats, DataSource, ImportProvider, ImportProviderInfo};
use crate::types::{BuildInfo, ProviderCapabilities};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// wago.tools API base URL.
const WAGO_API_BASE: &str = "https://wago.tools/api/builds";

/// Build record from the wago.tools API.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WagoBuild {
    product: String,
    version: String,
    created_at: String,
    build_config: String,
    product_config: Option<String>,
    cdn_config: String,
    is_bgdl: bool,
    #[serde(default)]
    seqn: Option<u32>,
}

/// Disk cache entry: builds grouped by product with a Unix timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    builds: HashMap<String, Vec<WagoBuild>>,
    /// Unix timestamp seconds when this cache was written.
    timestamp: u64,
}

/// wago.tools provider for build information.
pub struct WagoProvider {
    info: ImportProviderInfo,
    client: Client,
    cache_dir: PathBuf,
    build_cache: HashMap<String, Vec<WagoBuild>>,
    /// Unix timestamp seconds of the last cache refresh.
    last_refresh: Option<u64>,
}

impl WagoProvider {
    /// Create a new wago.tools provider.
    ///
    /// `cache_dir` is the directory where build data is cached on disk.
    pub fn new(cache_dir: PathBuf) -> ImportResult<Self> {
        let info = ImportProviderInfo {
            source: DataSource::Wago,
            name: "wago.tools".to_string(),
            version: "1.0.0".to_string(),
            description: "wago.tools community build archive".to_string(),
            endpoint: Some(WAGO_API_BASE.to_string()),
            capabilities: ProviderCapabilities {
                builds: true,
                file_mappings: false,
                real_time: false,
                requires_auth: false,
            },
            rate_limit: Some(60),
            cache_ttl: Duration::from_secs(3600),
        };

        crate::ensure_crypto_provider();
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(format!("cascette-import/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ImportError::Network)?;

        Ok(Self {
            info,
            client,
            cache_dir,
            build_cache: HashMap::new(),
            last_refresh: None,
        })
    }

    /// Fetch all builds from the wago.tools API.
    async fn fetch_builds(&self) -> ImportResult<Vec<WagoBuild>> {
        let response = self
            .client
            .get(WAGO_API_BASE)
            .send()
            .await
            .map_err(ImportError::Network)?;

        if !response.status().is_success() {
            return Err(ImportError::HttpStatus {
                provider: "wago.tools".to_string(),
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let text = response.text().await.map_err(|e| ImportError::Provider {
            provider: "wago.tools".to_string(),
            message: format!("failed to read response text: {e}"),
        })?;

        // The API returns `{ "product_code": [build, ...], ... }`.
        let products_map: HashMap<String, Vec<WagoBuild>> =
            serde_json::from_str(&text).map_err(|e| ImportError::Provider {
                provider: "wago.tools".to_string(),
                message: format!("failed to parse JSON: {e}"),
            })?;

        Ok(products_map.into_values().flatten().collect())
    }

    /// Convert a `WagoBuild` to the crate-level [`BuildInfo`].
    fn convert_build(wago_build: &WagoBuild) -> BuildInfo {
        let mut metadata = HashMap::new();
        metadata.insert("build_config".to_string(), wago_build.build_config.clone());
        metadata.insert("cdn_config".to_string(), wago_build.cdn_config.clone());
        if let Some(ref pc) = wago_build.product_config {
            metadata.insert("product_config".to_string(), pc.clone());
        }
        metadata.insert("is_bgdl".to_string(), wago_build.is_bgdl.to_string());
        if let Some(seqn) = wago_build.seqn {
            metadata.insert("seqn".to_string(), seqn.to_string());
        }

        // Build number is the last segment of the version string.
        let build_number = wago_build
            .version
            .split('.')
            .next_back()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        // Determine version type from the product code.
        let version_type =
            if wago_build.product.contains("_ptr") || wago_build.product.ends_with('t') {
                "ptr"
            } else if wago_build.product.contains("_beta") {
                "beta"
            } else {
                "live"
            }
            .to_string();

        BuildInfo {
            product: wago_build.product.to_lowercase(),
            version: wago_build.version.clone(),
            build: build_number,
            version_type,
            region: None,
            timestamp: None,
            created_at: Some(wago_build.created_at.clone()),
            metadata,
        }
    }

    /// Load builds from the disk cache if it exists and is within the TTL.
    async fn load_from_cache(&mut self) -> ImportResult<()> {
        let cache_file = self.cache_dir.join("builds.json");
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
            self.build_cache = entry.builds;
            self.last_refresh = Some(entry.timestamp);
        }

        Ok(())
    }

    /// Save current build cache to disk.
    async fn save_to_cache(&self) -> ImportResult<()> {
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(ImportError::Io)?;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            builds: self.build_cache.clone(),
            timestamp: now,
        };

        let json = serde_json::to_string_pretty(&entry).map_err(ImportError::Json)?;
        tokio::fs::write(self.cache_dir.join("builds.json"), json)
            .await
            .map_err(ImportError::Io)?;

        Ok(())
    }

    /// Fetch all builds from the API and populate the cache.
    async fn preload_builds_cache(&mut self) -> ImportResult<()> {
        let all_builds = self.fetch_builds().await?;

        let mut product_cache: HashMap<String, Vec<WagoBuild>> = HashMap::new();
        for build in all_builds {
            product_cache
                .entry(build.product.to_lowercase())
                .or_default()
                .push(build);
        }

        self.build_cache = product_cache;
        self.last_refresh = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        Ok(())
    }

    /// Check if the cache is stale or empty.
    fn cache_needs_refresh(&self) -> bool {
        if self.build_cache.is_empty() {
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
}

#[async_trait]
impl ImportProvider for WagoProvider {
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
            && self.preload_builds_cache().await.is_ok()
        {
            let _ = self.save_to_cache().await;
        }

        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.client
            .get(WAGO_API_BASE)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn get_builds(&self, product: &str) -> ImportResult<Vec<BuildInfo>> {
        // Empty product key means "return all products".
        if product.is_empty() {
            if !self.build_cache.is_empty() {
                return Ok(self
                    .build_cache
                    .values()
                    .flatten()
                    .map(Self::convert_build)
                    .collect());
            }

            let all_builds = self.fetch_builds().await?;
            return Ok(all_builds.iter().map(Self::convert_build).collect());
        }

        let product_key = product.to_lowercase();

        if let Some(cached_builds) = self.build_cache.get(&product_key) {
            return Ok(cached_builds.iter().map(Self::convert_build).collect());
        }

        // Cache miss: fetch all builds, return the requested product.
        let all_builds = self.fetch_builds().await?;
        let mut product_cache: HashMap<String, Vec<WagoBuild>> = HashMap::new();
        for build in all_builds {
            product_cache
                .entry(build.product.to_lowercase())
                .or_default()
                .push(build);
        }

        let product_builds = product_cache.get(&product_key).cloned().unwrap_or_default();
        Ok(product_builds.iter().map(Self::convert_build).collect())
    }

    async fn refresh_cache(&mut self) -> ImportResult<()> {
        self.preload_builds_cache().await?;
        self.save_to_cache().await
    }

    async fn cache_stats(&self) -> ImportResult<CacheStats> {
        let total_builds: usize = self.build_cache.values().map(Vec::len).sum();
        Ok(CacheStats {
            entries: total_builds,
            hits: 0,
            misses: 0,
            last_refresh: self.last_refresh,
            size_bytes: total_builds * std::mem::size_of::<WagoBuild>(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_build() {
        let wago = WagoBuild {
            product: "wow".to_string(),
            version: "11.2.5.62785".to_string(),
            created_at: "2025-01-15 12:00:00".to_string(),
            build_config: "abc123".to_string(),
            product_config: Some("def456".to_string()),
            cdn_config: "ghi789".to_string(),
            is_bgdl: false,
            seqn: Some(42),
        };

        let info = WagoProvider::convert_build(&wago);
        assert_eq!(info.product, "wow");
        assert_eq!(info.build, 62785);
        assert_eq!(info.version_type, "live");
        assert_eq!(info.created_at.as_deref(), Some("2025-01-15 12:00:00"));
        assert_eq!(info.metadata.get("seqn").unwrap(), "42");
    }

    #[test]
    fn test_convert_build_ptr() {
        let wago = WagoBuild {
            product: "wowt".to_string(),
            version: "11.2.5.62785".to_string(),
            created_at: String::new(),
            build_config: String::new(),
            product_config: None,
            cdn_config: String::new(),
            is_bgdl: false,
            seqn: None,
        };

        let info = WagoProvider::convert_build(&wago);
        assert_eq!(info.version_type, "ptr");
    }

    #[test]
    fn test_provider_creation() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let provider = WagoProvider::new(dir.path().to_path_buf());
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.info().name, "wago.tools");
        assert!(provider.info().capabilities.builds);
        assert!(!provider.info().capabilities.file_mappings);
    }
}
