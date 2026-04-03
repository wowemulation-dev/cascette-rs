//! Game file extraction from CASC storage to product directories.
//!
//! Reads files from the local CASC installation and writes them to
//! product subdirectories (e.g., `_classic_era_/`). Path normalization
//! handles mixed-case directory names from the install manifest.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use cascette_client_storage::Installation;
use cascette_formats::install::InstallManifest;

use crate::config::ExtractConfig;
use crate::error::{InstallationError, InstallationResult};
use crate::progress::ProgressEvent;

/// Report from extraction.
#[derive(Debug)]
pub struct ExtractReport {
    /// Files extracted.
    pub extracted: usize,
    /// Files that failed.
    pub failed: usize,
    /// Files skipped (filtered out by pattern).
    pub skipped: usize,
    /// Paths that failed.
    pub failed_paths: Vec<String>,
}

/// Extract pipeline.
///
/// Reads files from local CASC storage and writes them to the output
/// directory with path normalization applied.
pub struct ExtractPipeline {
    config: ExtractConfig,
}

impl ExtractPipeline {
    /// Create a new extract pipeline.
    #[must_use]
    pub fn new(config: ExtractConfig) -> Self {
        Self { config }
    }

    /// Run the extraction.
    ///
    /// The install manifest must be resolved by the caller (typically via
    /// `resolve_manifests()`) and passed in. This avoids re-fetching the
    /// manifest from CDN and removes the dependency on `.build.info` parsing.
    pub async fn run(
        self,
        manifest: InstallManifest,
        progress: impl Fn(ProgressEvent) + Send + Sync,
    ) -> InstallationResult<ExtractReport> {
        let installation = Installation::open(self.config.install_path.join("Data"))?;
        installation.initialize().await?;

        let tags: Vec<&str> = self
            .config
            .platform_tags
            .iter()
            .map(String::as_str)
            .collect();

        let files = if tags.is_empty() {
            manifest.entries.iter().enumerate().collect::<Vec<_>>()
        } else {
            manifest.get_files_for_tags(&tags)
        };

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let installation = Arc::new(installation);
        let mut extracted: usize = 0;
        let mut failed: usize = 0;
        let mut skipped: usize = 0;
        let mut failed_paths: Vec<String> = Vec::new();

        for (_idx, entry) in &files {
            // Apply pattern filter
            if let Some(ref pattern) = self.config.pattern
                && !entry.matches_pattern(pattern)
            {
                skipped += 1;
                continue;
            }

            let normalized = normalize_install_path(&entry.path);
            let output_file = self.config.output_path.join(&normalized);

            progress(ProgressEvent::ExtractStarted {
                path: entry.path.clone(),
            });

            // Validate path doesn't escape output directory
            if let Err(e) = validate_output_path(&self.config.output_path, &output_file) {
                warn!(path = %entry.path, error = %e, "path traversal detected");
                failed += 1;
                failed_paths.push(entry.path.clone());
                continue;
            }

            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| InstallationError::Cdn(format!("semaphore closed: {e}")))?;

            // Read from CASC and write to disk
            match installation
                .read_file_by_content_key(&entry.content_key)
                .await
            {
                Ok(data) => {
                    if let Some(parent) = output_file.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&output_file, &data).await?;
                    extracted += 1;
                    debug!(path = %normalized, "extracted");
                    progress(ProgressEvent::ExtractComplete {
                        path: entry.path.clone(),
                    });
                }
                Err(e) => {
                    warn!(path = %entry.path, error = %e, "failed to extract");
                    failed += 1;
                    failed_paths.push(entry.path.clone());
                }
            }
        }

        info!(
            extracted = extracted,
            failed = failed,
            skipped = skipped,
            "extraction complete"
        );

        Ok(ExtractReport {
            extracted,
            failed,
            skipped,
            failed_paths,
        })
    }
}

/// Normalize a file path from the install manifest for case-sensitive filesystems.
///
/// On Linux/macOS, directory names are uppercased to handle mixed-case
/// directory references in Blizzard's install manifest (e.g., both `Utils/`
/// and `UTILS/` appear). Filename case is preserved.
///
/// On Windows (case-insensitive), only backslash-to-forward-slash conversion
/// is applied.
#[must_use]
pub fn normalize_install_path(file_path: &str) -> String {
    let path = file_path.replace('\\', "/");

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let components: Vec<&str> = path.split('/').collect();
        if components.len() > 1 {
            let mut normalized = Vec::with_capacity(components.len());
            for (i, component) in components.iter().enumerate() {
                if i < components.len() - 1 {
                    // Directory component: uppercase for case-insensitive matching
                    normalized.push(component.to_uppercase());
                } else {
                    // Filename: preserve original case
                    normalized.push((*component).to_string());
                }
            }
            normalized.join("/")
        } else {
            path
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        path
    }
}

/// Validate that an output path does not escape the output directory.
pub(crate) fn validate_output_path(base: &Path, target: &Path) -> InstallationResult<()> {
    // Canonicalize the base. Target may not exist yet, so we check
    // that it starts with the base path.
    let canonical_base = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());

    // Build the full target path and check it's under base
    let mut accumulated = canonical_base.clone();
    for component in target
        .strip_prefix(&canonical_base)
        .unwrap_or(target)
        .components()
    {
        match component {
            std::path::Component::Normal(c) => accumulated.push(c),
            std::path::Component::ParentDir => {
                return Err(InstallationError::PathTraversal(format!(
                    "path contains '..' component: {}",
                    target.display()
                )));
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn normalize_backslashes() {
        assert_eq!(normalize_install_path("Interface\\Icons\\file.blp"), {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                "INTERFACE/ICONS/file.blp"
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                "Interface/Icons/file.blp"
            }
        });
    }

    #[test]
    fn normalize_single_component() {
        assert_eq!(normalize_install_path("readme.txt"), "readme.txt");
    }

    #[test]
    fn normalize_preserves_filename_case() {
        let result = normalize_install_path("Utils/ScanDLLs.lua");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert_eq!(result, "UTILS/ScanDLLs.lua");
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        assert_eq!(result, "Utils/ScanDLLs.lua");
    }

    #[test]
    fn path_traversal_detected() {
        let base = PathBuf::from("/tmp/test_base");
        let target = PathBuf::from("/tmp/test_base/../../../etc/passwd");
        assert!(validate_output_path(&base, &target).is_err());
    }
}
