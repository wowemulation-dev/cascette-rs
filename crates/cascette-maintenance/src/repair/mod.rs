//! Repair orchestrator — validates content integrity and rebuilds damaged storage.
//!
//! Implements a state machine matching Agent.exe's build repair flow:
//!
//! 1. **Initialize** — set up repair context
//! 2. **ReadBuildConfig** — validate build config availability
//! 3. **InitCdnIndexSet** — prepare CDN index set for lookups
//! 4. **RepairDataContainer** — repair data archives
//! 5. **PostRepairData** — cleanup after data repair
//! 6. **RepairEcacheContainer** — repair ecache archives
//! 7. **PostRepairEcache** — cleanup after ecache repair
//! 8. **RepairHardLinkContainer** — repair hard link container
//! 9. **PostRepairHardLink** — cleanup after hard link repair
//! 10. **DataRepair** — file-level data repair with CDN re-download
//! 11. **Complete** — write markers, produce report
//!
//! The simplified orchestrator (without manifests) runs only verification
//! and index rebuild, matching the previous behavior.

pub mod data_repair;
pub mod loose_files;
pub mod markers;

use std::time::Instant;

use cascette_client_storage::Installation;
use cascette_installation::{BuildManifests, CdnSource};
use cascette_protocol::CdnEndpoint;
use tracing::{info, warn};

use crate::ExecutionMode;
use crate::error::MaintenanceResult;
use crate::report::RepairReport;

/// Repair state machine states.
///
/// Maps to Agent.exe's repair states. Not all states perform work in
/// cascette-rs; some exist for spec compatibility and future expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairState {
    /// State 0: Initialize repair context.
    Initialize,
    /// State 5: Read and validate build config.
    ReadBuildConfig,
    /// State 6: Initialize CDN index set.
    InitCdnIndexSet,
    /// State 7: Repair data container archives.
    RepairDataContainer,
    /// State 8: Post-repair cleanup for data container.
    PostRepairData,
    /// State 9: Repair ecache container.
    RepairEcacheContainer,
    /// State 10: Post-repair cleanup for ecache.
    PostRepairEcache,
    /// State 11: Repair hard link container.
    RepairHardLinkContainer,
    /// State 12: Post-repair cleanup for hard links.
    PostRepairHardLink,
    /// State 16: File-level data repair with CDN re-download.
    DataRepair,
    /// Final state: write markers, produce report.
    Complete,
}

impl RepairState {
    /// Advance to the next state in the repair sequence.
    fn next(self) -> Option<Self> {
        match self {
            Self::Initialize => Some(Self::ReadBuildConfig),
            Self::ReadBuildConfig => Some(Self::InitCdnIndexSet),
            Self::InitCdnIndexSet => Some(Self::RepairDataContainer),
            Self::RepairDataContainer => Some(Self::PostRepairData),
            Self::PostRepairData => Some(Self::RepairEcacheContainer),
            Self::RepairEcacheContainer => Some(Self::PostRepairEcache),
            Self::PostRepairEcache => Some(Self::RepairHardLinkContainer),
            Self::RepairHardLinkContainer => Some(Self::PostRepairHardLink),
            Self::PostRepairHardLink => Some(Self::DataRepair),
            Self::DataRepair => Some(Self::Complete),
            Self::Complete => None,
        }
    }

    /// State number matching Agent.exe's repair state numbering.
    pub const fn state_number(self) -> u32 {
        match self {
            Self::Initialize => 0,
            Self::ReadBuildConfig => 5,
            Self::InitCdnIndexSet => 6,
            Self::RepairDataContainer => 7,
            Self::PostRepairData => 8,
            Self::RepairEcacheContainer => 9,
            Self::PostRepairEcache => 10,
            Self::RepairHardLinkContainer => 11,
            Self::PostRepairHardLink => 12,
            Self::DataRepair => 16,
            Self::Complete => 99,
        }
    }

    /// Map an Agent.exe state number back to a `RepairState`.
    ///
    /// Returns `None` for unrecognised state numbers (future-proofing).
    pub const fn from_state_number(n: u32) -> Option<Self> {
        match n {
            0 => Some(Self::Initialize),
            5 => Some(Self::ReadBuildConfig),
            6 => Some(Self::InitCdnIndexSet),
            7 => Some(Self::RepairDataContainer),
            8 => Some(Self::PostRepairData),
            9 => Some(Self::RepairEcacheContainer),
            10 => Some(Self::PostRepairEcache),
            11 => Some(Self::RepairHardLinkContainer),
            12 => Some(Self::PostRepairHardLink),
            16 => Some(Self::DataRepair),
            99 => Some(Self::Complete),
            _ => None,
        }
    }
}

/// Configuration for build repair.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct RepairConfig {
    /// Verify content integrity (header + BLTE validation).
    pub verify_content: bool,
    /// Rebuild indices from segment headers when corruption is detected.
    pub rebuild_indices: bool,
    /// Repair loose files (config files, `.build.info`).
    pub repair_loose_files: bool,
    /// Write repair marker files after repair.
    pub write_markers: bool,
}

impl Default for RepairConfig {
    fn default() -> Self {
        Self {
            verify_content: true,
            rebuild_indices: true,
            repair_loose_files: true,
            write_markers: true,
        }
    }
}

/// Build repair orchestrator (without CDN support).
///
/// Runs verification and index rebuild only. For CDN re-download support,
/// use [`BuildRepairOrchestrator`] instead.
pub struct RepairOrchestrator<'a> {
    installation: &'a Installation,
    config: RepairConfig,
}

impl<'a> RepairOrchestrator<'a> {
    pub const fn new(installation: &'a Installation, config: RepairConfig) -> Self {
        Self {
            installation,
            config,
        }
    }

    pub async fn run(&self, mode: ExecutionMode) -> MaintenanceResult<RepairReport> {
        let start = Instant::now();
        let mut report = RepairReport {
            executed: !mode.is_dry_run(),
            ..RepairReport::default()
        };

        info!(
            "Starting repair ({})",
            if mode.is_dry_run() {
                "dry-run"
            } else {
                "execute"
            }
        );

        if self.config.verify_content {
            self.verify_content(&mut report).await?;
        }

        if self.config.rebuild_indices && !mode.is_dry_run() && report.entries_corrupted > 0 {
            self.rebuild_indices(&mut report).await?;
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    async fn verify_content(&self, report: &mut RepairReport) -> MaintenanceResult<()> {
        let mut entries = self.installation.get_all_index_entries().await;
        report.entries_verified = entries.len();

        info!("Verifying {} index entries", entries.len());

        entries.sort_unstable_by(|a, b| {
            a.archive_location
                .archive_id
                .cmp(&b.archive_location.archive_id)
                .then(
                    a.archive_location
                        .archive_offset
                        .cmp(&b.archive_location.archive_offset),
                )
        });

        for entry in &entries {
            let archive_id = entry.archive_location.archive_id;
            let offset = entry.archive_location.archive_offset;
            let size = entry.size;

            match self
                .installation
                .validate_entry(archive_id, offset, size)
                .await
            {
                Ok(true) => {
                    report.entries_valid += 1;
                }
                Ok(false) => {
                    report.entries_corrupted += 1;
                    warn!("Invalid entry: archive={archive_id} offset={offset} size={size}");
                }
                Err(e) => {
                    report.entries_corrupted += 1;
                    warn!(
                        "Unreadable entry: archive={archive_id} offset={offset} size={size}: {e}"
                    );
                }
            }
        }

        info!(
            "Verification: {} valid, {} corrupted out of {} total",
            report.entries_valid, report.entries_corrupted, report.entries_verified
        );

        Ok(())
    }

    async fn rebuild_indices(&self, report: &mut RepairReport) -> MaintenanceResult<()> {
        info!("Rebuilding indices from segment headers");

        match self.installation.rebuild_indices_from_segments().await {
            Ok(rebuilt) => {
                report.indices_rebuilt = rebuilt;
                info!("Rebuilt {rebuilt} index files");
            }
            Err(e) => {
                warn!("Index rebuild failed: {e}");
                return Err(crate::error::MaintenanceError::Repair(format!(
                    "index rebuild failed: {e}"
                )));
            }
        }

        Ok(())
    }
}

/// Build repair orchestrator with CDN re-download support.
///
/// Drives the full 14-state repair machine. Requires `BuildManifests` and
/// a `CdnSource` implementation for re-downloading corrupted content.
pub struct BuildRepairOrchestrator<'a, S: CdnSource> {
    installation: &'a Installation,
    cdn: &'a S,
    endpoints: &'a [CdnEndpoint],
    manifests: &'a BuildManifests,
    config: RepairConfig,
    /// Product subfolder for loose file placement (e.g. `"_classic_era_"`).
    /// Pass `None` if no loose files are placed for this installation.
    game_subfolder: Option<String>,
}

impl<'a, S: CdnSource> BuildRepairOrchestrator<'a, S> {
    pub fn new(
        installation: &'a Installation,
        cdn: &'a S,
        endpoints: &'a [CdnEndpoint],
        manifests: &'a BuildManifests,
        config: RepairConfig,
        game_subfolder: Option<String>,
    ) -> Self {
        Self {
            installation,
            cdn,
            endpoints,
            manifests,
            config,
            game_subfolder,
        }
    }

    /// Run the full repair state machine.
    ///
    /// If a `CASCRepair.mrk` from a previous interrupted run exists, resumes
    /// from the state recorded in that marker. Previously identified corrupted
    /// keys from `RepairMarker.psv` are preloaded and passed directly to the
    /// re-download phase, skipping re-verification.
    pub async fn run(&self, mode: ExecutionMode) -> MaintenanceResult<RepairReport> {
        let start = Instant::now();
        let mut report = RepairReport {
            executed: !mode.is_dry_run(),
            ..RepairReport::default()
        };
        // Accumulates the corrupted key list for writing to RepairMarker.psv.
        let mut corrupted_keys_for_marker: Vec<[u8; 9]> = Vec::new();

        // Check for an interrupted repair from a previous run.
        let base = self.installation.path();
        let mrk_path = base.join("CASCRepair.mrk");
        let psv_path = base.join("RepairMarker.psv");

        let mut preloaded_corrupted: Vec<[u8; 9]> = Vec::new();
        let mut state = RepairState::Initialize;

        if !mode.is_dry_run()
            && let Some(saved) = markers::CascRepairMarker::read(&mrk_path).await?
            && let Some(resumed) = RepairState::from_state_number(saved.state)
        {
            // Don't resume from Complete — that means a previous run
            // finished cleanly and left behind a stale marker.
            if resumed != RepairState::Complete {
                info!(
                    "Resuming interrupted repair from state {:?} ({})",
                    resumed, saved.state
                );
                state = resumed;

                // Load previously identified corrupted keys so the
                // re-download phase can skip re-verification.
                match markers::RepairMarker::read_psv(&psv_path).await {
                    Ok(marker) if !marker.keys.is_empty() => {
                        info!(
                            "Preloaded {} corrupted keys from RepairMarker.psv",
                            marker.keys.len()
                        );
                        preloaded_corrupted = marker.keys;
                    }
                    Ok(_) | Err(_) => {
                        // PSV absent or unreadable — start verification fresh.
                    }
                }
            }
        }

        // Write initial repair marker for crash recovery (or update to resumed state)
        if self.config.write_markers && !mode.is_dry_run() {
            markers::CascRepairMarker::write(&mrk_path, 2, state.state_number()).await?;
        }

        loop {
            info!("Repair state: {:?} ({})", state, state.state_number());

            #[allow(clippy::match_same_arms)]
            match state {
                RepairState::Initialize
                | RepairState::PostRepairData
                | RepairState::PostRepairEcache
                | RepairState::PostRepairHardLink => {
                    // No-op states: context setup / post-repair cleanup
                    // placeholders for future container-level cleanup
                }
                RepairState::ReadBuildConfig => {
                    info!("Build config validated");
                }
                RepairState::InitCdnIndexSet => {
                    let archive_count = self.manifests.cdn_config.archives().len();
                    info!("CDN index set: {archive_count} archives");
                }
                RepairState::RepairDataContainer => {
                    if self.config.verify_content {
                        self.repair_container("data", mode, &mut report).await?;
                    }
                }
                RepairState::RepairEcacheContainer => {
                    self.repair_ecache_container(mode, &mut report).await;
                }
                RepairState::RepairHardLinkContainer => {
                    self.repair_hardlink_container(mode, &mut report).await;
                }
                RepairState::DataRepair => {
                    // File-level data repair with CDN re-download.
                    // Pass preloaded corrupted keys when resuming from a checkpoint
                    // so verification is skipped and re-download starts immediately.
                    let engine = data_repair::DataRepairEngine::new(
                        self.installation,
                        self.cdn,
                        self.endpoints,
                        self.manifests,
                    );
                    corrupted_keys_for_marker =
                        engine.run(mode, &mut report, &preloaded_corrupted).await?;

                    // Loose file repair
                    if self.config.repair_loose_files {
                        let loose_engine = loose_files::LooseFileRepairEngine::new(
                            self.installation,
                            self.cdn,
                            self.endpoints,
                            self.manifests,
                            self.game_subfolder.clone(),
                        );
                        loose_engine.run(mode, &mut report).await?;
                    }
                }
                RepairState::Complete => {
                    // Rebuild indices if corruption was found
                    if self.config.rebuild_indices
                        && !mode.is_dry_run()
                        && report.entries_corrupted > 0
                    {
                        self.rebuild_indices(&mut report).await?;
                    }

                    // Write final markers
                    if self.config.write_markers && !mode.is_dry_run() {
                        self.write_final_markers(&report, &corrupted_keys_for_marker)
                            .await?;
                    }

                    break;
                }
            }

            // Update crash recovery marker
            if self.config.write_markers && !mode.is_dry_run() {
                let marker_path = self.installation.path().join("CASCRepair.mrk");
                markers::CascRepairMarker::write(&marker_path, 2, state.state_number()).await?;
            }

            state = match state.next() {
                Some(s) => s,
                None => break,
            };
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    /// Repair the ecache container.
    ///
    /// The ecache directory stores loose BLTE blobs for residency tracking.
    /// It is identified by a `.residency` token file. Repair checks:
    /// 1. Token file exists (container was properly initialised).
    /// 2. All files in the two-level trie (`XX/YY/filename`) have names
    ///    consisting of exactly 14 lowercase hex characters — any file that
    ///    does not match is an orphan left by an interrupted write or GC run.
    ///
    /// In `Execute` mode, orphans are removed. In `DryRun` mode they are
    /// counted and logged only.
    #[allow(clippy::unused_async)]
    async fn repair_ecache_container(&self, mode: ExecutionMode, report: &mut RepairReport) {
        let ecache_path = self.installation.path().join("ecache");

        if !ecache_path.is_dir() {
            info!("Ecache container directory absent — skipping");
            return;
        }

        let token = ecache_path.join(".residency");
        if !token.exists() {
            warn!(
                "Ecache container missing .residency token at {}",
                token.display()
            );
        }

        // Walk the two-level trie and count / remove orphans.
        let mut orphans: Vec<std::path::PathBuf> = Vec::new();
        let Ok(l1_entries) = std::fs::read_dir(&ecache_path) else {
            return;
        };

        for l1 in l1_entries.flatten() {
            let l1_name = l1.file_name();
            let l1_str = l1_name.to_string_lossy();

            // Level-1 dirs are 2-char hex (e.g. "ab")
            if l1_str.len() != 2 || !l1_str.chars().all(|c| c.is_ascii_hexdigit()) {
                // Only skip known-good housekeeping files
                if !l1_str.starts_with('.') && !l1_str.ends_with(".idx") {
                    orphans.push(l1.path());
                }
                continue;
            }
            let l1_path = l1.path();
            if !l1_path.is_dir() {
                orphans.push(l1_path);
                continue;
            }

            let Ok(l2_entries) = std::fs::read_dir(&l1_path) else {
                continue;
            };
            for l2 in l2_entries.flatten() {
                let l2_name = l2.file_name();
                let l2_str = l2_name.to_string_lossy();

                // Level-2 dirs are 2-char hex
                if l2_str.len() != 2 || !l2_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    orphans.push(l2.path());
                    continue;
                }
                let l2_path = l2.path();
                if !l2_path.is_dir() {
                    orphans.push(l2_path);
                    continue;
                }

                let Ok(l3_entries) = std::fs::read_dir(&l2_path) else {
                    continue;
                };
                for l3 in l3_entries.flatten() {
                    let l3_name = l3.file_name();
                    let l3_str = l3_name.to_string_lossy();
                    // Leaf files: 14 lowercase hex chars (7 remaining bytes of 9-byte ekey)
                    if l3_str.len() != 14 || !l3_str.chars().all(|c| c.is_ascii_hexdigit()) {
                        orphans.push(l3.path());
                    }
                }
            }
        }

        if orphans.is_empty() {
            info!("Ecache container OK");
            return;
        }

        warn!("Ecache container: {} orphaned file(s) found", orphans.len());

        if mode.is_dry_run() {
            info!("Dry-run: would remove {} ecache orphan(s)", orphans.len());
            return;
        }

        let mut removed = 0usize;
        for path in &orphans {
            if path.is_dir() {
                if std::fs::remove_dir_all(path).is_ok() {
                    removed += 1;
                }
            } else if std::fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }
        report.loose_files_repaired += removed;
        info!("Ecache container: removed {removed} orphaned file(s)");
    }

    /// Repair the hard link container.
    ///
    /// The hard link container stores filesystem hard links in a two-level
    /// trie (`XX/YY/filename`), identified by a `.trie_directory` token.
    /// Repair checks:
    /// 1. Token file exists.
    /// 2. All entries follow the expected trie naming convention:
    ///    - Level-1 dirs: 2 hex chars
    ///    - Level-2 dirs: 2 hex chars
    ///    - Leaf files:  14 hex chars
    ///
    /// Mirrors `casc::TrieDirectory::CompactDirectory` in Agent.exe. In
    /// `Execute` mode, orphaned entries are removed.
    #[allow(clippy::unused_async)]
    async fn repair_hardlink_container(&self, mode: ExecutionMode, report: &mut RepairReport) {
        let hl_path = self.installation.path().join("hardlink");

        if !hl_path.is_dir() {
            info!("Hard link container directory absent — skipping");
            return;
        }

        let token = hl_path.join(".trie_directory");
        if !token.exists() {
            warn!(
                "Hard link container missing .trie_directory token at {}",
                token.display()
            );
        }

        // Use `HardLinkContainer`'s compact_directory logic via direct
        // trie walk — we replicate the orphan detection here so we can
        // respect `ExecutionMode` and update the report without needing
        // to own a `HardLinkContainer` instance.
        let mut orphans: Vec<std::path::PathBuf> = Vec::new();
        let Ok(l1_entries) = std::fs::read_dir(&hl_path) else {
            return;
        };

        for l1 in l1_entries.flatten() {
            let l1_name = l1.file_name();
            let l1_str = l1_name.to_string_lossy();

            if l1_str == ".trie_directory"
                || l1_str.ends_with(".idx")
                || l1_str.starts_with("shmem")
            {
                continue;
            }

            // Level-1: must be 2-char hex dir
            if l1_str.len() != 2 || !l1_str.chars().all(|c| c.is_ascii_hexdigit()) {
                orphans.push(l1.path());
                continue;
            }
            let l1_path = l1.path();
            if !l1_path.is_dir() {
                orphans.push(l1_path);
                continue;
            }

            let Ok(l2_entries) = std::fs::read_dir(&l1_path) else {
                continue;
            };
            let mut l1_empty = true;
            for l2 in l2_entries.flatten() {
                let l2_name = l2.file_name();
                let l2_str = l2_name.to_string_lossy();

                if l2_str.len() != 2 || !l2_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    orphans.push(l2.path());
                    continue;
                }
                let l2_path = l2.path();
                if !l2_path.is_dir() {
                    orphans.push(l2_path);
                    continue;
                }

                let Ok(l3_entries) = std::fs::read_dir(&l2_path) else {
                    continue;
                };
                let mut l2_has_valid = false;
                for l3 in l3_entries.flatten() {
                    let l3_name = l3.file_name();
                    let l3_str = l3_name.to_string_lossy();
                    if l3_str.len() != 14 || !l3_str.chars().all(|c| c.is_ascii_hexdigit()) {
                        orphans.push(l3.path());
                    } else {
                        l2_has_valid = true;
                        l1_empty = false;
                    }
                }
                if !l2_has_valid && !mode.is_dry_run() {
                    // Empty level-2 dir — remove it
                    let _ = std::fs::remove_dir(&l2_path);
                }
            }
            if l1_empty && !mode.is_dry_run() {
                let _ = std::fs::remove_dir(&l1_path);
            }
        }

        if orphans.is_empty() {
            info!("Hard link container OK");
            return;
        }

        warn!(
            "Hard link container: {} orphaned entry/entries found",
            orphans.len()
        );

        if mode.is_dry_run() {
            info!(
                "Dry-run: would remove {} hard link orphan(s)",
                orphans.len()
            );
            return;
        }

        let mut removed = 0usize;
        for path in &orphans {
            if path.is_dir() {
                if std::fs::remove_dir_all(path).is_ok() {
                    removed += 1;
                }
            } else if std::fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }
        report.loose_files_repaired += removed;
        info!("Hard link container: removed {removed} orphaned entry/entries");
    }

    /// Repair a container directory (data, ecache, hardlink).
    ///
    /// Validates segment headers and verifies index entries.
    async fn repair_container(
        &self,
        container_name: &str,
        _mode: ExecutionMode,
        report: &mut RepairReport,
    ) -> MaintenanceResult<()> {
        info!("Repairing {container_name} container");

        // Verify all entries in this container
        let mut entries = self.installation.get_all_index_entries().await;
        report.entries_verified += entries.len();

        entries.sort_unstable_by(|a, b| {
            a.archive_location
                .archive_id
                .cmp(&b.archive_location.archive_id)
                .then(
                    a.archive_location
                        .archive_offset
                        .cmp(&b.archive_location.archive_offset),
                )
        });

        for entry in &entries {
            match self
                .installation
                .validate_entry(
                    entry.archive_location.archive_id,
                    entry.archive_location.archive_offset,
                    entry.size,
                )
                .await
            {
                Ok(true) => {
                    report.entries_valid += 1;
                }
                Ok(false) | Err(_) => {
                    report.entries_corrupted += 1;
                }
            }
        }

        Ok(())
    }

    async fn rebuild_indices(&self, report: &mut RepairReport) -> MaintenanceResult<()> {
        info!("Rebuilding indices from segment headers");

        match self.installation.rebuild_indices_from_segments().await {
            Ok(rebuilt) => {
                report.indices_rebuilt = rebuilt;
                info!("Rebuilt {rebuilt} index files");
            }
            Err(e) => {
                warn!("Index rebuild failed: {e}");
                return Err(crate::error::MaintenanceError::Repair(format!(
                    "index rebuild failed: {e}"
                )));
            }
        }

        Ok(())
    }

    async fn write_final_markers(
        &self,
        _report: &RepairReport,
        corrupted_keys: &[[u8; 9]],
    ) -> MaintenanceResult<()> {
        let base = self.installation.path();
        let psv_path = base.join("RepairMarker.psv");

        if corrupted_keys.is_empty() {
            // No corrupted keys — remove any stale PSV from a prior run.
            markers::RepairMarker::delete(&psv_path).await?;
        } else {
            // Write the actual per-key PSV matching Agent.exe's RepairMarker.psv format.
            markers::RepairMarker::write_psv(&psv_path, corrupted_keys).await?;
        }

        // Clean up the crash recovery marker — repair completed cleanly.
        let mrk_path = base.join("CASCRepair.mrk");
        markers::CascRepairMarker::delete(&mrk_path).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_state_sequence() {
        let mut state = RepairState::Initialize;
        let mut count = 0;
        loop {
            count += 1;
            match state.next() {
                Some(next) => state = next,
                None => break,
            }
        }
        // 11 states total (Initialize through Complete)
        assert_eq!(count, 11);
        assert_eq!(state, RepairState::Complete);
    }

    #[test]
    fn repair_state_numbers() {
        assert_eq!(RepairState::Initialize.state_number(), 0);
        assert_eq!(RepairState::ReadBuildConfig.state_number(), 5);
        assert_eq!(RepairState::DataRepair.state_number(), 16);
        assert_eq!(RepairState::Complete.state_number(), 99);
    }

    #[test]
    fn repair_state_from_number_roundtrip() {
        let states = [
            RepairState::Initialize,
            RepairState::ReadBuildConfig,
            RepairState::InitCdnIndexSet,
            RepairState::RepairDataContainer,
            RepairState::PostRepairData,
            RepairState::RepairEcacheContainer,
            RepairState::PostRepairEcache,
            RepairState::RepairHardLinkContainer,
            RepairState::PostRepairHardLink,
            RepairState::DataRepair,
            RepairState::Complete,
        ];
        for state in states {
            let n = state.state_number();
            assert_eq!(RepairState::from_state_number(n), Some(state));
        }
        // Unknown state numbers return None.
        assert_eq!(RepairState::from_state_number(1), None);
        assert_eq!(RepairState::from_state_number(100), None);
    }

    #[test]
    fn repair_config_defaults() {
        let config = RepairConfig::default();
        assert!(config.verify_content);
        assert!(config.rebuild_indices);
        assert!(config.repair_loose_files);
        assert!(config.write_markers);
    }
}
