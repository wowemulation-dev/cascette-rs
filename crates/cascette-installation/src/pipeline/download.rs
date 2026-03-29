//! Priority-ordered concurrent download executor.
//!
//! Downloads artifacts from CDN with slot-based concurrent fan-out.
//!
//! All entries in a batch are submitted simultaneously. The batch completes
//! when the last slot finishes. This maps to Tokio's `buffer_unordered` with
//! concurrency bounded by `max_connections_global` (default 12).
//!
//! ## Concurrency model
//!
//! Each artifact's CDN fetch runs as an independent async task. Up to
//! `max_connections_global` fetches run simultaneously. Result processing
//! (CASC write, loose file placement, scorer update, checkpoint) is sequential —
//! `on_file_written` holds a `Mutex`-locked handler, so it cannot be parallelised.
//!
//! ```text
//! artifacts -> fan-out (buffer_unordered, N=max_connections_global)
//!   each slot: try loose blob -> try archive range -> FetchOutcome
//!   result loop (sequential):
//!     apply scorer updates
//!     if is_loose_file: on_file_written(artifact, inst, Some(blte_data))
//!     else: write_raw_blte -> on_file_written(artifact, inst, None)
//!     checkpoint.mark_completed
//! ```

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::{debug, info, warn};

use cascette_client_storage::Installation;
use cascette_formats::archive::ArchiveIndex;
use cascette_protocol::{CdnEndpoint, ContentType};

use crate::cdn_source::CdnSource;
use crate::checkpoint::Checkpoint;
use crate::config::InstallConfig;
use crate::endpoint_scorer::EndpointScorer;
use crate::error::InstallationResult;
use crate::pipeline::classify::ArtifactDescriptor;
use crate::progress::ProgressEvent;

/// Location of content within a CDN archive.
#[derive(Debug, Clone)]
pub struct ArchiveLocation {
    /// Archive content key (hex hash).
    pub archive_key: Vec<u8>,
    /// Byte offset within the archive.
    pub offset: u64,
    /// Compressed size in bytes.
    pub size: u32,
}

/// Lookup map from encoding key bytes to archive location.
///
/// Built by parsing `.index` files from `Data/indices/`. Used as fallback
/// when loose blob downloads fail (common for historical builds where
/// Blizzard's CDN has purged individual blobs but archives remain).
pub type ArchiveLookup = HashMap<Vec<u8>, ArchiveLocation>;

/// Parse all archive index files and build a lookup map.
///
/// For each archive key, reads the corresponding `.index` file from
/// `indices_dir`, parses it with `ArchiveIndex::parse()`, and maps every
/// entry's encoding key to its archive location (archive key + offset + size).
pub fn load_archive_indices(
    indices_dir: &Path,
    archive_keys: &[String],
) -> InstallationResult<ArchiveLookup> {
    let mut lookup = HashMap::new();

    for archive_key in archive_keys {
        let index_path = indices_dir.join(format!("{archive_key}.index"));
        if !index_path.exists() {
            debug!(archive = %archive_key, "index file not found, skipping");
            continue;
        }

        let file = std::fs::File::open(&index_path).map_err(|e| {
            crate::error::InstallationError::Format(format!(
                "failed to open index {archive_key}: {e}"
            ))
        })?;
        let mut reader = std::io::BufReader::new(file) as std::io::BufReader<std::fs::File>;

        match ArchiveIndex::parse(&mut reader) {
            Ok(index) => {
                let archive_key_bytes = hex::decode(archive_key).unwrap_or_default();
                for entry in &index.entries {
                    if entry.is_zero() {
                        continue;
                    }
                    lookup.insert(
                        entry.encoding_key.clone(),
                        ArchiveLocation {
                            archive_key: archive_key_bytes.clone(),
                            offset: entry.offset,
                            size: entry.size,
                        },
                    );
                }
                debug!(
                    archive = %archive_key,
                    entries = index.entries.len(),
                    "parsed archive index"
                );
            }
            Err(e) => {
                warn!(archive = %archive_key, error = %e, "failed to parse archive index, skipping");
            }
        }
    }

    info!(total_entries = lookup.len(), "archive index lookup built");
    Ok(lookup)
}

/// Look up an encoding key in the archive index map.
///
/// Tries exact match first (16-byte keys from CDN indices). If not found,
/// tries matching by the key length stored in the index (for indices with
/// shorter keys like 9-byte truncated entries).
fn lookup_archive<'a>(
    lookup: &'a ArchiveLookup,
    encoding_key: &[u8],
) -> Option<&'a ArchiveLocation> {
    let full_key = encoding_key.to_vec();
    if let Some(loc) = lookup.get(&full_key) {
        return Some(loc);
    }

    // Try truncated key lengths (9 bytes is common for some indices)
    for truncated_len in [9_usize] {
        if encoding_key.len() > truncated_len {
            let truncated = encoding_key[..truncated_len].to_vec();
            if let Some(loc) = lookup.get(&truncated) {
                return Some(loc);
            }
        }
    }

    None
}

/// Category of a download failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureCategory {
    /// CDN / HTTP / timeout error.
    Network,
    /// Hash verification mismatch after download.
    HashMismatch,
    /// Local storage write error.
    Storage,
    /// BLTE decode or format error.
    Decode,
}

impl std::fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network => write!(f, "network"),
            Self::HashMismatch => write!(f, "hash_mismatch"),
            Self::Storage => write!(f, "storage"),
            Self::Decode => write!(f, "decode"),
        }
    }
}

/// A single file that failed during download.
#[derive(Debug, Clone)]
pub struct FailedFile {
    /// Install path or identifier.
    pub install_path: String,
    /// Hex-encoded encoding key.
    pub encoding_key: String,
    /// Failure category.
    pub category: FailureCategory,
    /// CDN host that served the data (if applicable).
    pub endpoint_host: Option<String>,
}

/// Classify an `InstallationError` into a `FailureCategory`.
pub fn categorize_error(error: &crate::error::InstallationError) -> FailureCategory {
    match error {
        crate::error::InstallationError::Protocol(_) | crate::error::InstallationError::Cdn(_) => {
            FailureCategory::Network
        }
        crate::error::InstallationError::Storage(se) => {
            let msg = se.to_string();
            if msg.contains("Verification") {
                FailureCategory::HashMismatch
            } else {
                FailureCategory::Storage
            }
        }
        crate::error::InstallationError::Format(_) => FailureCategory::Decode,
        _ => FailureCategory::Storage,
    }
}

/// Result of a download operation.
#[derive(Debug, Default)]
pub struct DownloadReport {
    /// Number of files downloaded.
    pub downloaded: usize,
    /// Number of files that failed.
    pub failed: usize,
    /// Number of files skipped (already present or checkpoint).
    pub skipped: usize,
    /// Total bytes downloaded.
    pub bytes_downloaded: u64,
    /// Structured failure details.
    pub failed_files: Vec<FailedFile>,
}

impl DownloadReport {
    /// Summary of failures grouped by category with counts.
    #[must_use]
    pub fn failure_summary(&self) -> String {
        if self.failed_files.is_empty() {
            return String::from("no failures");
        }
        let mut counts = std::collections::HashMap::new();
        for f in &self.failed_files {
            *counts.entry(f.category.to_string()).or_insert(0usize) += 1;
        }
        let mut parts: Vec<String> = counts
            .into_iter()
            .map(|(cat, count)| format!("{cat}: {count}"))
            .collect();
        parts.sort();
        parts.join(", ")
    }
}

/// Scorer event recorded by a fetch slot, applied sequentially after all slots complete.
///
/// Each slot accumulates its stats and the result loop applies them to the scorer.
#[derive(Debug)]
enum ScoreEvent {
    /// Endpoint served data successfully (no weight change).
    Success,
    /// Endpoint returned an error; apply the error's failure weight.
    Failure(crate::error::InstallationError),
}

/// Outcome of a single CDN fetch slot.
///
/// Returned by each concurrent future and consumed sequentially in the result loop.
/// Carries all state needed by the scorer update and CASC write phases without
/// requiring shared mutable state between concurrent tasks.
#[derive(Debug)]
enum FetchOutcome {
    /// Data downloaded successfully from CDN.
    Downloaded {
        data: Vec<u8>,
        /// Host that served the data (for hash-mismatch attribution).
        serving_host: Option<String>,
        /// Scorer events to apply after fetch (host, event) pairs.
        score_events: Vec<(String, ScoreEvent)>,
    },
    /// All endpoints failed; no data was downloaded.
    Failed {
        error_msg: String,
        category: FailureCategory,
    },
}

/// Fetch one artifact from CDN.
///
/// When an archive location is known (from parsed CDN indices), fetches via
/// archive byte-range request directly — no loose blob attempt. This avoids
/// thousands of 404s against mirrors and CDN endpoints where individual blobs
/// have long been pruned.
///
/// When no archive location is known (file not in any index), falls back to
/// loose blob download by encoding key.
///
/// Returns a `FetchOutcome` carrying the data (or error) plus scorer events
/// to be applied by the result loop. No shared mutable state is accessed.
async fn fetch_artifact<S: CdnSource>(
    cdn: Arc<S>,
    endpoints: Vec<CdnEndpoint>,
    ekey_bytes: Vec<u8>,
    archive_loc: Option<ArchiveLocation>,
) -> FetchOutcome {
    let mut score_events: Vec<(String, ScoreEvent)> = Vec::new();
    let mut serving_host: Option<String> = None;
    let mut data_result: Option<Vec<u8>> = None;

    if let Some(loc) = archive_loc {
        // Archive location known — fetch via byte-range request directly.
        // This is the common path for builds where CDN indices have been downloaded.
        for ep in &endpoints {
            match cdn
                .download_range(
                    ep,
                    ContentType::Data,
                    &loc.archive_key,
                    loc.offset,
                    u64::from(loc.size),
                )
                .await
            {
                Ok(data) => {
                    score_events.push((ep.host.clone(), ScoreEvent::Success));
                    serving_host = Some(ep.host.clone());
                    data_result = Some(data);
                    break;
                }
                Err(e) => {
                    debug!(
                        host = %ep.host,
                        error = %e,
                        "archive range download failed, trying next endpoint"
                    );
                    score_events.push((ep.host.clone(), ScoreEvent::Failure(e)));
                }
            }
        }
    } else {
        // No archive location — try loose blob download by encoding key.
        // This path is used for files not found in any CDN archive index.
        for ep in &endpoints {
            match cdn.download(ep, ContentType::Data, &ekey_bytes).await {
                Ok(data) => {
                    score_events.push((ep.host.clone(), ScoreEvent::Success));
                    serving_host = Some(ep.host.clone());
                    data_result = Some(data);
                    break;
                }
                Err(e) => {
                    debug!(host = %ep.host, error = %e, "loose blob failed, trying next endpoint");
                    score_events.push((ep.host.clone(), ScoreEvent::Failure(e)));
                }
            }
        }
    }

    if let Some(data) = data_result {
        FetchOutcome::Downloaded {
            data,
            serving_host,
            score_events,
        }
    } else {
        // Derive category from the last non-success score event
        let category = score_events
            .iter()
            .rev()
            .find_map(|(_, ev)| {
                if let ScoreEvent::Failure(e) = ev {
                    Some(categorize_error(e))
                } else {
                    None
                }
            })
            .unwrap_or(FailureCategory::Network);
        FetchOutcome::Failed {
            error_msg: "all CDN endpoints failed".to_string(),
            category,
        }
    }
}

/// Execute downloads for a set of artifacts.
///
/// Fans out up to `max_connections_global` concurrent CDN fetches, matching
/// Slot-based fan-out download executor. Result processing
/// (CASC write, loose file placement, scorer update, checkpoint) is sequential.
///
/// ## Artifact routing
///
/// - `artifact.is_loose_file = true`: raw BLTE bytes passed to `on_file_written(artifact,
///   inst, Some(data))`. Callback decodes BLTE and writes to filesystem. No CASC write.
/// - `artifact.is_loose_file = false`: data written to CASC via `write_raw_blte`, then
///   `on_file_written(artifact, inst, None)` for any post-write hardlinking.
#[allow(clippy::too_many_arguments)]
pub async fn execute_downloads<S, F, Fut>(
    config: &InstallConfig,
    cdn: Arc<S>,
    installation: Arc<Installation>,
    endpoints: &[CdnEndpoint],
    artifacts: &[ArtifactDescriptor],
    archive_lookup: &ArchiveLookup,
    checkpoint: &mut Checkpoint,
    progress: &(impl Fn(ProgressEvent) + Send + Sync),
    on_file_written: F,
) -> InstallationResult<DownloadReport>
where
    S: CdnSource + 'static,
    F: Fn(&ArtifactDescriptor, &Arc<Installation>, Option<Vec<u8>>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    if endpoints.is_empty() {
        return Err(crate::error::InstallationError::InvalidConfig(
            "no CDN endpoints".to_string(),
        ));
    }

    // Adaptive endpoint scoring: reorders endpoints so healthier servers are tried first.
    // Score is snapshotted before each batch so concurrent futures use consistent ordering.
    let mut scorer = EndpointScorer::new();

    let mut downloaded: usize = 0;
    let mut failed: usize = 0;
    let mut skipped: usize = 0;
    let mut bytes_downloaded: u64 = 0;
    let mut failed_files: Vec<FailedFile> = Vec::new();
    let mut since_checkpoint: usize = 0;

    // Partition artifacts into skipped (already checkpointed) and pending.
    let mut pending: Vec<ArtifactDescriptor> = Vec::new();
    for artifact in artifacts {
        let ekey_hex = hex::encode(artifact.encoding_key.as_bytes());
        if checkpoint.is_completed(&ekey_hex) {
            skipped += 1;
        } else {
            pending.push(artifact.clone());
        }
    }

    info!(
        total = artifacts.len(),
        pending = pending.len(),
        skipped = skipped,
        "starting concurrent download fan-out"
    );

    // Build one future per pending artifact. Each captures owned data so it can run
    // concurrently without holding references to the shared scorer or checkpoint.
    //
    // Endpoint ordering is snapshotted once here (no failures yet) and per-slot
    // failures will be applied to the scorer in the sequential result loop.
    // For large batches with many failures, the scorer stays current across the
    // stream because buffer_unordered yields results as they complete — the result
    // loop can update the scorer between result processing but the endpoint list
    // passed into each future is already cloned at submission time.
    // Endpoint selection happens at submission time, before the batch starts,
    // not mid-flight.
    let sorted_eps: Vec<CdnEndpoint> = scorer
        .sort_endpoints(endpoints)?
        .into_iter()
        .cloned()
        .collect();

    // Build owned futures: each carries all data it needs so results are self-contained.
    // The artifact descriptor is cloned into the future output so the result loop can
    // correlate results with artifacts even when futures complete out-of-order.
    let artifact_futures = pending.into_iter().map(|artifact| {
        let ekey_bytes = artifact.encoding_key.as_bytes().to_vec();
        let archive_loc = lookup_archive(archive_lookup, &ekey_bytes).cloned();
        let cdn = Arc::clone(&cdn);
        let eps = sorted_eps.clone();
        let artifact_clone = artifact.clone();

        progress(ProgressEvent::FileDownloading {
            path: artifact.path.clone(),
            size: u64::from(artifact.file_size),
        });

        async move {
            let outcome = fetch_artifact(cdn, eps, ekey_bytes, archive_loc).await;
            (artifact_clone, outcome)
        }
    });

    // Fan out: up to max_connections_global concurrent CDN fetches.
    // Results arrive in completion order (not submission order); each result carries
    // its own ArtifactDescriptor clone so the loop does not need index tracking.
    let mut result_stream =
        stream::iter(artifact_futures).buffer_unordered(config.max_connections_global);

    while let Some((artifact, outcome)) = result_stream.next().await {
        let ekey_hex = hex::encode(artifact.encoding_key.as_bytes());

        match outcome {
            FetchOutcome::Downloaded {
                data,
                serving_host,
                score_events,
            } => {
                // Apply scorer updates from this slot (mirrors OnStreamComplete → UpdateServerStats)
                for (host, event) in score_events {
                    match event {
                        ScoreEvent::Success => scorer.record_success(&host),
                        ScoreEvent::Failure(e) => scorer.record_failure(&host, &e),
                    }
                }

                // Route based on artifact type:
                // - Loose files (install manifest): write directly to filesystem, skip CASC.
                // - CASC files (download manifest): write to archive storage.
                let write_result: Result<(), crate::error::InstallationError> =
                    if artifact.is_loose_file {
                        // Pass raw BLTE bytes to callback for direct filesystem write.
                        // The LooseFileHandler decodes BLTE and writes to the destination path.
                        on_file_written(&artifact, &installation, Some(data)).await;
                        Ok(())
                    } else {
                        installation
                            .write_raw_blte(data)
                            .await
                            .map(|_| ())
                            .map_err(crate::error::InstallationError::from)
                    };

                match write_result {
                    Ok(()) => {
                        if artifact.is_loose_file {
                            debug!(path = %artifact.path, "loose file written to filesystem");
                        } else {
                            debug!(path = %artifact.path, "file written to CASC storage");
                            on_file_written(&artifact, &installation, None).await;
                        }
                        downloaded += 1;
                        bytes_downloaded += u64::from(artifact.file_size);
                        checkpoint.mark_completed(ekey_hex);
                        since_checkpoint += 1;

                        progress(ProgressEvent::FileComplete {
                            path: artifact.path.clone(),
                            bytes: u64::from(artifact.file_size),
                        });
                    }
                    Err(e) => {
                        warn!(path = %artifact.path, error = %e, "failed to write file");
                        let error_msg = e.to_string();
                        let category = categorize_error(&e);
                        // Hash mismatch after download: penalize the serving endpoint
                        if category == FailureCategory::HashMismatch
                            && let Some(ref host) = serving_host
                        {
                            scorer.record_data_corruption(host);
                        }
                        failed += 1;
                        failed_files.push(FailedFile {
                            install_path: artifact.path.clone(),
                            encoding_key: ekey_hex.clone(),
                            category,
                            endpoint_host: serving_host,
                        });
                        progress(ProgressEvent::FileFailed {
                            path: artifact.path.clone(),
                            error: error_msg,
                        });
                    }
                }
            }
            FetchOutcome::Failed {
                error_msg,
                category,
            } => {
                warn!(path = %artifact.path, error = %error_msg, "failed to download file");
                failed += 1;
                failed_files.push(FailedFile {
                    install_path: artifact.path.clone(),
                    encoding_key: ekey_hex.clone(),
                    category,
                    endpoint_host: None,
                });
                progress(ProgressEvent::FileFailed {
                    path: artifact.path.clone(),
                    error: error_msg,
                });
            }
        }

        // Checkpoint save
        if since_checkpoint >= config.checkpoint_interval {
            checkpoint.write(&config.install_path).await?;
            progress(ProgressEvent::CheckpointSaved {
                completed: checkpoint.completed_count(),
                remaining: checkpoint.remaining_count(),
            });
            since_checkpoint = 0;
        }
    }

    // Final checkpoint save
    if since_checkpoint > 0 {
        checkpoint.write(&config.install_path).await?;
    }

    info!(
        downloaded = downloaded,
        failed = failed,
        skipped = skipped,
        bytes = bytes_downloaded,
        "download phase complete"
    );

    Ok(DownloadReport {
        downloaded,
        failed,
        skipped,
        bytes_downloaded,
        failed_files,
    })
}

// ---------------------------------------------------------------------------
// Archive-batched download: groups files by archive, coalesces adjacent ranges,
// and fetches large contiguous regions instead of individual per-file requests.
// Reduces hundreds of thousands of HTTP requests to hundreds.
// ---------------------------------------------------------------------------

/// Entry within an archive group, sorted by offset for coalescing.
#[derive(Debug, Clone)]
struct ArchiveEntry {
    artifact_idx: usize,
    offset: u64,
    size: u32,
}

/// A contiguous range within an archive covering one or more entries.
#[derive(Debug)]
struct CoalescedRange {
    offset: u64,
    total_size: u64,
    /// Indices into the parent ArchiveGroup.entries vec.
    entry_indices: Vec<usize>,
}

/// Group of files residing in the same CDN archive.
struct ArchiveGroup {
    archive_key: Vec<u8>,
    entries: Vec<ArchiveEntry>,
}

/// Merge adjacent entries within an archive into fewer HTTP range requests.
/// Entries must be sorted by offset. Gaps <= `gap_threshold` bytes are absorbed
/// into the range (cheaper than a new HTTP round-trip).
fn coalesce_ranges(entries: &[ArchiveEntry], gap_threshold: u64) -> Vec<CoalescedRange> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut current = CoalescedRange {
        offset: entries[0].offset,
        total_size: u64::from(entries[0].size),
        entry_indices: vec![0],
    };

    for (i, entry) in entries.iter().enumerate().skip(1) {
        let current_end = current.offset + current.total_size;
        let next_start = entry.offset;

        if next_start <= current_end + gap_threshold {
            // Merge: extend current range to cover this entry
            let new_end = next_start + u64::from(entry.size);
            current.total_size = new_end - current.offset;
            current.entry_indices.push(i);
        } else {
            // Gap too large — finalize current, start new range
            ranges.push(current);
            current = CoalescedRange {
                offset: entry.offset,
                total_size: u64::from(entry.size),
                entry_indices: vec![i],
            };
        }
    }
    ranges.push(current);
    ranges
}

/// Execute downloads using archive-batched range requests.
///
/// Groups artifacts by their containing CDN archive (via `archive_lookup`),
/// sorts entries by offset within each archive, coalesces adjacent entries
/// into larger byte ranges, and fetches each range with a single HTTP request.
/// This reduces ~131K individual requests to ~600 archive-level requests.
///
/// Falls back to individual per-file fetch for entries not found in the
/// archive lookup (e.g., loose blobs).
#[allow(clippy::too_many_arguments)]
pub async fn execute_batched_downloads<S, F, Fut>(
    config: &InstallConfig,
    cdn: Arc<S>,
    installation: Arc<Installation>,
    endpoints: &[CdnEndpoint],
    artifacts: &[ArtifactDescriptor],
    archive_lookup: &ArchiveLookup,
    checkpoint: &mut Checkpoint,
    progress: &(impl Fn(ProgressEvent) + Send + Sync),
    on_file_written: F,
) -> InstallationResult<DownloadReport>
where
    S: CdnSource + 'static,
    F: Fn(&ArtifactDescriptor, &Arc<Installation>, Option<Vec<u8>>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    if endpoints.is_empty() {
        return Err(crate::error::InstallationError::InvalidConfig(
            "no CDN endpoints".to_string(),
        ));
    }

    let mut downloaded: usize = 0;
    let mut failed: usize = 0;
    let mut skipped: usize = 0;
    let mut bytes_downloaded: u64 = 0;
    let mut failed_files: Vec<FailedFile> = Vec::new();

    // Partition into skipped (checkpointed), archive-grouped, and loose (no archive loc).
    let mut archive_groups: HashMap<Vec<u8>, ArchiveGroup> = HashMap::new();
    let mut loose_artifacts: Vec<ArtifactDescriptor> = Vec::new();
    let mut pending_artifacts: Vec<ArtifactDescriptor> = Vec::new(); // ordered for index correlation

    for artifact in artifacts {
        let ekey_hex = hex::encode(artifact.encoding_key.as_bytes());
        if checkpoint.is_completed(&ekey_hex) {
            skipped += 1;
            continue;
        }

        let ekey_bytes = artifact.encoding_key.as_bytes();
        if let Some(loc) = lookup_archive(archive_lookup, ekey_bytes) {
            let artifact_idx = pending_artifacts.len();
            pending_artifacts.push(artifact.clone());

            let group = archive_groups
                .entry(loc.archive_key.clone())
                .or_insert_with(|| ArchiveGroup {
                    archive_key: loc.archive_key.clone(),
                    entries: Vec::new(),
                });
            group.entries.push(ArchiveEntry {
                artifact_idx,
                offset: loc.offset,
                size: loc.size,
            });
        } else if artifact.is_loose_file {
            loose_artifacts.push(artifact.clone());
            pending_artifacts.push(artifact.clone());
        } else {
            // No archive location and not loose — fall back to individual fetch
            loose_artifacts.push(artifact.clone());
            pending_artifacts.push(artifact.clone());
        }
    }

    // Sort entries within each archive by offset (enables coalescing)
    for group in archive_groups.values_mut() {
        group.entries.sort_by_key(|e| e.offset);
    }

    // Collect groups sorted by total size descending (big archives first)
    let mut groups: Vec<ArchiveGroup> = archive_groups.into_values().collect();
    groups.sort_by(|a, b| {
        let size_a: u64 = a.entries.iter().map(|e| u64::from(e.size)).sum();
        let size_b: u64 = b.entries.iter().map(|e| u64::from(e.size)).sum();
        size_b.cmp(&size_a)
    });

    let total_ranges: usize = groups
        .iter()
        .map(|g| coalesce_ranges(&g.entries, 64).len())
        .sum();

    info!(
        archives = groups.len(),
        coalesced_ranges = total_ranges,
        loose = loose_artifacts.len(),
        skipped = skipped,
        total = artifacts.len(),
        "batched download: grouped by archive"
    );

    // Phase 1: Download archive-batched entries
    let gap_threshold = 64u64;
    let mut scorer = EndpointScorer::new();
    let sorted_eps: Vec<CdnEndpoint> = scorer
        .sort_endpoints(endpoints)?
        .into_iter()
        .cloned()
        .collect();

    for group in &groups {
        let ranges = coalesce_ranges(&group.entries, gap_threshold);

        for range in &ranges {
            // Fetch the coalesced range from CDN
            let mut range_data: Option<Vec<u8>> = None;
            let mut serving_host: Option<String> = None;

            for ep in &sorted_eps {
                match cdn
                    .download_range(
                        ep,
                        ContentType::Data,
                        &group.archive_key,
                        range.offset,
                        range.total_size,
                    )
                    .await
                {
                    Ok(data) => {
                        scorer.record_success(&ep.host);
                        serving_host = Some(ep.host.clone());
                        range_data = Some(data);
                        break;
                    }
                    Err(e) => {
                        let err: crate::error::InstallationError = e;
                        scorer.record_failure(&ep.host, &err);
                    }
                }
            }

            let Some(data) = range_data else {
                // All endpoints failed for this range
                for &entry_idx in &range.entry_indices {
                    let artifact = &pending_artifacts[group.entries[entry_idx].artifact_idx];
                    failed += 1;
                    failed_files.push(FailedFile {
                        install_path: artifact.path.clone(),
                        encoding_key: hex::encode(artifact.encoding_key.as_bytes()),
                        category: FailureCategory::Network,
                        endpoint_host: None,
                    });
                }
                continue;
            };

            // Split coalesced response into individual entries and write each
            for &entry_idx in &range.entry_indices {
                let entry = &group.entries[entry_idx];
                let artifact = &pending_artifacts[entry.artifact_idx];
                let local_offset = (entry.offset - range.offset) as usize;
                let entry_size = entry.size as usize;

                if local_offset + entry_size > data.len() {
                    warn!(
                        path = %artifact.path,
                        "entry extends past range data (offset={}, size={}, range_len={})",
                        local_offset, entry_size, data.len()
                    );
                    failed += 1;
                    failed_files.push(FailedFile {
                        install_path: artifact.path.clone(),
                        encoding_key: hex::encode(artifact.encoding_key.as_bytes()),
                        category: FailureCategory::HashMismatch,
                        endpoint_host: serving_host.clone(),
                    });
                    continue;
                }

                let entry_data = data[local_offset..local_offset + entry_size].to_vec();

                // Write to CASC or loose file
                let ekey_hex = hex::encode(artifact.encoding_key.as_bytes());
                let write_result: Result<(), crate::error::InstallationError> =
                    if artifact.is_loose_file {
                        on_file_written(artifact, &installation, Some(entry_data)).await;
                        Ok(())
                    } else {
                        installation
                            .write_raw_blte(entry_data)
                            .await
                            .map(|_| ())
                            .map_err(crate::error::InstallationError::from)
                    };

                match write_result {
                    Ok(()) => {
                        downloaded += 1;
                        bytes_downloaded += u64::from(entry.size);
                        checkpoint.mark_completed(ekey_hex);

                        progress(ProgressEvent::FileComplete {
                            path: artifact.path.clone(),
                            bytes: u64::from(entry.size),
                        });
                    }
                    Err(e) => {
                        warn!(path = %artifact.path, error = %e, "failed to write file");
                        failed += 1;
                        failed_files.push(FailedFile {
                            install_path: artifact.path.clone(),
                            encoding_key: ekey_hex,
                            category: FailureCategory::Storage,
                            endpoint_host: serving_host.clone(),
                        });
                    }
                }
            }
        }

        // Checkpoint after each archive
        checkpoint.write(&config.install_path).await?;
        progress(ProgressEvent::CheckpointSaved {
            completed: checkpoint.completed_count(),
            remaining: checkpoint.remaining_count(),
        });
    }

    // Phase 2: Download loose/unmapped artifacts individually (same as before)
    if !loose_artifacts.is_empty() {
        info!(loose = loose_artifacts.len(), "downloading loose artifacts");

        for artifact in &loose_artifacts {
            let ekey_hex = hex::encode(artifact.encoding_key.as_bytes());
            if checkpoint.is_completed(&ekey_hex) {
                continue;
            }

            let ekey_bytes = artifact.encoding_key.as_bytes().to_vec();
            let outcome =
                fetch_artifact(Arc::clone(&cdn), sorted_eps.clone(), ekey_bytes, None).await;

            match outcome {
                FetchOutcome::Downloaded {
                    data,
                    score_events,
                    serving_host,
                } => {
                    for (host, event) in score_events {
                        match event {
                            ScoreEvent::Success => scorer.record_success(&host),
                            ScoreEvent::Failure(e) => scorer.record_failure(&host, &e),
                        }
                    }

                    let write_result: Result<(), crate::error::InstallationError> =
                        if artifact.is_loose_file {
                            on_file_written(artifact, &installation, Some(data)).await;
                            Ok(())
                        } else {
                            installation
                                .write_raw_blte(data)
                                .await
                                .map(|_| ())
                                .map_err(crate::error::InstallationError::from)
                        };

                    match write_result {
                        Ok(()) => {
                            downloaded += 1;
                            bytes_downloaded += u64::from(artifact.file_size);
                            checkpoint.mark_completed(ekey_hex);
                            progress(ProgressEvent::FileComplete {
                                path: artifact.path.clone(),
                                bytes: u64::from(artifact.file_size),
                            });
                        }
                        Err(e) => {
                            warn!(path = %artifact.path, error = %e, "failed to write loose file");
                            failed += 1;
                            failed_files.push(FailedFile {
                                install_path: artifact.path.clone(),
                                encoding_key: ekey_hex,
                                category: FailureCategory::Storage,
                                endpoint_host: serving_host,
                            });
                        }
                    }
                }
                FetchOutcome::Failed {
                    error_msg,
                    category,
                } => {
                    warn!(path = %artifact.path, error = %error_msg, "failed to download loose file");
                    failed += 1;
                    failed_files.push(FailedFile {
                        install_path: artifact.path.clone(),
                        encoding_key: ekey_hex,
                        category,
                        endpoint_host: None,
                    });
                }
            }
        }
    }

    info!(
        downloaded,
        failed,
        skipped,
        bytes = bytes_downloaded,
        "batched download phase complete"
    );

    Ok(DownloadReport {
        downloaded,
        failed,
        skipped,
        bytes_downloaded,
        failed_files,
    })
}

/// Download archive index files in parallel batches.
///
/// Fetches `.index` files from CDN and stores them in `Data/indices/`.
/// Skips indices that already exist on disk (resume support).
pub async fn download_archive_indices<S: CdnSource + 'static>(
    cdn: &Arc<S>,
    endpoints: &[CdnEndpoint],
    archive_keys: &[String],
    indices_dir: &Path,
    batch_size: usize,
    progress: &(impl Fn(ProgressEvent) + Send + Sync),
) -> InstallationResult<usize> {
    if endpoints.is_empty() {
        return Err(crate::error::InstallationError::InvalidConfig(
            "no CDN endpoints".to_string(),
        ));
    }

    let mut downloaded: usize = 0;
    let total = archive_keys.len();

    // Filter out already-existing indices (owned strings to avoid lifetime issues)
    let mut to_download: Vec<(usize, String)> = Vec::new();
    for (i, key) in archive_keys.iter().enumerate() {
        let index_path = indices_dir.join(format!("{key}.index"));
        if index_path.exists() {
            debug!(key = %key, "archive index already exists, skipping");
            continue;
        }
        to_download.push((i, key.clone()));
    }

    info!(
        total = total,
        to_download = to_download.len(),
        "downloading archive indices"
    );

    // Process in batches
    for batch_start in (0..to_download.len()).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(to_download.len());
        // Build owned futures for the batch to avoid lifetime issues with async_trait
        let endpoints_owned: Vec<CdnEndpoint> = endpoints.to_vec();
        let futures: Vec<_> = (batch_start..batch_end)
            .map(|i| {
                let (idx, ref key) = to_download[i];
                progress(ProgressEvent::ArchiveIndexDownloading { index: idx, total });
                let key = key.clone();
                let endpoints = endpoints_owned.clone();
                let cdn = Arc::clone(cdn);
                async move {
                    // Try each endpoint in order
                    let mut last_err = None;
                    for ep in &endpoints {
                        match cdn.download_archive_index(ep, &key).await {
                            Ok(data) => return Ok((key, data)),
                            Err(e) => {
                                last_err = Some(e);
                            }
                        }
                    }
                    Err(last_err.unwrap_or_else(|| {
                        crate::error::InstallationError::Cdn("no endpoints available".to_string())
                    }))
                }
            })
            .collect();

        // Download batch concurrently
        let results: Vec<InstallationResult<(String, Vec<u8>)>> = stream::iter(futures)
            .buffer_unordered(batch_size)
            .collect()
            .await;

        for result in results {
            match result {
                Ok((key, data)) => {
                    // Validate index before writing to disk
                    let cursor = std::io::Cursor::new(&data);
                    if let Err(e) = ArchiveIndex::parse(cursor) {
                        warn!(key = %key, error = %e, "downloaded archive index failed validation, skipping");
                        continue;
                    }
                    let index_path = indices_dir.join(format!("{key}.index"));
                    tokio::fs::write(&index_path, &data).await?;
                    downloaded += 1;
                    progress(ProgressEvent::ArchiveIndexComplete { archive_key: key });
                }
                Err(e) => {
                    warn!(error = %e, "failed to download archive index");
                }
            }
        }
    }

    Ok(downloaded)
}

/// Collect locally known encoding keys from an existing checkpoint.
#[must_use]
pub fn collect_known_keys(checkpoint: &Option<Checkpoint>) -> HashSet<String> {
    checkpoint
        .as_ref()
        .map(|cp| cp.completed_keys.clone())
        .unwrap_or_default()
}
