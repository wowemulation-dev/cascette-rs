//! WoWDev community listfile integration for file ID to path mappings.
//!
//! The community listfile is a CSV file (`FileDataID;Path`) maintained at
//! `github.com/wowdev/wow-listfile`. This provider downloads and parses it,
//! caching the result to disk with TTL-based invalidation.

use crate::error::{ImportError, ImportResult};
use crate::providers::{CacheStats, DataSource, ImportProvider, ImportProviderInfo};
use crate::types::{BuildInfo, FileMapping, ProviderCapabilities};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Raw listfile download URL (GitHub releases).
const LISTFILE_RAW_URL: &str =
    "https://github.com/wowdev/wow-listfile/releases/latest/download/community-listfile.csv";

/// WoWDev listfile provider for file ID mappings.
pub struct ListfileProvider {
    info: ImportProviderInfo,
    client: Client,
    cache_dir: PathBuf,
    file_mappings: HashMap<u32, String>,
    /// Unix timestamp seconds of the last cache refresh.
    last_refresh: Option<u64>,
}

/// Disk cache format.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    mappings: HashMap<u32, String>,
    /// Unix timestamp seconds.
    timestamp: u64,
}

impl ListfileProvider {
    /// Create a new listfile provider.
    ///
    /// `cache_dir` is the directory where the parsed listfile is cached.
    pub fn new(cache_dir: PathBuf) -> ImportResult<Self> {
        crate::ensure_crypto_provider();
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(format!("cascette-import/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ImportError::Network)?;

        Ok(Self {
            info: ImportProviderInfo {
                source: DataSource::Listfile,
                name: "WoWDev Listfile".to_string(),
                description: "Community-maintained file ID to path mappings".to_string(),
                version: "1.0.0".to_string(),
                endpoint: Some(LISTFILE_RAW_URL.to_string()),
                capabilities: ProviderCapabilities {
                    builds: false,
                    file_mappings: true,
                    real_time: false,
                    requires_auth: false,
                },
                rate_limit: Some(10),
                cache_ttl: Duration::from_secs(86400),
            },
            client,
            cache_dir,
            file_mappings: HashMap::new(),
            last_refresh: None,
        })
    }

    /// Download and parse the community listfile.
    async fn fetch_listfile(&self) -> ImportResult<HashMap<u32, String>> {
        let response = self
            .client
            .get(LISTFILE_RAW_URL)
            .send()
            .await
            .map_err(ImportError::Network)?;

        if !response.status().is_success() {
            return Err(ImportError::HttpStatus {
                provider: "listfile".to_string(),
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let content = response.text().await.map_err(ImportError::Network)?;
        parse_listfile_content(&content)
    }

    /// Load mappings from disk cache if valid.
    async fn load_from_cache(&mut self) -> ImportResult<()> {
        let cache_file = self.cache_dir.join("mappings.json");
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
            self.file_mappings = entry.mappings;
            self.last_refresh = Some(entry.timestamp);
        }

        Ok(())
    }

    /// Save mappings to disk cache.
    async fn save_to_cache(&self) -> ImportResult<()> {
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(ImportError::Io)?;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            mappings: self.file_mappings.clone(),
            timestamp: now,
        };

        let json = serde_json::to_string_pretty(&entry).map_err(ImportError::Json)?;
        tokio::fs::write(self.cache_dir.join("mappings.json"), json)
            .await
            .map_err(ImportError::Io)?;

        Ok(())
    }

    /// Get a reference to the in-memory file mappings.
    pub fn file_mappings(&self) -> &HashMap<u32, String> {
        &self.file_mappings
    }

    /// Check if the cache is stale or empty.
    fn cache_needs_refresh(&self) -> bool {
        if self.file_mappings.is_empty() {
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
impl ImportProvider for ListfileProvider {
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
            && let Ok(mappings) = self.fetch_listfile().await
        {
            self.file_mappings = mappings;
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
            .head(LISTFILE_RAW_URL)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn get_builds(&self, _product: &str) -> ImportResult<Vec<BuildInfo>> {
        Ok(Vec::new())
    }

    async fn resolve_file_id(&self, file_id: u32) -> ImportResult<Option<String>> {
        Ok(self.file_mappings.get(&file_id).cloned())
    }

    async fn get_file_mappings(&self, file_ids: &[u32]) -> ImportResult<Vec<FileMapping>> {
        let mut mappings = Vec::new();
        for &file_id in file_ids {
            if let Some(path) = self.file_mappings.get(&file_id) {
                let filename = path.split('/').next_back().unwrap_or(path).to_string();
                let directory = path
                    .rsplit_once('/')
                    .map_or(String::new(), |x| x.0.to_string());
                mappings.push(FileMapping {
                    file_id,
                    path: path.clone(),
                    filename,
                    directory,
                });
            }
        }
        Ok(mappings)
    }

    async fn refresh_cache(&mut self) -> ImportResult<()> {
        let mappings = self.fetch_listfile().await?;
        self.file_mappings = mappings;
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
            entries: self.file_mappings.len(),
            hits: 0,
            misses: 0,
            last_refresh: self.last_refresh,
            size_bytes: self.file_mappings.len() * (std::mem::size_of::<u32>() + 50),
        })
    }

    fn get_all_file_ids(&self) -> Vec<u32> {
        self.file_mappings.keys().copied().collect()
    }
}

/// Parse listfile CSV content (`FileDataID;Path` per line).
///
/// Lines starting with `#` and empty lines are skipped.
/// Backslashes in paths are normalized to forward slashes.
fn parse_listfile_content(content: &str) -> ImportResult<HashMap<u32, String>> {
    let mut mappings = HashMap::new();
    let reader = BufReader::new(content.as_bytes());

    for line in reader.lines() {
        let line = line.map_err(ImportError::Io)?;
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((id_str, path)) = line.split_once(';')
            && let Ok(file_id) = id_str.trim().parse::<u32>()
        {
            mappings.insert(file_id, path.trim().replace('\\', "/"));
        }
    }

    Ok(mappings)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_listfile_parsing() {
        let content = r"# Comment line
123456;world/maps/azeroth/azeroth.wmo
789012;creature/human/male/humanmale.m2

345678;sound/music/zonemusic/stormwind.mp3
";

        let mappings = parse_listfile_content(content).expect("parse should succeed");

        assert_eq!(mappings.len(), 3);
        assert_eq!(
            mappings.get(&123_456),
            Some(&"world/maps/azeroth/azeroth.wmo".to_string())
        );
        assert_eq!(
            mappings.get(&789_012),
            Some(&"creature/human/male/humanmale.m2".to_string())
        );
        assert_eq!(
            mappings.get(&345_678),
            Some(&"sound/music/zonemusic/stormwind.mp3".to_string())
        );
    }

    #[test]
    fn test_path_normalization() {
        let content = "123456;world\\maps\\azeroth\\azeroth.wmo";
        let mappings = parse_listfile_content(content).expect("parse should succeed");
        assert_eq!(
            mappings.get(&123_456),
            Some(&"world/maps/azeroth/azeroth.wmo".to_string())
        );
    }

    #[test]
    fn test_provider_creation() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let provider = ListfileProvider::new(dir.path().to_path_buf()).expect("create provider");
        assert_eq!(provider.info().source, DataSource::Listfile);
        assert!(!provider.info().capabilities.builds);
        assert!(provider.info().capabilities.file_mappings);
    }
}
