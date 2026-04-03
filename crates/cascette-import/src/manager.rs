//! Import manager for coordinating multiple data providers.

use crate::error::{ImportError, ImportResult};
use crate::providers::{BuildSearchCriteria, CacheStats, DataSource, ImportProvider};
use crate::types::BuildInfo;
use std::collections::HashMap;

/// Import manager that coordinates multiple data providers.
///
/// Providers are registered by name and queried in registration order.
/// The manager aggregates results across providers and maintains
/// in-memory caches for builds and file mappings.
pub struct ImportManager {
    providers: HashMap<String, Box<dyn ImportProvider>>,
    provider_health: HashMap<String, bool>,
    build_cache: HashMap<String, Vec<BuildInfo>>,
    file_mapping_cache: HashMap<u32, String>,
}

impl ImportManager {
    /// Create a new import manager.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            provider_health: HashMap::new(),
            build_cache: HashMap::new(),
            file_mapping_cache: HashMap::new(),
        }
    }

    /// Add a provider to the manager.
    ///
    /// The provider is initialized and its availability is checked.
    pub async fn add_provider(
        &mut self,
        name: &str,
        mut provider: Box<dyn ImportProvider>,
    ) -> ImportResult<()> {
        provider.initialize().await?;

        let is_available = provider.is_available().await;
        self.provider_health.insert(name.to_string(), is_available);
        self.providers.insert(name.to_string(), provider);

        Ok(())
    }

    /// Remove a provider from the manager.
    pub fn remove_provider(&mut self, name: &str) -> ImportResult<()> {
        if self.providers.remove(name).is_some() {
            self.provider_health.remove(name);
            Ok(())
        } else {
            Err(ImportError::NotFound(format!("Provider: {name}")))
        }
    }

    /// List registered provider names.
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Check if a provider is registered.
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// Get provider health status.
    pub fn get_provider_health(&self, name: &str) -> Option<bool> {
        self.provider_health.get(name).copied()
    }

    /// Get builds for a product from all healthy providers.
    ///
    /// Pass an empty string to return builds for all products.
    /// Results are deduplicated by (product, version, build number) and
    /// cached in memory until [`refresh_all_caches`](Self::refresh_all_caches)
    /// is called.
    pub async fn get_builds(&self, product: &str) -> ImportResult<Vec<BuildInfo>> {
        let cache_key = product.to_string();
        if let Some(cached) = self.build_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let mut all_builds = Vec::new();
        let mut errors = Vec::new();

        for (name, provider) in &self.providers {
            if self.get_provider_health(name) == Some(false) {
                continue;
            }
            match provider.get_builds(product).await {
                Ok(mut builds) => all_builds.append(&mut builds),
                Err(e) => errors.push((name.clone(), e)),
            }
        }

        if all_builds.is_empty() && !errors.is_empty() {
            let error_msg = errors
                .iter()
                .map(|(name, err)| format!("{name}: {err}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(ImportError::Provider {
                provider: "all".to_string(),
                message: format!("all providers failed: {error_msg}"),
            });
        }

        dedup_builds(&mut all_builds);
        Ok(all_builds)
    }

    /// Get all builds from all providers.
    pub async fn get_all_builds(&self) -> ImportResult<Vec<BuildInfo>> {
        let mut all_builds = Vec::new();

        for (name, provider) in &self.providers {
            if self.get_provider_health(name) == Some(false) {
                continue;
            }
            if let Ok(mut builds) = provider.get_builds("").await {
                all_builds.append(&mut builds);
            }
        }

        dedup_builds(&mut all_builds);
        Ok(all_builds)
    }

    /// Resolve a file ID to its path using listfile providers.
    pub async fn resolve_file_id(&self, file_id: u32) -> ImportResult<Option<String>> {
        if let Some(path) = self.file_mapping_cache.get(&file_id) {
            return Ok(Some(path.clone()));
        }

        for (name, provider) in &self.providers {
            if self.get_provider_health(name) == Some(false) {
                continue;
            }
            if !provider.info().capabilities.file_mappings {
                continue;
            }
            if let Ok(Some(path)) = provider.resolve_file_id(file_id).await {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    /// Search builds across all providers.
    pub async fn search_builds(
        &self,
        criteria: &BuildSearchCriteria,
    ) -> ImportResult<Vec<BuildInfo>> {
        let mut results = Vec::new();

        for (name, provider) in &self.providers {
            if self.get_provider_health(name) == Some(false) {
                continue;
            }
            if !provider.info().capabilities.builds {
                continue;
            }
            if let Ok(mut builds) = provider.search_builds(criteria).await {
                results.append(&mut builds);
            }
        }

        dedup_builds(&mut results);
        Ok(results)
    }

    /// Refresh all provider caches and clear in-memory caches.
    pub async fn refresh_all_caches(&mut self) -> ImportResult<()> {
        self.build_cache.clear();
        self.file_mapping_cache.clear();

        for provider in self.providers.values_mut() {
            let _ = provider.refresh_cache().await;
        }

        Ok(())
    }

    /// Get cache statistics from all providers.
    pub async fn get_cache_stats(&self) -> HashMap<String, CacheStats> {
        let mut stats = HashMap::new();
        for (name, provider) in &self.providers {
            if let Ok(provider_stats) = provider.cache_stats().await {
                stats.insert(name.clone(), provider_stats);
            }
        }
        stats
    }

    /// Run health checks on all providers and update status.
    pub async fn health_check_all(&mut self) -> HashMap<String, bool> {
        let mut status = HashMap::new();

        for (name, provider) in &self.providers {
            let healthy = provider.is_available().await;
            status.insert(name.clone(), healthy);
        }

        self.provider_health
            .extend(status.iter().map(|(k, &v)| (k.clone(), v)));

        status
    }

    /// Get providers that match a specific data source type.
    pub fn get_providers_for_source(&self, source: DataSource) -> Vec<String> {
        self.providers
            .iter()
            .filter(|(_, p)| p.info().source == source)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl Default for ImportManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort and deduplicate builds by (product, version, build number).
fn dedup_builds(builds: &mut Vec<BuildInfo>) {
    builds.sort_by(|a, b| {
        a.product
            .cmp(&b.product)
            .then(a.version.cmp(&b.version))
            .then(a.build.cmp(&b.build))
    });
    builds.dedup_by(|a, b| a.product == b.product && a.version == b.version && a.build == b.build);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::providers::ImportProviderInfo;
    use crate::types::ProviderCapabilities;
    use async_trait::async_trait;
    use std::time::Duration;

    struct MockProvider {
        info: ImportProviderInfo,
        available: bool,
    }

    impl MockProvider {
        fn new(source: DataSource, name: &str) -> Self {
            Self {
                info: ImportProviderInfo {
                    source,
                    name: name.to_string(),
                    description: "Mock provider for testing".to_string(),
                    version: "1.0.0".to_string(),
                    endpoint: None,
                    capabilities: ProviderCapabilities {
                        builds: true,
                        file_mappings: false,
                        real_time: false,
                        requires_auth: false,
                    },
                    rate_limit: None,
                    cache_ttl: Duration::from_secs(300),
                },
                available: true,
            }
        }
    }

    #[async_trait]
    impl ImportProvider for MockProvider {
        fn info(&self) -> &ImportProviderInfo {
            &self.info
        }

        async fn initialize(&mut self) -> ImportResult<()> {
            Ok(())
        }

        async fn is_available(&self) -> bool {
            self.available
        }

        async fn get_builds(&self, product: &str) -> ImportResult<Vec<BuildInfo>> {
            Ok(vec![BuildInfo {
                product: product.to_string(),
                version: "1.0.0".to_string(),
                build: 12_345,
                version_type: "live".to_string(),
                region: None,
                timestamp: None,
                created_at: None,
                metadata: HashMap::new(),
            }])
        }
    }

    #[tokio::test]
    async fn test_manager_basic_operations() {
        let mut manager = ImportManager::new();

        assert!(manager.list_providers().is_empty());
        assert!(!manager.has_provider("test"));

        let provider = MockProvider::new(DataSource::Wago, "test-provider");
        manager
            .add_provider("test", Box::new(provider))
            .await
            .expect("add_provider should succeed");

        assert_eq!(manager.list_providers().len(), 1);
        assert!(manager.has_provider("test"));

        let builds = manager
            .get_builds("wow")
            .await
            .expect("get_builds should succeed");
        assert!(!builds.is_empty());

        manager
            .remove_provider("test")
            .expect("remove_provider should succeed");
        assert!(manager.list_providers().is_empty());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_provider() {
        let mut manager = ImportManager::new();
        let result = manager.remove_provider("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let mut manager = ImportManager::new();
        let provider = MockProvider::new(DataSource::Wago, "test");
        manager
            .add_provider("test", Box::new(provider))
            .await
            .expect("add_provider should succeed");

        let health = manager.health_check_all().await;
        assert_eq!(health.get("test"), Some(&true));
    }

    #[tokio::test]
    async fn test_get_providers_for_source() {
        let mut manager = ImportManager::new();
        let provider = MockProvider::new(DataSource::Wago, "wago");
        manager
            .add_provider("wago", Box::new(provider))
            .await
            .expect("add_provider should succeed");

        let wago_providers = manager.get_providers_for_source(DataSource::Wago);
        assert_eq!(wago_providers.len(), 1);

        let listfile_providers = manager.get_providers_for_source(DataSource::Listfile);
        assert!(listfile_providers.is_empty());
    }

    #[test]
    fn test_dedup_builds() {
        let mut builds = vec![
            BuildInfo {
                product: "wow".to_string(),
                version: "1.0.0".to_string(),
                build: 100,
                version_type: "live".to_string(),
                region: None,
                timestamp: None,
                created_at: None,
                metadata: HashMap::new(),
            },
            BuildInfo {
                product: "wow".to_string(),
                version: "1.0.0".to_string(),
                build: 100,
                version_type: "live".to_string(),
                region: None,
                timestamp: None,
                created_at: None,
                metadata: HashMap::new(),
            },
            BuildInfo {
                product: "wow".to_string(),
                version: "2.0.0".to_string(),
                build: 200,
                version_type: "live".to_string(),
                region: None,
                timestamp: None,
                created_at: None,
                metadata: HashMap::new(),
            },
        ];

        dedup_builds(&mut builds);
        assert_eq!(builds.len(), 2);
    }
}
