//! File-level data repair — verifies archive entries and re-downloads corrupted ones.
//!
//! Extends the existing `verify_content()` logic with CDN re-download capability.
//! When a corrupted entry is found, looks up the encoding key in manifests and
//! downloads a replacement from CDN.
//!
//! Re-download strategy (mirrors Agent.exe repair state 6 CDN index set):
//!
//! 1. Try loose blob download from each endpoint.
//! 2. If all loose blob attempts fail, look up the key in the local archive
//!    index files and retry with a byte-range request against the CDN archive.
//!
//! The archive index lookup uses the same `Data/indices/` files that were
//! downloaded during install. Keys are matched by full 16-byte EKey first,
//! then by 9-byte truncated key (matching the on-disk IDX format).

use cascette_client_storage::Installation;
use cascette_installation::pipeline::download::{ArchiveLookup, load_archive_indices};
use cascette_installation::{BuildManifests, CdnSource};
use cascette_protocol::{CdnEndpoint, ContentType};
use tracing::{info, warn};

use crate::ExecutionMode;
use crate::error::MaintenanceResult;
use crate::report::RepairReport;

/// Data repair engine with CDN re-download support.
pub struct DataRepairEngine<'a, S: CdnSource> {
    installation: &'a Installation,
    cdn: &'a S,
    endpoints: &'a [CdnEndpoint],
    manifests: &'a BuildManifests,
}

impl<'a, S: CdnSource> DataRepairEngine<'a, S> {
    pub const fn new(
        installation: &'a Installation,
        cdn: &'a S,
        endpoints: &'a [CdnEndpoint],
        manifests: &'a BuildManifests,
    ) -> Self {
        Self {
            installation,
            cdn,
            endpoints,
            manifests,
        }
    }

    /// Run data repair: verify all entries and re-download corrupted ones.
    ///
    /// When `preloaded_keys` is non-empty (resumed from a checkpoint), the
    /// verification scan is skipped and those keys are re-downloaded directly.
    ///
    /// Returns the list of corrupted 9-byte truncated keys identified during
    /// this run (or the preloaded set when resuming) so the caller can write
    /// them to `RepairMarker.psv`.
    pub async fn run(
        &self,
        mode: ExecutionMode,
        report: &mut RepairReport,
        preloaded_keys: &[[u8; 9]],
    ) -> MaintenanceResult<Vec<[u8; 9]>> {
        let corrupted_keys: Vec<[u8; 9]> = if preloaded_keys.is_empty() {
            // Step 1: Verify all entries
            let mut entries = self.installation.get_all_index_entries().await;
            report.entries_verified = entries.len();

            info!("Verifying {} index entries", entries.len());

            // Sort by archive ID and offset for sequential I/O
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

            let mut found: Vec<[u8; 9]> = Vec::new();

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
                        found.push(entry.key);
                        warn!("Corrupted entry: archive={archive_id} offset={offset} size={size}");
                    }
                    Err(e) => {
                        report.entries_corrupted += 1;
                        found.push(entry.key);
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

            found
        } else {
            // Resumed from checkpoint: skip verification, re-download the
            // previously identified set directly.
            info!(
                "Skipping verification: using {} preloaded corrupted keys from checkpoint",
                preloaded_keys.len()
            );
            report.entries_corrupted = preloaded_keys.len();
            preloaded_keys.to_vec()
        };

        // Step 2: Re-download corrupted entries from CDN
        if !corrupted_keys.is_empty() && !mode.is_dry_run() {
            // Build archive index lookup for byte-range fallback.
            // This mirrors Agent.exe repair state 6 (InitCdnIndexSet) which initialises
            // a CDN index set covering both data and patch archives.
            let archive_lookup = self.build_archive_lookup();

            self.redownload_corrupted(&corrupted_keys, &archive_lookup, report)
                .await?;
        }

        Ok(corrupted_keys)
    }

    /// Build an archive index lookup from locally cached index files.
    ///
    /// Reads `Data/indices/*.index` files that were downloaded during install.
    /// Returns an empty map if the indices directory doesn't exist or no archives
    /// are configured, rather than failing — re-download will fall back to
    /// loose blob only in that case.
    fn build_archive_lookup(&self) -> ArchiveLookup {
        // installation.path() returns Data/, so indices live at Data/indices/
        let indices_dir = self.installation.path().join("indices");
        if !indices_dir.is_dir() {
            return ArchiveLookup::new();
        }

        // Collect all archive keys from CDN config (data + patch archives).
        let mut archive_keys: Vec<String> = self
            .manifests
            .cdn_config
            .archives()
            .iter()
            .map(|a| a.content_key.clone())
            .collect();

        // Include patch archives if present — Agent.exe's CDN index set covers both.
        for patch_archive in self.manifests.cdn_config.patch_archives() {
            archive_keys.push(patch_archive.content_key.clone());
        }

        match load_archive_indices(&indices_dir, &archive_keys) {
            Ok(lookup) => {
                info!(
                    "Archive index lookup: {} entries from {} archives",
                    lookup.len(),
                    archive_keys.len()
                );
                lookup
            }
            Err(e) => {
                warn!("Failed to build archive index lookup: {e}; falling back to loose blob only");
                ArchiveLookup::new()
            }
        }
    }

    /// Attempt to re-download corrupted entries from CDN.
    async fn redownload_corrupted(
        &self,
        corrupted_keys: &[[u8; 9]],
        archive_lookup: &ArchiveLookup,
        report: &mut RepairReport,
    ) -> MaintenanceResult<()> {
        if self.endpoints.is_empty() {
            warn!("No CDN endpoints configured, skipping re-download");
            report.redownload_failed = corrupted_keys.len();
            return Ok(());
        }

        info!(
            "Attempting to re-download {} corrupted entries",
            corrupted_keys.len()
        );

        for key in corrupted_keys {
            match self.try_redownload_entry(key, archive_lookup).await {
                Ok(true) => {
                    report.entries_redownloaded += 1;
                }
                Ok(false) => {
                    report.redownload_failed += 1;
                    warn!("No CDN source found for key {}", hex::encode(key));
                }
                Err(e) => {
                    report.redownload_failed += 1;
                    warn!("Re-download failed for key {}: {e}", hex::encode(key));
                }
            }
        }

        info!(
            "Re-download: {} succeeded, {} failed",
            report.entries_redownloaded, report.redownload_failed
        );

        Ok(())
    }

    /// Try to re-download a single entry from CDN.
    ///
    /// Strategy:
    /// 1. Resolve the 9-byte truncated key to a full 16-byte encoding key via
    ///    the encoding table's EKey pages.
    /// 2. Try loose blob download from each endpoint.
    /// 3. If all loose blob attempts fail, look up the key in the archive index
    ///    and retry with a byte-range request (mirrors Agent.exe CDN index set).
    async fn try_redownload_entry(
        &self,
        truncated_key: &[u8; 9],
        archive_lookup: &ArchiveLookup,
    ) -> MaintenanceResult<bool> {
        let Some(full_ekey) = self.find_full_encoding_key(truncated_key) else {
            return Ok(false);
        };

        let ekey_bytes = full_ekey.as_bytes();

        // Attempt 1: loose blob from each endpoint
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
                    warn!("CDN loose blob failed from {}: {e}", endpoint.host);
                }
            }
        }

        // Attempt 2: archive byte-range fallback
        // Look up full 16-byte key first, then 9-byte truncated (matching IDX format).
        // ArchiveLookup keys are Vec<u8>; HashMap<Vec<u8>, _> accepts &[u8] via Borrow.
        let archive_loc = archive_lookup
            .get(ekey_bytes as &[u8])
            .or_else(|| archive_lookup.get(&ekey_bytes[..9]));

        let Some(loc) = archive_loc else {
            return Ok(false);
        };

        for endpoint in self.endpoints {
            match self
                .cdn
                .download_range(
                    endpoint,
                    ContentType::Data,
                    &loc.archive_key,
                    loc.offset,
                    u64::from(loc.size),
                )
                .await
            {
                Ok(data) => {
                    self.installation.write_raw_blte(data).await?;
                    return Ok(true);
                }
                Err(e) => {
                    warn!("CDN archive range failed from {}: {e}", endpoint.host);
                }
            }
        }

        Ok(false)
    }

    /// Find the full 16-byte encoding key matching a 9-byte truncated key.
    fn find_full_encoding_key(&self, truncated: &[u8; 9]) -> Option<cascette_crypto::EncodingKey> {
        for page in &self.manifests.encoding.ekey_pages {
            for entry in &page.entries {
                if entry.encoding_key.first_9() == *truncated {
                    return Some(entry.encoding_key);
                }
            }
        }
        None
    }
}
