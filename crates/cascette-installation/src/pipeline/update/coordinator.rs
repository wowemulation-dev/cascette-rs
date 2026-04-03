//! Download coordinator with priority ordering and callbacks.
//!
//! Extends the base download executor with update-specific features:
//! priority-ordered processing, three-level callbacks matching agent.exe,
//! partial file resume, and pre-download leech checking.

use std::cmp::Ordering;
use std::sync::Arc;

use tracing::{debug, info, warn};

use cascette_client_storage::Installation;
use cascette_protocol::{CdnEndpoint, ContentType};

use crate::cdn_source::CdnSource;
use crate::checkpoint::Checkpoint;
use crate::config::UpdateConfig;
use crate::endpoint_scorer::EndpointScorer;
use crate::error::{InstallationError, InstallationResult};
use crate::patch::applicator;
use crate::pipeline::download::ArchiveLookup;
use crate::pipeline::update::PatchPlan;
use crate::pipeline::update::leech::AlternateSource;
use crate::progress::ProgressEvent;

/// Callbacks for download progress tracking.
///
/// Matches agent.exe's three download callbacks.
pub trait DownloadCallbacks: Send + Sync {
    /// Called as data arrives during a download.
    fn on_data_received(&self, ekey: &[u8; 16], bytes_received: u64, total_bytes: u64);
    /// Called when a BLTE block is fully written.
    fn on_block_complete(&self, ekey: &[u8; 16], block_index: u32, block_size: u32);
    /// Called when an entire file finishes downloading.
    fn on_file_complete(&self, ekey: &[u8; 16], total_bytes: u64);
}

/// No-op implementation of download callbacks.
pub struct NoopCallbacks;

impl DownloadCallbacks for NoopCallbacks {
    fn on_data_received(&self, _ekey: &[u8; 16], _bytes_received: u64, _total_bytes: u64) {}
    fn on_block_complete(&self, _ekey: &[u8; 16], _block_index: u32, _block_size: u32) {}
    fn on_file_complete(&self, _ekey: &[u8; 16], _total_bytes: u64) {}
}

/// Download priority levels.
///
/// Lower value = higher priority (processed first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DownloadPriority(pub i8);

impl DownloadPriority {
    /// Complete partial files first (closest to done).
    pub const PARTIAL: Self = Self(1);
    /// Patch-related files second.
    pub const PATCH: Self = Self(2);
    /// Regular downloads last.
    pub const NORMAL: Self = Self(3);
}

/// A single download request with priority and resume offset.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    /// Encoding key of the file.
    pub ekey: [u8; 16],
    /// File path or identifier.
    pub path: String,
    /// Total file size.
    pub size: u64,
    /// Download priority.
    pub priority: DownloadPriority,
    /// Byte offset for partial resume (>0 means resume from this point).
    pub offset: u64,
}

impl Eq for DownloadRequest {}

impl PartialEq for DownloadRequest {
    fn eq(&self, other: &Self) -> bool {
        self.ekey == other.ekey
    }
}

impl PartialOrd for DownloadRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DownloadRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower priority value = higher priority (processed first)
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.ekey.cmp(&other.ekey))
    }
}

/// Report from update download operations.
#[derive(Debug, Default)]
pub struct UpdateDownloadReport {
    /// Files downloaded from CDN.
    pub downloaded: usize,
    /// Files that failed to download.
    pub failed: usize,
    /// Files skipped (checkpoint or already present).
    pub skipped: usize,
    /// Total bytes downloaded from CDN.
    pub bytes_downloaded: u64,
    /// Number of downloads that required retry.
    pub retried: usize,
    /// Files leeched from alternate source.
    pub leech_count: usize,
    /// Bytes leeched from alternate source.
    pub leech_bytes: u64,
    /// Failed leech attempts.
    pub leech_failed: usize,
    /// Patches applied.
    pub patch_applied: usize,
    /// Patches that failed.
    pub patch_failed: usize,
    /// Structured failure details.
    pub failed_files: Vec<crate::pipeline::download::FailedFile>,
}

/// Execute downloads for update artifacts with priority ordering.
///
/// Processes downloads in priority order: partial > patch > normal.
/// For each request:
/// 1. Check checkpoint (skip if completed)
/// 2. If alternate source available, try leeching first
/// 3. If offset > 0, use range request for partial resume
/// 4. Otherwise, full download from CDN
/// 5. Invoke callbacks and save checkpoints at intervals
#[allow(clippy::too_many_arguments)]
pub async fn execute_update_downloads<S, F, Fut>(
    config: &UpdateConfig,
    cdn: Arc<S>,
    installation: Arc<Installation>,
    endpoints: &[CdnEndpoint],
    mut requests: Vec<DownloadRequest>,
    alternate: Option<&AlternateSource>,
    patch_plan: &PatchPlan,
    archive_lookup: &ArchiveLookup,
    callbacks: &impl DownloadCallbacks,
    checkpoint: &mut Checkpoint,
    progress: &(impl Fn(ProgressEvent) + Send + Sync),
    on_file_written: F,
) -> InstallationResult<UpdateDownloadReport>
where
    S: CdnSource + 'static,
    F: Fn(&[u8; 16], &Arc<Installation>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    if endpoints.is_empty() {
        return Err(InstallationError::InvalidConfig(
            "no CDN endpoints".to_string(),
        ));
    }

    // Sort by priority
    requests.sort();

    let mut report = UpdateDownloadReport::default();
    let mut scorer = EndpointScorer::new();
    let mut since_checkpoint: usize = 0;

    for request in &requests {
        let ekey_hex = hex::encode(request.ekey);

        // Skip if already completed
        if checkpoint.is_completed(&ekey_hex) {
            report.skipped += 1;
            continue;
        }

        progress(ProgressEvent::FileDownloading {
            path: request.path.clone(),
            size: request.size,
        });

        // Try leeching from alternate source first
        if let Some(alt) = alternate
            && alt.is_available(&request.ekey)
        {
            match alt.leech(&request.ekey, &installation).await? {
                super::leech::LeechResult::Copied { bytes } => {
                    debug!(path = %request.path, bytes, "leeched from alternate");
                    callbacks.on_file_complete(&request.ekey, bytes);
                    on_file_written(&request.ekey, &installation).await;
                    checkpoint.mark_completed(ekey_hex);
                    since_checkpoint += 1;
                    report.downloaded += 1;
                    report.bytes_downloaded += bytes;
                    report.leech_count += 1;
                    report.leech_bytes += bytes;

                    progress(ProgressEvent::FileLeeched {
                        path: request.path.clone(),
                        bytes,
                    });

                    if since_checkpoint >= config.checkpoint_interval {
                        checkpoint.write(&config.install_path).await?;
                        progress(ProgressEvent::CheckpointSaved {
                            completed: checkpoint.completed_count(),
                            remaining: checkpoint.remaining_count(),
                        });
                        since_checkpoint = 0;
                    }
                    continue;
                }
                super::leech::LeechResult::NotAvailable => {}
                super::leech::LeechResult::Failed { error } => {
                    report.leech_failed += 1;
                    progress(ProgressEvent::LeechFailed {
                        path: request.path.clone(),
                        error,
                    });
                    // Fall through to CDN download
                }
            }
        }

        // Try patch application if a chain exists for this file
        if let Some(chain) = patch_plan.chains.get(&request.ekey) {
            progress(ProgressEvent::PatchApplying {
                path: request.path.clone(),
            });

            // Read the base file from the installation
            let base_result = installation
                .read_file_by_encoding_key(&cascette_crypto::EncodingKey::from_bytes(
                    chain.start_key,
                ))
                .await;

            match base_result {
                Ok(base_data) => {
                    match applicator::apply_patch_chain(
                        chain,
                        base_data,
                        &patch_plan.resolver,
                        cdn.as_ref(),
                        endpoints,
                    )
                    .await
                    {
                        Ok(patched_data) => {
                            match installation.write_file(patched_data, false).await {
                                Ok(_) => {
                                    debug!(path = %request.path, "patch applied and written");
                                    callbacks.on_file_complete(&request.ekey, request.size);
                                    on_file_written(&request.ekey, &installation).await;
                                    report.downloaded += 1;
                                    report.bytes_downloaded += request.size;
                                    report.patch_applied += 1;
                                    checkpoint.mark_completed(ekey_hex);
                                    since_checkpoint += 1;

                                    progress(ProgressEvent::PatchApplied {
                                        path: request.path.clone(),
                                    });

                                    if since_checkpoint >= config.checkpoint_interval {
                                        checkpoint.write(&config.install_path).await?;
                                        progress(ProgressEvent::CheckpointSaved {
                                            completed: checkpoint.completed_count(),
                                            remaining: checkpoint.remaining_count(),
                                        });
                                        since_checkpoint = 0;
                                    }
                                    continue;
                                }
                                Err(e) => {
                                    report.patch_failed += 1;
                                    warn!(path = %request.path, error = %e, "failed to write patched file");
                                }
                            }
                        }
                        Err(e) => {
                            report.patch_failed += 1;
                            warn!(path = %request.path, error = %e, "patch application failed");
                            progress(ProgressEvent::PatchFailed {
                                path: request.path.clone(),
                                error: e.to_string(),
                            });
                            // Fall through to CDN download
                        }
                    }
                }
                Err(e) => {
                    report.patch_failed += 1;
                    warn!(path = %request.path, error = %e, "failed to read base file for patching");
                    progress(ProgressEvent::PatchFailed {
                        path: request.path.clone(),
                        error: e.to_string(),
                    });
                    // Fall through to CDN download
                }
            }
        }

        // Sort endpoints by adaptive score (healthiest first)
        let sorted_eps = scorer.sort_endpoints(endpoints)?;

        // Download from CDN (with range support for partial resume),
        // trying each endpoint in score order until one succeeds.
        // Track the serving host for hash-failure attribution.
        let mut download_result = Err(InstallationError::Cdn("no endpoints available".to_string()));
        let mut serving_host: Option<String> = None;
        for ep in &sorted_eps {
            let result = if request.offset > 0 {
                let remaining = request.size.saturating_sub(request.offset);
                cdn.download_range(
                    ep,
                    ContentType::Data,
                    &request.ekey,
                    request.offset,
                    remaining,
                )
                .await
            } else {
                cdn.download(ep, ContentType::Data, &request.ekey).await
            };
            match result {
                Ok(data) => {
                    scorer.record_success(&ep.host);
                    serving_host = Some(ep.host.clone());
                    download_result = Ok(data);
                    break;
                }
                Err(e) => {
                    debug!(host = %ep.host, error = %e, "loose blob failed, trying next endpoint");
                    scorer.record_failure(&ep.host, &e);
                    download_result = Err(e);
                }
            }
        }

        // If loose blob failed, try archive byte-range download
        if download_result.is_err() {
            let ekey_vec = request.ekey.to_vec();
            if let Some(loc) = archive_lookup.get(&ekey_vec) {
                debug!(
                    path = %request.path,
                    archive = %hex::encode(&loc.archive_key),
                    offset = loc.offset,
                    size = loc.size,
                    "falling back to archive range download"
                );
                // Re-sort: scores may have changed from loose blob failures
                let sorted_eps = scorer.sort_endpoints(endpoints)?;
                for ep in &sorted_eps {
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
                            scorer.record_success(&ep.host);
                            serving_host = Some(ep.host.clone());
                            download_result = Ok(data);
                            break;
                        }
                        Err(e) => {
                            debug!(
                                host = %ep.host,
                                error = %e,
                                "archive range download failed, trying next endpoint"
                            );
                            scorer.record_failure(&ep.host, &e);
                            download_result = Err(e);
                        }
                    }
                }
            }
        }

        match download_result {
            Ok(data) => {
                let bytes = data.len() as u64;
                callbacks.on_data_received(&request.ekey, bytes, request.size);

                match installation.write_raw_blte(data).await {
                    Ok(_) => {
                        debug!(path = %request.path, "file written to CASC storage");
                        callbacks.on_file_complete(&request.ekey, bytes);
                        on_file_written(&request.ekey, &installation).await;
                        report.downloaded += 1;
                        report.bytes_downloaded += bytes;
                        checkpoint.mark_completed(ekey_hex);
                        since_checkpoint += 1;

                        progress(ProgressEvent::FileComplete {
                            path: request.path.clone(),
                        });
                    }
                    Err(e) => {
                        warn!(path = %request.path, error = %e, "failed to write file");
                        let error_msg = e.to_string();
                        let inst_err = InstallationError::from(e);
                        let category = crate::pipeline::download::categorize_error(&inst_err);
                        if category == crate::pipeline::download::FailureCategory::HashMismatch
                            && let Some(ref host) = serving_host
                        {
                            scorer.record_data_corruption(host);
                        }
                        report.failed += 1;
                        report
                            .failed_files
                            .push(crate::pipeline::download::FailedFile {
                                install_path: request.path.clone(),
                                encoding_key: ekey_hex.clone(),
                                category,
                                endpoint_host: serving_host.clone(),
                            });
                        progress(ProgressEvent::FileFailed {
                            path: request.path.clone(),
                            error: error_msg,
                        });
                    }
                }
            }
            Err(e) => {
                warn!(path = %request.path, error = %e, "failed to download file");
                let category = crate::pipeline::download::categorize_error(&e);
                report.failed += 1;
                report
                    .failed_files
                    .push(crate::pipeline::download::FailedFile {
                        install_path: request.path.clone(),
                        encoding_key: ekey_hex.clone(),
                        category,
                        endpoint_host: None,
                    });
                progress(ProgressEvent::FileFailed {
                    path: request.path.clone(),
                    error: e.to_string(),
                });
            }
        }

        // Checkpoint save at intervals
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
        downloaded = report.downloaded,
        failed = report.failed,
        skipped = report.skipped,
        bytes = report.bytes_downloaded,
        "update download phase complete"
    );

    Ok(report)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(DownloadPriority::PARTIAL < DownloadPriority::PATCH);
        assert!(DownloadPriority::PATCH < DownloadPriority::NORMAL);
    }

    #[test]
    fn test_download_request_ordering() {
        let req_partial = DownloadRequest {
            ekey: [0xAA; 16],
            path: "partial.dat".to_string(),
            size: 100,
            priority: DownloadPriority::PARTIAL,
            offset: 50,
        };
        let req_patch = DownloadRequest {
            ekey: [0xBB; 16],
            path: "patch.dat".to_string(),
            size: 200,
            priority: DownloadPriority::PATCH,
            offset: 0,
        };
        let req_normal = DownloadRequest {
            ekey: [0xCC; 16],
            path: "normal.dat".to_string(),
            size: 300,
            priority: DownloadPriority::NORMAL,
            offset: 0,
        };

        let mut requests = [req_normal, req_partial, req_patch];
        requests.sort();

        assert_eq!(requests[0].priority, DownloadPriority::PARTIAL);
        assert_eq!(requests[1].priority, DownloadPriority::PATCH);
        assert_eq!(requests[2].priority, DownloadPriority::NORMAL);
    }
}
