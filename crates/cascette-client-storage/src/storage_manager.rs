//! Main storage system implementation
//!
//! Implements the official CASC directory structure as specified in wowdev.wiki:
//! `INSTALL_DIR\Data\data\` with proper validation and organization.

use crate::{Installation, Result, StorageConfig};
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

/// Main storage system for managing CASC installations
pub struct Storage {
    config: StorageConfig,
    installations: DashMap<String, Arc<Installation>>,
    base_path: PathBuf,
}

impl Storage {
    /// Create a new storage system with the given configuration
    ///
    /// # Errors
    ///
    /// Returns error if base directory cannot be created
    pub fn new(config: StorageConfig) -> Result<Self> {
        // Ensure base path exists
        if !config.base_path.exists() {
            info!(
                "Creating CASC storage directory: {}",
                config.base_path.display()
            );
            std::fs::create_dir_all(&config.base_path)?;
        }

        // Validate and create official CASC directory structure
        Self::validate_casc_directory_structure(&config.base_path)?;

        Ok(Self {
            base_path: config.base_path.clone(),
            config,
            installations: DashMap::new(),
        })
    }

    /// Open an existing installation or create a new one
    ///
    /// # Errors
    ///
    /// Returns error if installation cannot be opened or created
    pub fn open_installation(&self, name: &str) -> Result<Arc<Installation>> {
        if let Some(installation) = self.installations.get(name) {
            return Ok(installation.clone());
        }

        let installation_path = self.base_path.join(name);
        let installation = Arc::new(Installation::open(installation_path)?);

        self.installations
            .insert(name.to_string(), installation.clone());
        Ok(installation)
    }

    /// Returns names of all currently open installations.
    pub fn list_installations(&self) -> Vec<String> {
        self.installations
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Base directory for CASC storage.
    pub const fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Storage configuration used at creation time.
    pub const fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Validate and create official CASC directory structure.
    ///
    /// CASC creates five subdirectories under the storage root:
    /// - `data/` -- dynamic container (.idx + .data files, shmem temp file)
    /// - `indices/` -- CDN index cache (.index files)
    /// - `residency/` -- residency tracking database
    /// - `ecache/` -- e-header cache (preservation set)
    /// - `hardlink/` -- hard link container trie directory
    ///
    /// Build/CDN config files are stored inside the dynamic container,
    /// not in a separate `config/` directory. Shared memory uses named
    /// kernel objects + a temp file in `data/`, not a `shmem/` directory.
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be created or validated
    fn validate_casc_directory_structure(base_path: &std::path::Path) -> Result<()> {
        use crate::{DATA_DIR, ECACHE_DIR, HARDLINK_DIR, INDICES_DIR, RESIDENCY_DIR};

        // CASC creates five subdirectories under the storage root.
        // tact::BuildRepairState::RepairContainers.
        let required_dirs = [
            ("data", DATA_DIR),
            ("indices", INDICES_DIR),
            ("residency", RESIDENCY_DIR),
            ("ecache", ECACHE_DIR),
            ("hardlink", HARDLINK_DIR),
        ];

        for (desc, dir_name) in &required_dirs {
            let dir_path = base_path.join(dir_name);

            if dir_path.exists() {
                // Validate existing directory is accessible
                if !dir_path.is_dir() {
                    return Err(crate::StorageError::Config(format!(
                        "{desc} path exists but is not a directory: {}",
                        dir_path.display()
                    )));
                }

                // Check write permissions
                let test_file = dir_path.join(".casc_test");
                if let Err(e) = std::fs::write(&test_file, b"test") {
                    warn!(
                        "No write permission for {desc} directory {}: {e}",
                        dir_path.display()
                    );
                } else {
                    let _ = std::fs::remove_file(test_file); // Clean up test file
                }
            } else {
                info!("Creating CASC {desc} directory: {}", dir_path.display());
                std::fs::create_dir_all(&dir_path).map_err(|e| {
                    crate::StorageError::Config(format!("Failed to create {desc} directory: {e}"))
                })?;
            }
        }

        info!(
            "CASC directory structure validated at: {}",
            base_path.display()
        );
        Ok(())
    }

    /// Get the data directory path.
    ///
    /// CASC stores `.idx`, `.data`, LRU, KMT, and shmem temp files
    /// all in `Data/data/`.
    pub fn data_path(&self) -> PathBuf {
        self.base_path.join(crate::DATA_DIR)
    }

    /// Get the CDN indices directory path.
    ///
    /// Downloaded CDN archive indices (`.index` files) are cached here.
    pub fn indices_path(&self) -> PathBuf {
        self.base_path.join(crate::INDICES_DIR)
    }

    /// Get the residency container directory path.
    pub fn residency_path(&self) -> PathBuf {
        self.base_path.join(crate::RESIDENCY_DIR)
    }

    /// Get the e-header cache directory path.
    pub fn ecache_path(&self) -> PathBuf {
        self.base_path.join(crate::ECACHE_DIR)
    }

    /// Get the hard link container directory path.
    pub fn hardlink_path(&self) -> PathBuf {
        self.base_path.join(crate::HARDLINK_DIR)
    }

    /// Get the `.build.info` file path at the installation root.
    ///
    /// CASC reads `.build.info` from the top-level installation
    /// directory to determine product, region, build config, and CDN config.
    pub fn build_info_path(&self) -> PathBuf {
        self.base_path.join(crate::BUILD_INFO_FILE)
    }
}
