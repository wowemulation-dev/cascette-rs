//! Loose file repair — verifies and re-places files stored outside CASC archives.
//!
//! Operates on the full install manifest loose file set: opens each file, reads
//! the E-header, resolves CKey→EKey via the encoding table, checks size and hash,
//! and re-downloads from CDN if the file is missing or corrupt.
//!
//! Two passes:
//!
//! ## Pass 1: Install manifest loose files
//!
//! For each entry in `manifests.install.entries`:
//!
//! 1. Resolve the content key → encoding key via `manifests.encoding.find_encoding`.
//! 2. Compute the loose file path: `{install_path}/{game_subfolder}/{entry.path}`.
//! 3. If the file is missing or the on-disk size does not match `entry.file_size`:
//!    - Re-extract the file from local CASC storage using the resolved encoding key.
//!    - If CASC extraction fails (key not present locally), attempt CDN re-download
//!      via `ContentType::Data` and write to CASC, then re-extract.
//!
//! Pass 1 is skipped when `game_subfolder` is `None` (no loose file placement was
//! configured for this installation).
//!
//! ## Pass 2: CDN config files
//!
//! Verifies config files referenced by the current build (build config encoding
//! key and root file key) exist in `Data/config/` with correct MD5 hashes.
//! Re-downloads from CDN via `ContentType::Config` if missing or corrupt.

use std::path::PathBuf;

use cascette_client_storage::Installation;
use cascette_installation::{BuildManifests, CdnSource};
use cascette_protocol::{CdnEndpoint, ContentType};
use tracing::{info, warn};

use crate::ExecutionMode;
use crate::error::MaintenanceResult;
use crate::report::RepairReport;

/// Loose file repair engine.
pub struct LooseFileRepairEngine<'a, S: CdnSource> {
    installation: &'a Installation,
    cdn: &'a S,
    endpoints: &'a [CdnEndpoint],
    manifests: &'a BuildManifests,
    /// Product subfolder containing placed loose files (e.g. `"_classic_era_"`).
    /// When `None`, install manifest loose file repair is skipped.
    game_subfolder: Option<String>,
}

impl<'a, S: CdnSource> LooseFileRepairEngine<'a, S> {
    pub fn new(
        installation: &'a Installation,
        cdn: &'a S,
        endpoints: &'a [CdnEndpoint],
        manifests: &'a BuildManifests,
        game_subfolder: Option<String>,
    ) -> Self {
        Self {
            installation,
            cdn,
            endpoints,
            manifests,
            game_subfolder,
        }
    }

    /// Run loose file repair.
    ///
    /// Pass 1 repairs placed loose files from the install manifest.
    /// Pass 2 verifies and repairs CDN config files.
    pub async fn run(
        &self,
        mode: ExecutionMode,
        report: &mut RepairReport,
    ) -> MaintenanceResult<()> {
        // Pass 1: install manifest loose files
        if let Some(ref subfolder) = self.game_subfolder {
            self.repair_install_manifest_files(subfolder, mode, report)
                .await?;
        } else {
            info!("No game subfolder configured; skipping install manifest loose file repair");
        }

        // Pass 2: CDN config files
        self.repair_config_files(mode, report).await?;

        info!(
            "Loose files: {} checked, {} repaired",
            report.loose_files_checked, report.loose_files_repaired
        );

        Ok(())
    }

    /// Repair loose files placed from the install manifest.
    ///
    /// Iterates all install manifest entries, resolves each content key to an
    /// encoding key, checks the placed file at `{install_path}/{subfolder}/{path}`,
    /// and re-extracts from CASC storage if the file is missing or wrong size.
    async fn repair_install_manifest_files(
        &self,
        subfolder: &str,
        mode: ExecutionMode,
        report: &mut RepairReport,
    ) -> MaintenanceResult<()> {
        // installation.path() = {install_root}/Data/
        // Loose files live at {install_root}/{subfolder}/
        let install_root = self
            .installation
            .path()
            .parent()
            .unwrap_or(self.installation.path());
        let base = install_root.join(subfolder);
        let entry_count = self.manifests.install.entries.len();

        info!(
            "Checking {} install manifest entries under {}",
            entry_count,
            base.display()
        );

        for entry in &self.manifests.install.entries {
            report.loose_files_checked += 1;

            // Resolve content key → encoding key via encoding table (CKey→EKey).
            let Some(ekey) = self.manifests.encoding.find_encoding(&entry.content_key) else {
                // No encoding entry means this file is not in the local archives.
                // Skip — DataRepairEngine handles missing archive entries.
                continue;
            };

            let loose_path = base.join(&entry.path);
            let expected_size = u64::from(entry.file_size);

            let needs_repair = if loose_path.exists() {
                match tokio::fs::metadata(&loose_path).await {
                    Ok(meta) if meta.len() != expected_size => {
                        warn!(
                            "Loose file size mismatch: {} (expected {expected_size}, got {})",
                            loose_path.display(),
                            meta.len()
                        );
                        true
                    }
                    Ok(_) => false,
                    Err(e) => {
                        warn!("Cannot stat loose file {}: {e}", loose_path.display());
                        true
                    }
                }
            } else {
                warn!("Missing loose file: {}", loose_path.display());
                true
            };

            if !needs_repair || mode.is_dry_run() {
                continue;
            }

            // Re-extract from local CASC storage using the resolved encoding key.
            match self.installation.read_file_by_encoding_key(&ekey).await {
                Ok(data) => {
                    if let Some(parent) = loose_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&loose_path, &data).await?;
                    report.loose_files_repaired += 1;
                    info!("Re-extracted loose file: {}", loose_path.display());
                }
                Err(e) => {
                    // CASC extraction failed — the archive entry itself may be
                    // corrupted or absent. Attempt CDN re-download of the raw BLTE
                    // blob, write it to CASC storage, then retry extraction.
                    warn!(
                        "CASC extraction failed for {} ({e}), attempting CDN re-download",
                        loose_path.display()
                    );
                    match self.redownload_data(ekey.as_bytes()).await {
                        Ok(true) => {
                            // Blob is now in CASC; retry extraction.
                            match self.installation.read_file_by_encoding_key(&ekey).await {
                                Ok(data) => {
                                    if let Some(parent) = loose_path.parent() {
                                        tokio::fs::create_dir_all(parent).await?;
                                    }
                                    tokio::fs::write(&loose_path, &data).await?;
                                    report.loose_files_repaired += 1;
                                    info!(
                                        "Re-extracted loose file after CDN repair: {}",
                                        loose_path.display()
                                    );
                                }
                                Err(e2) => {
                                    warn!(
                                        "Extraction still failed after CDN repair for {}: {e2}",
                                        loose_path.display()
                                    );
                                }
                            }
                        }
                        Ok(false) => {
                            warn!("No CDN source found for {}", loose_path.display());
                        }
                        Err(e2) => {
                            warn!("CDN re-download failed for {}: {e2}", loose_path.display());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Repair CDN config files referenced by the current build.
    ///
    /// Checks existence and MD5 hash of config files in `Data/config/`.
    /// Re-downloads from CDN via `ContentType::Config` if missing or corrupt.
    async fn repair_config_files(
        &self,
        mode: ExecutionMode,
        report: &mut RepairReport,
    ) -> MaintenanceResult<()> {
        let config_files = self.collect_expected_config_files();

        info!("Checking {} config files", config_files.len());

        for (path, expected_hash) in &config_files {
            report.loose_files_checked += 1;

            if !path.exists() {
                warn!("Missing config file: {}", path.display());
                if !mode.is_dry_run() {
                    match self.redownload_config(expected_hash).await {
                        Ok(data) => {
                            if let Some(parent) = path.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }
                            tokio::fs::write(path, &data).await?;
                            report.loose_files_repaired += 1;
                            info!("Repaired config file: {}", path.display());
                        }
                        Err(e) => {
                            warn!("Failed to re-download config file {}: {e}", path.display());
                        }
                    }
                }
                continue;
            }

            let content = tokio::fs::read(path).await?;
            let actual_hash = format!("{:x}", md5::compute(&content));

            if actual_hash != *expected_hash {
                warn!(
                    "Config file hash mismatch: {} (expected {expected_hash}, got {actual_hash})",
                    path.display()
                );
                if !mode.is_dry_run() {
                    match self.redownload_config(expected_hash).await {
                        Ok(data) => {
                            tokio::fs::write(path, &data).await?;
                            report.loose_files_repaired += 1;
                            info!("Repaired config file: {}", path.display());
                        }
                        Err(e) => {
                            warn!("Failed to re-download config file {}: {e}", path.display());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Collect expected CDN config files with their MD5 hashes.
    ///
    /// `installation.path()` returns the `Data/` directory, so config files
    /// are at `Data/config/XX/YY/hash`.
    fn collect_expected_config_files(&self) -> Vec<(PathBuf, String)> {
        let base = self.installation.path().clone();
        let mut files = Vec::new();

        // Build config encoding info content key
        if let Some(encoding_info) = self.manifests.build_config.encoding() {
            let key = &encoding_info.content_key;
            if key.len() >= 4 {
                let path = base
                    .join("config")
                    .join(&key[..2])
                    .join(&key[2..4])
                    .join(key);
                files.push((path, key.to_lowercase()));
            }
        }

        // Root file
        if let Some(root_key) = self.manifests.build_config.root()
            && root_key.len() >= 4
        {
            let path = base
                .join("config")
                .join(&root_key[..2])
                .join(&root_key[2..4])
                .join(root_key);
            files.push((path, root_key.to_lowercase()));
        }

        files
    }

    /// Download a data blob from CDN by encoding key and write to CASC storage.
    ///
    /// Returns `Ok(true)` on success, `Ok(false)` if all endpoints failed.
    async fn redownload_data(&self, ekey_bytes: &[u8]) -> MaintenanceResult<bool> {
        for endpoint in self.endpoints {
            match self
                .cdn
                .download(endpoint, ContentType::Data, ekey_bytes)
                .await
            {
                Ok(data) => {
                    self.installation.write_raw_blte(data).await?;
                    return Ok(true);
                }
                Err(e) => {
                    warn!("CDN data download failed from {}: {e}", endpoint.host);
                }
            }
        }
        Ok(false)
    }

    /// Download a config file from CDN by its content hash.
    async fn redownload_config(&self, hash: &str) -> MaintenanceResult<Vec<u8>> {
        let key_bytes = hex::decode(hash).map_err(|e| {
            crate::error::MaintenanceError::Repair(format!("invalid config hash: {e}"))
        })?;

        for endpoint in self.endpoints {
            match self
                .cdn
                .download(endpoint, ContentType::Config, &key_bytes)
                .await
            {
                Ok(data) => return Ok(data),
                Err(e) => {
                    warn!("CDN config download failed from {}: {e}", endpoint.host);
                }
            }
        }

        Err(crate::error::MaintenanceError::Repair(format!(
            "all CDN endpoints failed for config {hash}"
        )))
    }
}
