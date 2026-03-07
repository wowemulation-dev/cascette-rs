//! BlizzTrack integration for Blizzard TACT product build information.
//!
//! The BlizzTrack API (`https://blizztrack.com/api/`) wraps all responses in a
//! `{"success": bool, "result": ...}` envelope and returns per-region version
//! entries. Each region entry is expanded into a separate [`BuildInfo`].
//!
//! This provider supports a fixed set of TACT product codes:
//! `agent`, `bna`, `wow`, `wow_classic`, `wow_classic_era`,
//! `wow_classic_titan`, `wow_anniversary`.

use crate::error::{ImportError, ImportResult};
use crate::providers::{CacheStats, DataSource, ImportProvider, ImportProviderInfo};
use crate::types::{BuildInfo, ProviderCapabilities};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// BlizzTrack API base URL.
const BLIZZTRACK_API_BASE: &str = "https://blizztrack.com/api";

/// TACT product codes supported by this provider.
const SUPPORTED_PRODUCTS: &[&str] = &[
    "agent",
    "bna",
    "wow",
    "wow_classic",
    "wow_classic_era",
    "wow_classic_titan",
    "wow_anniversary",
];

/// Generic BlizzTrack API response envelope.
#[derive(Debug, Deserialize)]
struct BlizzTrackResponse<T> {
    #[allow(dead_code)]
    success: bool,
    result: T,
}

/// Version manifest returned by `/api/manifest/{tact}/versions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionManifest {
    seqn: u32,
    tact: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    created_at: String,
    data: Vec<RegionEntry>,
}

/// Per-region entry within a version manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegionEntry {
    region: String,
    #[allow(dead_code)]
    name: String,
    version_name: String,
    build_id: u32,
    build_config: String,
    cdn_config: String,
    product_config: Option<String>,
}

/// Flat build record stored in the disk cache.
///
/// Combines fields from both [`VersionManifest`] and [`RegionEntry`] so the
/// cache is self-contained without nested structures.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlizzTrackBuild {
    product: String,
    region: String,
    version: String,
    build_id: u32,
    seqn: u32,
    build_config: String,
    cdn_config: String,
    product_config: Option<String>,
}

/// Disk cache entry keyed by product code with a Unix timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    builds: HashMap<String, Vec<BlizzTrackBuild>>,
    /// Unix timestamp seconds when this cache was written.
    timestamp: u64,
}

/// BlizzTrack provider for TACT build information.
pub struct BlizzTrackProvider {
    info: ImportProviderInfo,
    client: Client,
    cache_dir: PathBuf,
    build_cache: HashMap<String, Vec<BlizzTrackBuild>>,
    /// Unix timestamp seconds of the last cache refresh.
    last_refresh: Option<u64>,
}

impl BlizzTrackProvider {
    /// Create a new BlizzTrack provider.
    ///
    /// `cache_dir` is the directory where build data is cached on disk.
    pub fn new(cache_dir: PathBuf) -> ImportResult<Self> {
        let info = ImportProviderInfo {
            source: DataSource::BlizzTrack,
            name: "BlizzTrack".to_string(),
            version: "1.0.0".to_string(),
            description: "BlizzTrack community TACT product archive".to_string(),
            endpoint: Some(format!("{BLIZZTRACK_API_BASE}/manifest")),
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

    /// Fetch versions for a single TACT product code.
    async fn fetch_product_versions(&self, product: &str) -> ImportResult<Vec<BlizzTrackBuild>> {
        let url = format!("{BLIZZTRACK_API_BASE}/manifest/{product}/versions");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(ImportError::Network)?;

        if !response.status().is_success() {
            return Err(ImportError::HttpStatus {
                provider: "BlizzTrack".to_string(),
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let text = response.text().await.map_err(|e| ImportError::Provider {
            provider: "BlizzTrack".to_string(),
            message: format!("failed to read response text: {e}"),
        })?;

        let envelope: BlizzTrackResponse<VersionManifest> =
            serde_json::from_str(&text).map_err(|e| ImportError::Provider {
                provider: "BlizzTrack".to_string(),
                message: format!("failed to parse JSON for product '{product}': {e}"),
            })?;

        let manifest = envelope.result;
        let seqn = manifest.seqn;
        let tact = manifest.tact.clone();

        Ok(manifest
            .data
            .into_iter()
            .map(|entry| BlizzTrackBuild {
                product: tact.to_lowercase(),
                region: entry.region,
                version: entry.version_name,
                build_id: entry.build_id,
                seqn,
                build_config: entry.build_config,
                cdn_config: entry.cdn_config,
                product_config: entry.product_config,
            })
            .collect())
    }

    /// Fetch versions for all supported product codes.
    async fn fetch_all_products(&self) -> ImportResult<Vec<BlizzTrackBuild>> {
        let mut all_builds = Vec::new();

        for product in SUPPORTED_PRODUCTS {
            match self.fetch_product_versions(product).await {
                Ok(builds) => all_builds.extend(builds),
                // Skip unavailable products rather than aborting the whole fetch.
                Err(ImportError::HttpStatus { status: 404, .. }) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(all_builds)
    }

    /// Convert a [`BlizzTrackBuild`] to the crate-level [`BuildInfo`].
    fn convert_build(bt: &BlizzTrackBuild) -> BuildInfo {
        let mut metadata = HashMap::new();
        metadata.insert("build_config".to_string(), bt.build_config.clone());
        metadata.insert("cdn_config".to_string(), bt.cdn_config.clone());
        if let Some(ref pc) = bt.product_config {
            metadata.insert("product_config".to_string(), pc.clone());
        }
        metadata.insert("seqn".to_string(), bt.seqn.to_string());

        // Build number is the last segment of the version string.
        let build_number = bt
            .version
            .split('.')
            .next_back()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(bt.build_id);

        // Determine version type from the product code suffix.
        let version_type = if bt.product.contains("_ptr") || bt.product.ends_with('t') {
            "ptr"
        } else if bt.product.contains("_beta") {
            "beta"
        } else {
            "live"
        }
        .to_string();

        BuildInfo {
            product: bt.product.to_lowercase(),
            version: bt.version.clone(),
            build: build_number,
            version_type,
            region: Some(bt.region.clone()),
            timestamp: None,
            created_at: None,
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

    /// Fetch all products from the API and populate the build cache.
    async fn preload_builds_cache(&mut self) -> ImportResult<()> {
        let all_builds = self.fetch_all_products().await?;

        let mut product_cache: HashMap<String, Vec<BlizzTrackBuild>> = HashMap::new();
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
impl ImportProvider for BlizzTrackProvider {
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
        let url = format!("{BLIZZTRACK_API_BASE}/manifest/agent/versions");
        self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn get_builds(&self, product: &str) -> ImportResult<Vec<BuildInfo>> {
        // Empty product key means "return all supported products".
        if product.is_empty() {
            if !self.build_cache.is_empty() {
                return Ok(self
                    .build_cache
                    .values()
                    .flatten()
                    .map(Self::convert_build)
                    .collect());
            }

            let all_builds = self.fetch_all_products().await?;
            return Ok(all_builds.iter().map(Self::convert_build).collect());
        }

        let product_key = product.to_lowercase();

        if !SUPPORTED_PRODUCTS.contains(&product_key.as_str()) {
            return Err(ImportError::Provider {
                provider: "BlizzTrack".to_string(),
                message: format!(
                    "unsupported product '{product_key}'; supported: {}",
                    SUPPORTED_PRODUCTS.join(", ")
                ),
            });
        }

        if let Some(cached_builds) = self.build_cache.get(&product_key) {
            return Ok(cached_builds.iter().map(Self::convert_build).collect());
        }

        // Cache miss: fetch the specific product directly.
        let builds = self.fetch_product_versions(&product_key).await?;
        Ok(builds.iter().map(Self::convert_build).collect())
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
            size_bytes: total_builds * std::mem::size_of::<BlizzTrackBuild>(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_build(product: &str) -> BlizzTrackBuild {
        BlizzTrackBuild {
            product: product.to_string(),
            region: "us".to_string(),
            version: "11.2.5.62785".to_string(),
            build_id: 62785,
            seqn: 100,
            build_config: "abc123".to_string(),
            cdn_config: "def456".to_string(),
            product_config: Some("ghi789".to_string()),
        }
    }

    #[test]
    fn test_convert_build() {
        let bt = make_build("wow");
        let info = BlizzTrackProvider::convert_build(&bt);
        assert_eq!(info.product, "wow");
        assert_eq!(info.build, 62785);
        assert_eq!(info.version_type, "live");
        assert_eq!(info.region.as_deref(), Some("us"));
        assert_eq!(info.metadata.get("seqn").unwrap(), "100");
        assert_eq!(info.metadata.get("build_config").unwrap(), "abc123");
    }

    #[test]
    fn test_convert_build_ptr() {
        let bt = make_build("wowt");
        let info = BlizzTrackProvider::convert_build(&bt);
        assert_eq!(info.version_type, "ptr");
    }

    #[test]
    fn test_convert_build_beta() {
        let bt = make_build("wow_beta");
        let info = BlizzTrackProvider::convert_build(&bt);
        assert_eq!(info.version_type, "beta");
    }

    #[test]
    fn test_provider_creation() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let provider = BlizzTrackProvider::new(dir.path().to_path_buf());
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.info().name, "BlizzTrack");
        assert!(provider.info().capabilities.builds);
        assert!(!provider.info().capabilities.file_mappings);
    }

    #[test]
    fn test_data_source_blizztrack() {
        assert_eq!(DataSource::BlizzTrack.id(), "blizztrack");
        assert_eq!(DataSource::BlizzTrack.display_name(), "BlizzTrack");
    }

    #[tokio::test]
    async fn test_get_builds_unsupported_product() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let provider = BlizzTrackProvider::new(dir.path().to_path_buf()).expect("provider");
        let result = provider.get_builds("diablo4").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unsupported product"), "got: {err}");
    }
}
