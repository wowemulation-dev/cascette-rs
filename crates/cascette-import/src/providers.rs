//! Import provider trait and types.

use crate::error::ImportResult;
use crate::types::{BuildInfo, FileMapping, ProviderCapabilities};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

/// Data source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataSource {
    /// wago.tools build information.
    Wago,
    /// WoWDev community listfile.
    Listfile,
    /// WoWDev TACT keys repository.
    TactKeys,
    /// BlizzTrack TACT product archive.
    BlizzTrack,
    /// Custom/third-party provider.
    Custom(&'static str),
}

impl DataSource {
    /// Get display name for the data source.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wago => "wago.tools",
            Self::Listfile => "WoWDev Listfile",
            Self::TactKeys => "WoWDev TACT Keys",
            Self::BlizzTrack => "BlizzTrack",
            Self::Custom(name) => name,
        }
    }

    /// Get unique identifier string for the data source.
    pub fn id(self) -> &'static str {
        match self {
            Self::Wago => "wago",
            Self::Listfile => "listfile",
            Self::TactKeys => "tactkeys",
            Self::BlizzTrack => "blizztrack",
            Self::Custom(id) => id,
        }
    }
}

/// Provider information and metadata.
#[derive(Debug, Clone)]
pub struct ImportProviderInfo {
    /// Data source type.
    pub source: DataSource,
    /// Provider name.
    pub name: String,
    /// Provider description.
    pub description: String,
    /// Provider version.
    pub version: String,
    /// Base URL or endpoint.
    pub endpoint: Option<String>,
    /// Provider capabilities.
    pub capabilities: ProviderCapabilities,
    /// Rate limit (requests per minute).
    pub rate_limit: Option<u32>,
    /// Cache time to live.
    pub cache_ttl: Duration,
}

/// Trait for import data providers.
///
/// Each provider integrates with a community data source (wago.tools,
/// WoWDev listfile, WoWDev TACT keys) and exposes build information,
/// file mappings, or encryption keys through a uniform interface.
#[async_trait]
pub trait ImportProvider: Send + Sync {
    /// Get provider information.
    fn info(&self) -> &ImportProviderInfo;

    /// Initialize the provider (authenticate, validate config, etc.).
    async fn initialize(&mut self) -> ImportResult<()>;

    /// Check if the provider is reachable.
    async fn is_available(&self) -> bool;

    /// Get build information for a product.
    ///
    /// Pass an empty string to return builds for all products.
    async fn get_builds(&self, product: &str) -> ImportResult<Vec<BuildInfo>>;

    /// Resolve a file ID to its path using community listfile data.
    ///
    /// Returns `Ok(None)` if the provider does not support file mappings
    /// or the ID is not found.
    async fn resolve_file_id(&self, _file_id: u32) -> ImportResult<Option<String>> {
        Ok(None)
    }

    /// Get file mappings for a set of file IDs.
    async fn get_file_mappings(&self, file_ids: &[u32]) -> ImportResult<Vec<FileMapping>> {
        let mut mappings = Vec::new();
        for &file_id in file_ids {
            if let Some(path) = self.resolve_file_id(file_id).await? {
                let filename = path.split('/').next_back().unwrap_or(&path).to_string();
                let directory = path
                    .rsplit_once('/')
                    .map_or(String::new(), |x| x.0.to_string());
                mappings.push(FileMapping {
                    file_id,
                    path,
                    filename,
                    directory,
                });
            }
        }
        Ok(mappings)
    }

    /// Search for builds matching criteria.
    async fn search_builds(&self, criteria: &BuildSearchCriteria) -> ImportResult<Vec<BuildInfo>> {
        let builds = self.get_builds("").await?;
        Ok(builds
            .into_iter()
            .filter(|build| criteria.matches(build))
            .collect())
    }

    /// Refresh cached data from the upstream source.
    async fn refresh_cache(&mut self) -> ImportResult<()> {
        Ok(())
    }

    /// Get cache statistics.
    async fn cache_stats(&self) -> ImportResult<CacheStats> {
        Ok(CacheStats::default())
    }

    /// Get all file IDs available from this provider.
    ///
    /// Returns an empty vector if enumeration is not supported.
    fn get_all_file_ids(&self) -> Vec<u32> {
        Vec::new()
    }
}

/// Build search criteria.
#[derive(Debug, Clone, Default)]
pub struct BuildSearchCriteria {
    /// Product filter.
    pub product: Option<String>,
    /// Version pattern (supports `*` wildcards).
    pub version_pattern: Option<String>,
    /// Minimum build number.
    pub min_build: Option<u32>,
    /// Maximum build number.
    pub max_build: Option<u32>,
    /// Version type filter (e.g., "live", "ptr", "beta").
    pub version_type: Option<String>,
    /// Region filter.
    pub region: Option<String>,
    /// Additional metadata filters.
    pub metadata_filters: HashMap<String, String>,
}

impl BuildSearchCriteria {
    /// Check if a build matches these criteria.
    pub fn matches(&self, build: &BuildInfo) -> bool {
        if let Some(ref product) = self.product
            && build.product != *product
        {
            return false;
        }

        if let Some(ref pattern) = self.version_pattern
            && !matches_pattern(&build.version, pattern)
        {
            return false;
        }

        if let Some(min_build) = self.min_build
            && build.build < min_build
        {
            return false;
        }

        if let Some(max_build) = self.max_build
            && build.build > max_build
        {
            return false;
        }

        if let Some(ref version_type) = self.version_type
            && build.version_type != *version_type
        {
            return false;
        }

        if let Some(ref region) = self.region {
            match &build.region {
                Some(build_region) if build_region == region => {}
                _ => return false,
            }
        }

        for (key, expected_value) in &self.metadata_filters {
            match build.metadata.get(key) {
                Some(actual_value) if actual_value == expected_value => {}
                _ => return false,
            }
        }

        true
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cached entries.
    pub entries: usize,
    /// Cache hit count.
    pub hits: u64,
    /// Cache miss count.
    pub misses: u64,
    /// Last refresh as Unix timestamp seconds.
    pub last_refresh: Option<u64>,
    /// Cache size in bytes (estimate).
    pub size_bytes: usize,
}

impl CacheStats {
    /// Calculate cache hit rate as a percentage.
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Pattern matching with `*` wildcard support.
fn matches_pattern(text: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return text == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut pos = 0;

    // First part must match at the beginning.
    if !parts[0].is_empty() {
        if !text.starts_with(parts[0]) {
            return false;
        }
        pos += parts[0].len();
    }

    // Middle parts can match anywhere.
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        if let Some(found_pos) = text[pos..].find(part) {
            pos += found_pos + part.len();
        } else {
            return false;
        }
    }

    // Last part must match at the end.
    if let Some(last_part) = parts.last()
        && !last_part.is_empty()
    {
        return text[pos..].ends_with(last_part);
    }

    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("1.15.2.55140", "*"));
        assert!(matches_pattern("1.15.2.55140", "1.15.*"));
        assert!(matches_pattern("1.15.2.55140", "*.55140"));
        assert!(matches_pattern("1.15.2.55140", "1.15.2.55140"));
        assert!(!matches_pattern("1.15.2.55140", "1.14.*"));
        assert!(!matches_pattern("1.15.2.55140", "*.55141"));
    }

    #[test]
    fn test_build_search_criteria() {
        let build = BuildInfo {
            product: "wow".to_string(),
            version: "1.15.2.55140".to_string(),
            build: 55140,
            version_type: "live".to_string(),
            region: Some("us".to_string()),
            timestamp: None,
            created_at: None,
            metadata: HashMap::new(),
        };

        let mut criteria = BuildSearchCriteria::default();
        assert!(criteria.matches(&build));

        criteria.product = Some("wow".to_string());
        assert!(criteria.matches(&build));

        criteria.product = Some("diablo4".to_string());
        assert!(!criteria.matches(&build));

        criteria.product = Some("wow".to_string());
        criteria.min_build = Some(55000);
        assert!(criteria.matches(&build));

        criteria.min_build = Some(60000);
        assert!(!criteria.matches(&build));
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            ..CacheStats::default()
        };
        let rate = stats.hit_rate();
        assert!((rate - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_empty() {
        let stats = CacheStats::default();
        assert!((stats.hit_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_data_source_id_and_name() {
        assert_eq!(DataSource::Wago.id(), "wago");
        assert_eq!(DataSource::Wago.display_name(), "wago.tools");
        assert_eq!(DataSource::Custom("test").id(), "test");
    }
}
