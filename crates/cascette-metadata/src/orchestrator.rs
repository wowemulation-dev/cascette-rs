//! Metadata orchestrator combining FDID resolution and TACT key access.
//!
//! Coordinates [`FileDataIdService`] and a [`TactKeyProvider`] backend under a
//! unified API with health and statistics reporting.

use std::collections::HashMap;

use cascette_crypto::TactKeyProvider;
#[cfg(feature = "import")]
use cascette_crypto::TactKeyStore;

use crate::content::{ContentCategory, ContentInfo};
use crate::error::MetadataResult;
use crate::fdid::FileDataIdService;

/// Orchestrator configuration.
#[derive(Debug, Clone, Copy)]
pub struct OrchestratorConfig {
    /// Whether to include hardcoded WoW keys when building the key store
    /// from import providers.
    pub include_hardcoded_keys: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            include_hardcoded_keys: true,
        }
    }
}

/// Aggregate statistics.
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    /// Number of FileDataID mappings loaded.
    pub fdid_count: usize,
    /// Number of TACT keys available.
    pub tact_key_count: usize,
    /// Whether the FDID service has data.
    pub fdid_ready: bool,
    /// Whether the key provider has data.
    pub keys_ready: bool,
}

/// Health status of the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// All subsystems operational.
    Healthy,
    /// Partially operational.
    Degraded {
        /// Description of the degradation.
        reason: String,
    },
}

/// Coordinates FDID resolution and TACT key access.
pub struct MetadataOrchestrator {
    fdid: FileDataIdService,
    keys: Box<dyn TactKeyProvider>,
}

impl MetadataOrchestrator {
    /// Build from raw data with any `TactKeyProvider` backend.
    pub fn from_raw(
        mappings: HashMap<u32, String>,
        keys: impl TactKeyProvider + 'static,
        _config: OrchestratorConfig,
    ) -> Self {
        Self {
            fdid: FileDataIdService::from_map(mappings),
            keys: Box::new(keys),
        }
    }

    /// Build from initialized import providers.
    ///
    /// Both providers must have been initialized (with `initialize().await`)
    /// before calling this constructor. This method only reads in-memory data.
    #[cfg(feature = "import")]
    pub fn from_providers(
        listfile: &cascette_import::ListfileProvider,
        tact_keys: &cascette_import::TactKeysProvider,
        config: OrchestratorConfig,
    ) -> Self {
        let fdid = FileDataIdService::from_listfile_provider(listfile);

        let mut store = if config.include_hardcoded_keys {
            TactKeyStore::new()
        } else {
            TactKeyStore::empty()
        };

        for key in tact_keys.get_all_tact_keys() {
            store.add(key);
        }

        Self {
            fdid,
            keys: Box::new(store),
        }
    }

    // --- FDID delegation ---

    /// Resolve a FileDataID to its path.
    pub fn resolve_id(&self, id: u32) -> MetadataResult<&str> {
        self.fdid.resolve_id(id)
    }

    /// Resolve a file path to its FileDataID (case-insensitive).
    pub fn resolve_path(&self, path: &str) -> MetadataResult<u32> {
        self.fdid.resolve_path(path)
    }

    /// Get content metadata for a FileDataID.
    pub fn content_info(&self, id: u32) -> MetadataResult<ContentInfo> {
        self.fdid.content_info(id)
    }

    /// Get the content category for a FileDataID.
    pub fn content_category(&self, id: u32) -> MetadataResult<ContentCategory> {
        Ok(self.fdid.content_info(id)?.category)
    }

    // --- TACT key delegation ---

    /// Look up a TACT encryption key by ID.
    pub fn get_tact_key(&self, id: u64) -> Option<[u8; 16]> {
        self.keys.get_key(id).ok().flatten()
    }

    /// Check whether a TACT key is available.
    pub fn has_tact_key(&self, id: u64) -> bool {
        self.keys.contains_key(id).unwrap_or(false)
    }

    // --- Combined queries ---

    /// Check whether a file is likely encrypted based on its content category.
    pub fn is_likely_encrypted(&self, id: u32) -> MetadataResult<bool> {
        Ok(self.content_category(id)?.is_likely_encrypted())
    }

    // --- Diagnostics ---

    /// Get aggregate statistics.
    pub fn stats(&self) -> OrchestratorStats {
        let fdid_count = self.fdid.len();
        let tact_key_count = self.keys.key_count().unwrap_or(0);

        OrchestratorStats {
            fdid_count,
            tact_key_count,
            fdid_ready: fdid_count > 0,
            keys_ready: tact_key_count > 0,
        }
    }

    /// Evaluate health of the orchestrator.
    pub fn health(&self) -> HealthStatus {
        let stats = self.stats();

        match (stats.fdid_ready, stats.keys_ready) {
            (true, true) => HealthStatus::Healthy,
            (false, false) => HealthStatus::Degraded {
                reason: "no FileDataID mappings and no TACT keys loaded".to_string(),
            },
            (false, true) => HealthStatus::Degraded {
                reason: "no FileDataID mappings loaded".to_string(),
            },
            (true, false) => HealthStatus::Degraded {
                reason: "no TACT keys loaded".to_string(),
            },
        }
    }

    /// Get a reference to the underlying FDID service.
    pub fn fdid_service(&self) -> &FileDataIdService {
        &self.fdid
    }

    /// Get a reference to the underlying key provider.
    pub fn key_provider(&self) -> &dyn TactKeyProvider {
        &*self.keys
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::error::MetadataError;
    use cascette_crypto::{TactKey, TactKeyStore};

    fn sample_mappings() -> HashMap<u32, String> {
        let mut m = HashMap::new();
        m.insert(100, "world/maps/azeroth/azeroth.wmo".to_string());
        m.insert(200, "dbfilesclient/spell.db2".to_string());
        m.insert(300, "sound/music/zone.mp3".to_string());
        m
    }

    fn sample_key_store() -> TactKeyStore {
        let mut store = TactKeyStore::empty();
        store.add(TactKey::new(0xABCD, [0x42; 16]));
        store
    }

    #[test]
    fn test_from_raw() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert_eq!(
            orch.resolve_id(100).unwrap(),
            "world/maps/azeroth/azeroth.wmo"
        );
        assert_eq!(orch.get_tact_key(0xABCD), Some([0x42; 16]));
    }

    #[test]
    fn test_resolve_path() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert_eq!(orch.resolve_path("sound/music/zone.mp3").unwrap(), 300);
    }

    #[test]
    fn test_content_category() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert_eq!(orch.content_category(200).unwrap(), ContentCategory::Data);
        assert_eq!(orch.content_category(300).unwrap(), ContentCategory::Audio);
    }

    #[test]
    fn test_is_likely_encrypted() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert!(orch.is_likely_encrypted(200).unwrap()); // db2 -> Data -> encrypted
        assert!(!orch.is_likely_encrypted(300).unwrap()); // mp3 -> Audio -> not encrypted
    }

    #[test]
    fn test_tact_key_lookup() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert!(orch.has_tact_key(0xABCD));
        assert!(!orch.has_tact_key(0xFFFF));
        assert_eq!(orch.get_tact_key(0xABCD), Some([0x42; 16]));
        assert_eq!(orch.get_tact_key(0xFFFF), None);
    }

    #[test]
    fn test_stats() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        let stats = orch.stats();
        assert_eq!(stats.fdid_count, 3);
        assert_eq!(stats.tact_key_count, 1);
        assert!(stats.fdid_ready);
        assert!(stats.keys_ready);
    }

    #[test]
    fn test_health_healthy() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert_eq!(orch.health(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_degraded_no_keys() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            TactKeyStore::empty(),
            OrchestratorConfig::default(),
        );

        assert!(matches!(orch.health(), HealthStatus::Degraded { .. }));
    }

    #[test]
    fn test_health_degraded_no_fdids() {
        let orch = MetadataOrchestrator::from_raw(
            HashMap::new(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert!(matches!(orch.health(), HealthStatus::Degraded { .. }));
    }

    #[test]
    fn test_health_degraded_nothing() {
        let orch = MetadataOrchestrator::from_raw(
            HashMap::new(),
            TactKeyStore::empty(),
            OrchestratorConfig::default(),
        );

        match orch.health() {
            HealthStatus::Degraded { reason } => {
                assert!(reason.contains("FileDataID"));
                assert!(reason.contains("TACT keys"));
            }
            HealthStatus::Healthy => panic!("expected degraded"),
        }
    }

    #[test]
    fn test_error_on_missing_fdid() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        let err = orch.resolve_id(999).unwrap_err();
        assert!(matches!(err, MetadataError::FileDataIdNotFound(999)));
    }

    #[test]
    fn test_fdid_service_accessor() {
        let orch = MetadataOrchestrator::from_raw(
            sample_mappings(),
            sample_key_store(),
            OrchestratorConfig::default(),
        );

        assert_eq!(orch.fdid_service().len(), 3);
    }
}
