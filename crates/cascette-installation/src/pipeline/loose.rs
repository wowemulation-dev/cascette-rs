//! Loose file handler for product directory hardlinks.
//!
//! Handles file completion by creating hardlinks (or copies when hardlinks
//! are not supported) from CASC archives to the product directory.
//! Matches Blizzard Agent's loose file handling behavior.

use std::collections::HashSet;
use std::path::PathBuf;

use tracing::{debug, info, warn};

use cascette_client_storage::Installation;
use cascette_client_storage::container::AccessMode;
use cascette_client_storage::container::hardlink::HardLinkContainer;

use crate::error::InstallationResult;
use crate::extract::validate_output_path;

/// Handles loose file operations for a product directory.
pub struct LooseFileHandler {
    subfolder: String,
    install_path: PathBuf,
    hardlink_container: HardLinkContainer,
    hardlinks_supported: bool,
    completed: HashSet<[u8; 16]>,
}

/// Report from loose file operations.
#[derive(Debug, Default)]
pub struct LooseFileReport {
    /// Number of files linked via hardlink.
    pub linked: usize,
    /// Number of files copied (hardlink fallback).
    pub copied: usize,
    /// Number of failed operations.
    pub failed: usize,
}

impl LooseFileHandler {
    /// Create and initialize a loose file handler.
    ///
    /// Tests hardlink support between the CASC data directory and the
    /// product directory.
    pub fn new(subfolder: String, install_path: PathBuf) -> InstallationResult<Self> {
        let data_dir = install_path.join("Data").join("data");
        let product_dir = install_path.join(&subfolder);

        // Ensure product directory exists
        std::fs::create_dir_all(&product_dir)?;

        let mut hardlink_container =
            HardLinkContainer::new(AccessMode::ReadWrite, product_dir.clone());

        let hardlinks_supported = hardlink_container
            .test_support(&data_dir, &product_dir)
            .unwrap_or(false);

        if hardlinks_supported {
            info!("loose file hardlinks allowed");
        } else {
            info!("reducing maximum duplicate extraction size due to drive type");
        }

        Ok(Self {
            subfolder,
            install_path,
            hardlink_container,
            hardlinks_supported,
            completed: HashSet::new(),
        })
    }

    /// Handle file completion: create hardlink or copy to product directory.
    pub async fn on_file_complete(
        &mut self,
        ekey: &[u8; 16],
        file_path: &str,
        installation: &Installation,
        key_store: Option<&(dyn cascette_crypto::TactKeyProvider + Send + Sync)>,
    ) -> InstallationResult<()> {
        if self.completed.contains(ekey) {
            return Ok(());
        }

        let product_dir = self.install_path.join(&self.subfolder);
        let destination = product_dir.join(file_path);

        // Reject paths that escape the product directory (e.g. "../../../etc/passwd")
        validate_output_path(&product_dir, &destination)?;

        // Ensure parent directories exist
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if self.hardlinks_supported {
            let source = installation.path().join("data");
            match self
                .hardlink_container
                .create_link(ekey, &source, &destination)
            {
                Ok(()) => {
                    debug!(path = %file_path, "hardlink created");
                    self.completed.insert(*ekey);
                    return Ok(());
                }
                Err(e) => {
                    warn!(path = %file_path, error = %e, "hardlink failed, falling back to copy");
                }
            }
        }

        // Fallback: read from CASC and write to product directory
        let ekey_obj = cascette_crypto::EncodingKey::from_bytes(*ekey);
        let data = if let Some(keys) = key_store {
            installation
                .read_file_by_encoding_key_with_keys(&ekey_obj, keys)
                .await?
        } else {
            installation.read_file_by_encoding_key(&ekey_obj).await?
        };
        tokio::fs::write(&destination, &data).await?;
        debug!(path = %file_path, "loose file copied");
        self.completed.insert(*ekey);

        Ok(())
    }

    /// Check if a file has already been completed.
    pub fn is_completed(&self, ekey: &[u8; 16]) -> bool {
        self.completed.contains(ekey)
    }

    /// Get the current report.
    pub fn report(&self) -> LooseFileReport {
        LooseFileReport {
            linked: if self.hardlinks_supported {
                self.completed.len()
            } else {
                0
            },
            copied: if self.hardlinks_supported {
                0
            } else {
                self.completed.len()
            },
            failed: 0,
        }
    }
}
