//! Checkpoint state for resumable downloads.
//!
//! During the download phase, checkpoint state is periodically saved to
//! `{install_path}/Data/.cascette-checkpoint.json`. On resume, the pipeline
//! reads this file to skip already-downloaded files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{InstallationError, InstallationResult};

const CHECKPOINT_FILENAME: &str = ".cascette-checkpoint.json";

/// Persistent checkpoint state for download resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Product code.
    pub product: String,

    /// Build config hash (hex) that this checkpoint applies to.
    pub build_config: String,

    /// CDN config hash (hex) that this checkpoint applies to.
    pub cdn_config: String,

    /// Set of encoding keys (hex) that have been downloaded.
    pub completed_keys: HashSet<String>,

    /// Total number of artifacts in the download plan.
    pub total_artifacts: usize,
}

impl Checkpoint {
    /// Create a new empty checkpoint.
    #[must_use]
    pub fn new(product: String, build_config: String, cdn_config: String, total: usize) -> Self {
        Self {
            product,
            build_config,
            cdn_config,
            completed_keys: HashSet::new(),
            total_artifacts: total,
        }
    }

    /// Number of completed downloads.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.completed_keys.len()
    }

    /// Number of remaining downloads.
    #[must_use]
    pub fn remaining_count(&self) -> usize {
        self.total_artifacts
            .saturating_sub(self.completed_keys.len())
    }

    /// Mark a key as completed.
    pub fn mark_completed(&mut self, encoding_key_hex: String) {
        self.completed_keys.insert(encoding_key_hex);
    }

    /// Check if a key is already completed.
    #[must_use]
    pub fn is_completed(&self, encoding_key_hex: &str) -> bool {
        self.completed_keys.contains(encoding_key_hex)
    }

    /// Resolve the checkpoint file path for a given install directory.
    #[must_use]
    pub fn file_path(install_path: &Path) -> PathBuf {
        install_path.join("Data").join(CHECKPOINT_FILENAME)
    }

    /// Read a checkpoint from disk. Returns `None` if no checkpoint file exists.
    ///
    /// Returns an error only if the file exists but cannot be parsed.
    pub async fn read(install_path: &Path) -> InstallationResult<Option<Self>> {
        let path = Self::file_path(install_path);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let checkpoint: Self = serde_json::from_str(&content).map_err(|e| {
                    InstallationError::Checkpoint(format!(
                        "failed to parse checkpoint at {}: {e}",
                        path.display()
                    ))
                })?;
                Ok(Some(checkpoint))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(InstallationError::Checkpoint(format!(
                "failed to read checkpoint at {}: {e}",
                path.display()
            ))),
        }
    }

    /// Write the checkpoint to disk.
    pub async fn write(&self, install_path: &Path) -> InstallationResult<()> {
        let path = Self::file_path(install_path);

        // Ensure the Data/ directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(self)?;

        // Write atomically via temp file
        let tmp_path = path.with_extension("json.tmp");
        tokio::fs::write(&tmp_path, content.as_bytes()).await?;
        tokio::fs::rename(&tmp_path, &path).await?;

        Ok(())
    }

    /// Remove the checkpoint file.
    pub async fn clear(install_path: &Path) -> InstallationResult<()> {
        let path = Self::file_path(install_path);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(InstallationError::Checkpoint(format!(
                "failed to remove checkpoint at {}: {e}",
                path.display()
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checkpoint_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let install_path = tmp.path();

        // Create Data/ directory
        std::fs::create_dir_all(install_path.join("Data")).expect("dir creation");

        let mut checkpoint = Checkpoint::new(
            "wow_classic_era".to_string(),
            "abc123".to_string(),
            "def456".to_string(),
            100,
        );
        checkpoint.mark_completed("key1".to_string());
        checkpoint.mark_completed("key2".to_string());

        // Write
        checkpoint
            .write(install_path)
            .await
            .expect("test: write should succeed");

        // Read back
        let loaded = Checkpoint::read(install_path)
            .await
            .expect("test: read should succeed")
            .expect("test: checkpoint should exist");

        assert_eq!(loaded.product, "wow_classic_era");
        assert_eq!(loaded.build_config, "abc123");
        assert_eq!(loaded.cdn_config, "def456");
        assert_eq!(loaded.total_artifacts, 100);
        assert_eq!(loaded.completed_count(), 2);
        assert!(loaded.is_completed("key1"));
        assert!(loaded.is_completed("key2"));
        assert!(!loaded.is_completed("key3"));
        assert_eq!(loaded.remaining_count(), 98);
    }

    #[tokio::test]
    async fn checkpoint_read_nonexistent() {
        let tmp = tempfile::tempdir().expect("test: tempdir creation should succeed");
        let result = Checkpoint::read(tmp.path())
            .await
            .expect("test: read should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn checkpoint_clear() {
        let tmp = tempfile::tempdir().expect("test: tempdir creation should succeed");
        let install_path = tmp.path();

        let checkpoint =
            Checkpoint::new("wow".to_string(), "abc".to_string(), "def".to_string(), 10);
        checkpoint
            .write(install_path)
            .await
            .expect("test: write should succeed");

        // Verify it exists
        assert!(
            Checkpoint::read(install_path)
                .await
                .expect("test: read should succeed")
                .is_some()
        );

        // Clear
        Checkpoint::clear(install_path)
            .await
            .expect("test: clear should succeed");

        // Verify it is gone
        assert!(
            Checkpoint::read(install_path)
                .await
                .expect("test: read should succeed")
                .is_none()
        );

        // Clearing again should not error
        Checkpoint::clear(install_path)
            .await
            .expect("test: clear of nonexistent should succeed");
    }
}
