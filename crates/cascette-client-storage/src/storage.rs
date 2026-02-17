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

    /// List all available installations
    pub fn list_installations(&self) -> Vec<String> {
        self.installations
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get the base storage path
    pub const fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Get the storage configuration
    pub const fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Validate and create official CASC directory structure
    /// Based on wowdev.wiki CASC specification: `INSTALL_DIR\Data\data\`
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be created or validated
    fn validate_casc_directory_structure(base_path: &std::path::Path) -> Result<()> {
        use crate::{CONFIG_DIR, DATA_DIR, INDICES_DIR, SHMEM_DIR};

        // Create official CASC subdirectories
        let required_dirs = [
            ("indices", INDICES_DIR),
            ("data", DATA_DIR),
            ("config", CONFIG_DIR),
            ("shmem", SHMEM_DIR),
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

    /// Get the indices directory path (for .idx files)
    pub fn indices_path(&self) -> PathBuf {
        self.base_path.join(crate::INDICES_DIR)
    }

    /// Get the data directory path (for .data files)
    pub fn data_path(&self) -> PathBuf {
        self.base_path.join(crate::DATA_DIR)
    }

    /// Get the config directory path (for configuration files)
    pub fn config_path(&self) -> PathBuf {
        self.base_path.join(crate::CONFIG_DIR)
    }

    /// Get the shmem directory path (for shared memory files)
    pub fn shmem_path(&self) -> PathBuf {
        self.base_path.join(crate::SHMEM_DIR)
    }
}
