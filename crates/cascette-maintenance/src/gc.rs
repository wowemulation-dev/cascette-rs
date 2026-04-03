//! Garbage collection — removes content not in the preservation set.
//!
//! Three stages:
//! 1. **Core GC** — removes unreferenced index entries, orphaned segments, empty dirs
//! 2. **Config cleanup** — removes config files not referenced by the current build
//! 3. **CDN index cleanup** — removes `.index` files for archives not in CdnConfig
//!
//! Stages 2-3 require `BuildManifests` and are skipped when manifests are unavailable.

use std::collections::HashSet;
use std::time::Instant;

use cascette_client_storage::Installation;
use cascette_installation::BuildManifests;
use tracing::info;

use crate::ExecutionMode;
use crate::error::MaintenanceResult;
use crate::preservation::PreservationSet;
use crate::report::GcReport;

/// Configuration for garbage collection.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct GcConfig {
    /// Remove orphaned data files (segments with no index references).
    pub remove_orphaned_segments: bool,
    /// Clean empty directories under Data/.
    pub clean_empty_dirs: bool,
    /// Remove config files not referenced by the current build.
    /// Requires manifests.
    pub remove_unused_configs: bool,
    /// Remove CDN index files not corresponding to current archives.
    /// Requires manifests.
    pub remove_cdn_indices: bool,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            remove_orphaned_segments: true,
            clean_empty_dirs: true,
            remove_unused_configs: true,
            remove_cdn_indices: true,
        }
    }
}

/// Garbage collector for CASC installations.
pub struct GarbageCollector<'a> {
    installation: &'a Installation,
    preservation: &'a PreservationSet,
    manifests: Option<&'a BuildManifests>,
    config: GcConfig,
}

impl<'a> GarbageCollector<'a> {
    /// Create a new garbage collector without manifest awareness.
    pub const fn new(
        installation: &'a Installation,
        preservation: &'a PreservationSet,
        config: GcConfig,
    ) -> Self {
        Self {
            installation,
            preservation,
            manifests: None,
            config,
        }
    }

    /// Create a garbage collector with manifest awareness.
    ///
    /// When manifests are provided, stages 2-3 (config cleanup, CDN index
    /// cleanup) are enabled.
    pub const fn with_manifests(
        installation: &'a Installation,
        preservation: &'a PreservationSet,
        manifests: &'a BuildManifests,
        config: GcConfig,
    ) -> Self {
        Self {
            installation,
            preservation,
            manifests: Some(manifests),
            config,
        }
    }

    /// Run garbage collection.
    pub async fn run(&self, mode: ExecutionMode) -> MaintenanceResult<GcReport> {
        let start = Instant::now();
        let mut report = GcReport {
            executed: !mode.is_dry_run(),
            ..GcReport::default()
        };

        info!(
            "Starting garbage collection ({})",
            if mode.is_dry_run() {
                "dry-run"
            } else {
                "execute"
            }
        );

        // Stage 1: Core GC — unreferenced entries, orphaned segments, empty dirs
        self.stage_core_gc(mode, &mut report).await?;

        // Stage 2: Config file cleanup (requires manifests)
        if self.config.remove_unused_configs
            && let Some(manifests) = self.manifests
        {
            self.stage_config_cleanup(mode, manifests, &mut report)
                .await?;
        }

        // Stage 3: CDN index cleanup (requires manifests)
        if self.config.remove_cdn_indices
            && let Some(manifests) = self.manifests
        {
            self.stage_cdn_index_cleanup(mode, manifests, &mut report)
                .await?;
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    /// Stage 1: Remove unreferenced entries, orphaned segments, empty dirs.
    async fn stage_core_gc(
        &self,
        mode: ExecutionMode,
        report: &mut GcReport,
    ) -> MaintenanceResult<()> {
        let entries = self.installation.get_all_index_entries().await;
        report.entries_scanned = entries.len();

        let mut unreferenced_keys = Vec::new();
        let mut unreferenced_bytes: u64 = 0;

        for entry in &entries {
            if !self.preservation.contains(&entry.key) {
                unreferenced_keys.push(entry.key);
                unreferenced_bytes += u64::from(entry.size);
            }
        }

        report.entries_removed = unreferenced_keys.len();
        report.bytes_freed = unreferenced_bytes;

        info!(
            "Found {} unreferenced entries ({} bytes)",
            unreferenced_keys.len(),
            unreferenced_bytes
        );

        if mode.is_dry_run() {
            if self.config.remove_orphaned_segments {
                report.segments_orphaned = self.count_orphaned_segments(&entries).await;
            }
        } else {
            if !unreferenced_keys.is_empty() {
                let removed = self
                    .installation
                    .remove_index_entries(&unreferenced_keys)
                    .await?;
                info!("Removed {removed} index entries");
            }

            if self.config.remove_orphaned_segments {
                let (orphaned, deleted) = self.remove_orphaned_segments(&entries).await?;
                report.segments_orphaned = orphaned;
                report.segments_deleted = deleted;
            }

            if self.config.clean_empty_dirs {
                report.empty_dirs_cleaned = self.clean_empty_dirs().await?;
            }
        }

        Ok(())
    }

    /// Stage 2: Remove config files not referenced by the current build.
    ///
    /// Scans `Data/config/` (two-level hex directory structure) and removes
    /// config files whose hash does not match any active config hash from
    /// the build config or CDN config.
    async fn stage_config_cleanup(
        &self,
        mode: ExecutionMode,
        manifests: &BuildManifests,
        report: &mut GcReport,
    ) -> MaintenanceResult<()> {
        let config_dir = self.installation.path().join("config");

        if !config_dir.is_dir() {
            return Ok(());
        }

        let active_hashes = collect_active_config_hashes(manifests);

        let mut removed = 0;

        // Walk config directory (two-level: config/XX/XXXXXXXXXXX...)
        let mut top_dir = tokio::fs::read_dir(&config_dir).await?;
        while let Some(prefix_entry) = top_dir.next_entry().await? {
            let prefix_path = prefix_entry.path();
            if !prefix_path.is_dir() {
                continue;
            }

            let mut sub_dir = tokio::fs::read_dir(&prefix_path).await?;
            while let Some(file_entry) = sub_dir.next_entry().await? {
                let file_path = file_entry.path();
                if !file_path.is_file() {
                    continue;
                }

                // Full hash = prefix_dir_name + file_name
                let prefix_name = prefix_entry.file_name();
                let file_name = file_entry.file_name();
                let full_hash = format!(
                    "{}{}",
                    prefix_name.to_string_lossy().to_lowercase(),
                    file_name.to_string_lossy().to_lowercase(),
                );

                if !active_hashes.contains(&full_hash) {
                    if !mode.is_dry_run() {
                        tokio::fs::remove_file(&file_path).await?;
                    }
                    removed += 1;
                    info!("Config cleanup: removing {}", file_path.display());
                }
            }
        }

        report.config_files_removed = removed;
        info!("Config cleanup: {removed} files would be removed");

        Ok(())
    }

    /// Stage 3: Remove CDN index files not corresponding to current archives.
    ///
    /// Scans `Data/indices/` and removes `.index` files whose base name
    /// does not match an archive listed in `CdnConfig.archives()`.
    async fn stage_cdn_index_cleanup(
        &self,
        mode: ExecutionMode,
        manifests: &BuildManifests,
        report: &mut GcReport,
    ) -> MaintenanceResult<()> {
        let indices_dir = self.installation.path().join("indices");

        if !indices_dir.is_dir() {
            return Ok(());
        }

        let active_archives: HashSet<String> = manifests
            .cdn_config
            .archives()
            .iter()
            .map(|a| a.content_key.to_lowercase())
            .collect();

        let mut removed = 0;

        let mut dir = tokio::fs::read_dir(&indices_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // CDN index files are named <hash>.index
            let base = if let Some(stripped) = name.strip_suffix(".index") {
                stripped.to_lowercase()
            } else {
                continue;
            };

            if !active_archives.contains(&base) {
                if !mode.is_dry_run() {
                    tokio::fs::remove_file(entry.path()).await?;
                }
                removed += 1;
                info!("CDN index cleanup: removing {}", entry.path().display());
            }
        }

        report.cdn_indices_removed = removed;
        info!("CDN index cleanup: {removed} files would be removed");

        Ok(())
    }

    async fn count_orphaned_segments(
        &self,
        entries: &[cascette_client_storage::IndexEntry],
    ) -> usize {
        let stats = self.installation.stats().await;
        let total_segments = stats.archive_files;

        let mut referenced_segments = HashSet::new();
        for entry in entries {
            if self.preservation.contains(&entry.key) {
                referenced_segments.insert(entry.archive_location.archive_id);
            }
        }

        total_segments.saturating_sub(referenced_segments.len())
    }

    async fn remove_orphaned_segments(
        &self,
        entries: &[cascette_client_storage::IndexEntry],
    ) -> MaintenanceResult<(usize, usize)> {
        let stats = self.installation.stats().await;
        let data_path = self.installation.path().join("data");

        let mut referenced_segments = HashSet::new();
        for entry in entries {
            if self.preservation.contains(&entry.key) {
                referenced_segments.insert(entry.archive_location.archive_id);
            }
        }

        let mut orphaned = 0;
        let mut deleted = 0;

        for segment_id in 0..stats.archive_files {
            #[allow(clippy::cast_possible_truncation)]
            if !referenced_segments.contains(&(segment_id as u16)) {
                orphaned += 1;
                let segment_path = data_path.join(format!("data.{segment_id:03}"));
                if segment_path.exists() {
                    tokio::fs::remove_file(&segment_path).await?;
                    deleted += 1;
                    info!("Deleted orphaned segment: {}", segment_path.display());
                }
            }
        }

        Ok((orphaned, deleted))
    }

    async fn clean_empty_dirs(&self) -> MaintenanceResult<usize> {
        let data_path = self.installation.path().join("data");
        let mut cleaned = 0;

        if data_path.is_dir() {
            let mut dir = tokio::fs::read_dir(&data_path).await?;
            while let Some(entry) = dir.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    let mut sub_dir = tokio::fs::read_dir(&path).await?;
                    if sub_dir.next_entry().await?.is_none() {
                        tokio::fs::remove_dir(&path).await?;
                        cleaned += 1;
                    }
                }
            }
        }

        Ok(cleaned)
    }
}

/// Collect all config hashes that are actively referenced by the current build.
fn collect_active_config_hashes(manifests: &BuildManifests) -> HashSet<String> {
    let mut active = HashSet::new();

    // Encoding manifest hashes
    if let Some(encoding_info) = manifests.build_config.encoding() {
        active.insert(encoding_info.content_key.to_lowercase());
        if let Some(ref ekey) = encoding_info.encoding_key {
            active.insert(ekey.to_lowercase());
        }
    }

    // Root manifest hash (root() returns Option<&str> — just the content key)
    if let Some(root_key) = manifests.build_config.root() {
        active.insert(root_key.to_lowercase());
    }

    // Install manifest hashes
    for info in manifests.build_config.install() {
        active.insert(info.content_key.to_lowercase());
        if let Some(ref ekey) = info.encoding_key {
            active.insert(ekey.to_lowercase());
        }
    }

    // Download manifest hashes
    for info in manifests.build_config.download() {
        active.insert(info.content_key.to_lowercase());
        if let Some(ref ekey) = info.encoding_key {
            active.insert(ekey.to_lowercase());
        }
    }

    // CDN config archive keys
    for archive in manifests.cdn_config.archives() {
        active.insert(archive.content_key.to_lowercase());
    }

    active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc_config_defaults() {
        let config = GcConfig::default();
        assert!(config.remove_orphaned_segments);
        assert!(config.clean_empty_dirs);
        assert!(config.remove_unused_configs);
        assert!(config.remove_cdn_indices);
    }

    #[test]
    fn gc_config_stages_disabled() {
        let config = GcConfig {
            remove_unused_configs: false,
            remove_cdn_indices: false,
            ..Default::default()
        };
        assert!(!config.remove_unused_configs);
        assert!(!config.remove_cdn_indices);
        // Core stages still on
        assert!(config.remove_orphaned_segments);
        assert!(config.clean_empty_dirs);
    }
}
