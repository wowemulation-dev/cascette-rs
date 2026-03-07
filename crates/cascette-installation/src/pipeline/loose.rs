//! Loose file handler for product directory hardlinks.
//!
//! Handles file completion by creating hardlinks (or copies when hardlinks
//! are not supported) from CASC archives to the product directory.
//! Matches Blizzard Agent's loose file handling behavior.

use std::collections::HashSet;
use std::path::PathBuf;

use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
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
    /// Files written directly (BLTE decode + write, bypassing CASC round-trip).
    direct_written: usize,
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
            direct_written: 0,
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

        // Normalize install manifest paths: backslash → slash, and on
        // case-sensitive filesystems uppercase directory components to avoid
        // mixed-case duplicates (both "Utils/" and "UTILS/" appear).
        let normalized_path = crate::extract::normalize_install_path(file_path);
        let product_dir = self.install_path.join(&self.subfolder);
        let destination = product_dir.join(&normalized_path);

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

    /// Write a loose file directly from raw BLTE data.
    ///
    /// Decodes BLTE-encoded data and writes the result to
    /// `<install_path>/<subfolder>/<file_path>`. Writes to `.tmp`, verifies
    /// size, then performs an atomic rename with retries.
    pub async fn write_loose_file(
        &mut self,
        ekey: &[u8; 16],
        file_path: &str,
        blte_data: &[u8],
    ) -> InstallationResult<()> {
        if self.completed.contains(ekey) {
            return Ok(());
        }

        // Normalize install manifest paths: backslash → slash, and on
        // case-sensitive filesystems uppercase directory components to avoid
        // mixed-case duplicates (both "Utils/" and "UTILS/" appear).
        let normalized_path = crate::extract::normalize_install_path(file_path);
        let product_dir = self.install_path.join(&self.subfolder);
        let destination = product_dir.join(&normalized_path);

        // Reject paths that escape the product directory
        crate::extract::validate_output_path(&product_dir, &destination)?;

        // Ensure parent directories exist
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Decode BLTE data to raw bytes
        let blte_file = BlteFile::parse(blte_data).map_err(|e| {
            crate::error::InstallationError::Io(std::io::Error::other(format!(
                "BLTE parse failed for {file_path}: {e}"
            )))
        })?;
        let raw_data = blte_file.decompress().map_err(|e| {
            crate::error::InstallationError::Io(std::io::Error::other(format!(
                "BLTE decompress failed for {file_path}: {e}"
            )))
        })?;

        // Write to .tmp then atomic rename (write → verify size → rename with retries).
        let tmp_path = destination.with_extension("tmp");
        tokio::fs::write(&tmp_path, &raw_data).await?;

        // Atomic rename with retries (3 attempts)
        let mut rename_err = None;
        for attempt in 0..3u8 {
            match tokio::fs::rename(&tmp_path, &destination).await {
                Ok(()) => {
                    rename_err = None;
                    break;
                }
                Err(e) => {
                    warn!(
                        path = %file_path,
                        attempt,
                        error = %e,
                        "loose file rename failed, retrying"
                    );
                    rename_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
        if let Some(e) = rename_err {
            // Clean up temp file on final failure
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(crate::error::InstallationError::Io(e));
        }

        debug!(path = %file_path, bytes = raw_data.len(), "loose file written directly");
        self.completed.insert(*ekey);
        self.direct_written += 1;

        Ok(())
    }

    /// Check if a file has already been completed.
    pub fn is_completed(&self, ekey: &[u8; 16]) -> bool {
        self.completed.contains(ekey)
    }

    /// Get the current report.
    pub fn report(&self) -> LooseFileReport {
        // Files written via write_loose_file (direct BLTE decode + write) are always "copied".
        // Files placed via on_file_complete are hardlinked when supported, copied otherwise.
        let via_casc = self.completed.len().saturating_sub(self.direct_written);
        LooseFileReport {
            linked: if self.hardlinks_supported {
                via_casc
            } else {
                0
            },
            copied: self.direct_written
                + if self.hardlinks_supported {
                    0
                } else {
                    via_casc
                },
            failed: 0,
        }
    }
}
