//! Build update pipeline: transitions an existing CASC installation between versions.
//!
//! Unlike the install pipeline (which populates a fresh directory), the update
//! pipeline operates on an existing installation. It classifies files into
//! required/partial/inflight categories, supports alternate container leeching,
//! patch application, and priority-ordered downloads.

pub mod classify;
pub mod coordinator;
pub mod leech;
pub mod patch_resolution;

pub use super::loose;

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, info, warn};

use cascette_client_storage::Installation;
use cascette_client_storage::container::AccessMode;
use cascette_client_storage::container::residency::ResidencyContainer;
use cascette_formats::patch_chain::PatchChain;
use cascette_protocol::CdnEndpoint;

use crate::cdn_source::CdnSource;
use crate::checkpoint::Checkpoint;
use crate::config::{InstallConfig, UpdateConfig};
use crate::error::{InstallationError, InstallationResult};
use crate::layout;
use crate::patch::resolver::PatchResolver;
use crate::pipeline::download;
use crate::pipeline::manifests::BuildManifests;
use crate::pipeline::update::classify::UpdateArtifactSet;
use crate::pipeline::update::coordinator::{
    DownloadPriority, DownloadRequest, NoopCallbacks, UpdateDownloadReport,
};
use crate::pipeline::update::leech::AlternateSource;
use crate::progress::ProgressEvent;

/// Resolved patch application plan.
#[derive(Debug)]
pub struct PatchPlan {
    /// Chains to apply, keyed by target encoding key.
    pub chains: HashMap<[u8; 16], PatchChain>,
    /// Patch data resolver for locating patch blobs.
    pub resolver: PatchResolver,
}

/// Report from a completed update operation.
#[derive(Debug)]
pub struct UpdateReport {
    /// Files that needed downloading.
    pub missing_files: usize,
    /// Total bytes of missing files.
    pub missing_bytes: u64,
    /// Total bytes written to storage.
    pub written_bytes: u64,
    /// Files leeched from alternate source.
    pub leech_count: usize,
    /// Bytes leeched from alternate source.
    pub leech_bytes: u64,
    /// Failed leech attempts.
    pub leech_failed: usize,
    /// Bytes downloaded from CDN.
    pub downloaded_bytes: u64,
    /// Downloads that required retry.
    pub retried: usize,
    /// Files that had patch chains.
    pub patchable: usize,
    /// Patches applied.
    pub patch_applied: usize,
    /// Patches that failed.
    pub patch_failed: usize,
    /// Archive indices downloaded.
    pub indices_downloaded: usize,
    /// Obsolete files removed.
    pub obsolete_removed: usize,
    /// Structured failure details.
    pub failed_files: Vec<crate::pipeline::download::FailedFile>,
}

/// Pipeline state machine states.
enum UpdateState {
    /// Validate prerequisites: encoding table, download manifest present.
    ValidatingPrerequisites,
    /// Fetch base + target build configs and manifests.
    FetchingConfigs,
    /// Classify artifacts into required/partial/inflight.
    ClassifyingArtifacts {
        base_manifests: BuildManifests,
        target_manifests: BuildManifests,
    },
    /// Process patch index, build patch chains, init patcher.
    ProcessingPatches {
        base_manifests: BuildManifests,
        target_manifests: BuildManifests,
        artifact_set: UpdateArtifactSet,
        obsolete: Vec<classify::ObsoleteFile>,
        checkpoint: Option<Box<Checkpoint>>,
    },
    /// Fetch archive indices for new archives.
    FetchingArchiveIndices {
        target_manifests: BuildManifests,
        artifact_set: UpdateArtifactSet,
        patch_plan: PatchPlan,
        obsolete: Vec<classify::ObsoleteFile>,
        checkpoint: Option<Box<Checkpoint>>,
    },
    /// Download required files, complete partials, apply patches, leech.
    Downloading {
        target_manifests: BuildManifests,
        artifact_set: UpdateArtifactSet,
        patch_plan: PatchPlan,
        indices_downloaded: usize,
        archive_lookup: download::ArchiveLookup,
        obsolete: Vec<classify::ObsoleteFile>,
        checkpoint: Option<Box<Checkpoint>>,
    },
    /// Write updated layout files (.build.info, configs).
    WritingLayout {
        target_manifests: BuildManifests,
        download_report: UpdateDownloadReport,
        patch_plan: PatchPlan,
        indices_downloaded: usize,
        obsolete: Vec<classify::ObsoleteFile>,
    },
    /// Update complete.
    Complete { report: UpdateReport },
}

/// The update pipeline.
///
/// Transitions an existing CASC installation from one build version to another.
/// Reuses the existing install manifest classification, CDN download, and layout
/// writing infrastructure while adding update-specific classification, leeching,
/// and patch application.
pub struct UpdatePipeline {
    config: UpdateConfig,
}

impl UpdatePipeline {
    /// Create a new update pipeline.
    #[must_use]
    pub fn new(config: UpdateConfig) -> Self {
        Self { config }
    }

    /// Run the full update pipeline.
    ///
    /// Progresses through states: ValidatingPrerequisites -> FetchingConfigs ->
    /// ClassifyingArtifacts -> ProcessingPatches -> FetchingArchiveIndices ->
    /// Downloading -> WritingLayout -> Complete.
    pub async fn run<S: CdnSource + 'static>(
        self,
        cdn: Arc<S>,
        endpoints: Vec<CdnEndpoint>,
        installation: Arc<Installation>,
        progress: impl Fn(ProgressEvent) + Send + Sync,
    ) -> InstallationResult<UpdateReport> {
        let mut state = UpdateState::ValidatingPrerequisites;

        loop {
            state = match state {
                UpdateState::ValidatingPrerequisites => {
                    info!("validating update prerequisites");

                    let data_dir = self.config.install_path.join("Data");
                    if !data_dir.exists() {
                        return Err(InstallationError::InvalidConfig(format!(
                            "installation data directory does not exist: {}",
                            data_dir.display()
                        )));
                    }

                    let data_data_dir = data_dir.join("data");
                    if !data_data_dir.exists() {
                        return Err(InstallationError::InvalidConfig(format!(
                            "CASC data directory does not exist: {}",
                            data_data_dir.display()
                        )));
                    }

                    UpdateState::FetchingConfigs
                }

                UpdateState::FetchingConfigs => {
                    info!("fetching base and target manifests");

                    // Resolve base manifests
                    let base_install_config = self.to_install_config(
                        &self.config.base_build_config,
                        &self.config.base_cdn_config,
                    );
                    let base_manifests = crate::pipeline::metadata::resolve_manifests(
                        &base_install_config,
                        cdn.as_ref(),
                        &endpoints,
                        &progress,
                    )
                    .await?;

                    // Resolve target manifests
                    let target_install_config = self.to_install_config(
                        &self.config.target_build_config,
                        &self.config.target_cdn_config,
                    );
                    let target_manifests = crate::pipeline::metadata::resolve_manifests(
                        &target_install_config,
                        cdn.as_ref(),
                        &endpoints,
                        &progress,
                    )
                    .await?;

                    UpdateState::ClassifyingArtifacts {
                        base_manifests,
                        target_manifests,
                    }
                }

                UpdateState::ClassifyingArtifacts {
                    base_manifests,
                    target_manifests,
                } => {
                    info!("classifying update artifacts");

                    // Load checkpoint early so completed keys can be filtered
                    // during classification rather than only during download.
                    let checkpoint = if self.config.resume {
                        Checkpoint::read(&self.config.install_path).await?
                    } else {
                        None
                    };
                    let known_keys = download::collect_known_keys(&checkpoint);

                    let data_path = self.config.install_path.join("Data").join("data");
                    let mut residency = ResidencyContainer::new(
                        self.config.product.clone(),
                        AccessMode::ReadOnly,
                        data_path,
                    );
                    residency.initialize().await.map_err(|e| {
                        InstallationError::UpdatePrerequisite(format!(
                            "failed to initialize residency container: {e}"
                        ))
                    })?;

                    // Open alternate source if configured
                    let alternate = if let Some(ref alt_path) = self.config.alternate_install_path {
                        match AlternateSource::open(alt_path.clone()).await {
                            Ok(alt) => Some(alt),
                            Err(e) => {
                                warn!(
                                    "failed to fetch alternate container state for e-key set, \
                                     leeching disabled: {e}"
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };

                    let tags: Vec<&str> = self
                        .config
                        .platform_tags
                        .iter()
                        .map(String::as_str)
                        .collect();

                    // Build empty patch chains map for initial classification
                    // (real chains are built in ProcessingPatches)
                    let patch_chains: HashMap<[u8; 16], PatchChain> = HashMap::new();

                    // A previous session was interrupted if a checkpoint exists
                    // with fewer completed keys than its total artifact count.
                    let is_resuming = checkpoint
                        .as_ref()
                        .is_some_and(|cp| cp.completed_count() < cp.total_artifacts);

                    let mut artifact_set = classify::classify_update_artifacts(
                        &target_manifests.install,
                        &target_manifests.encoding,
                        &target_manifests.download,
                        &tags,
                        &residency,
                        alternate.as_ref().map(AlternateSource::residency),
                        &patch_chains,
                        &known_keys,
                        is_resuming,
                    )?;

                    // Detect files present in the base build but absent from the
                    // target build (agent.exe status 6, FilterLooseFiles state 9).
                    artifact_set.obsolete = classify::detect_obsolete_files(
                        &base_manifests.install,
                        &target_manifests.install,
                        &tags,
                    );

                    if !artifact_set.obsolete.is_empty() {
                        info!(
                            count = artifact_set.obsolete.len(),
                            "detected obsolete files to remove"
                        );
                    }

                    progress(ProgressEvent::UpdateClassified {
                        required: artifact_set.required.len(),
                        partial: artifact_set.partial.len(),
                        inflight: artifact_set.inflight.len(),
                        leechable: artifact_set.leechable.len(),
                        total_bytes: artifact_set.total_download_bytes(),
                    });

                    info!(
                        required = artifact_set.required.len(),
                        partial = artifact_set.partial.len(),
                        inflight = artifact_set.inflight.len(),
                        leechable = artifact_set.leechable.len(),
                        present = artifact_set.already_present,
                        unresolved = artifact_set.unresolved,
                        bytes = artifact_set.total_download_bytes(),
                        "update artifact classification complete"
                    );

                    let obsolete = std::mem::take(&mut artifact_set.obsolete);

                    UpdateState::ProcessingPatches {
                        base_manifests,
                        target_manifests,
                        artifact_set,
                        obsolete,
                        checkpoint: checkpoint.map(Box::new),
                    }
                }

                UpdateState::ProcessingPatches {
                    base_manifests,
                    target_manifests,
                    artifact_set,
                    obsolete,
                    checkpoint,
                } => {
                    let patch_plan = if self.config.enable_patching
                        && target_manifests.build_config.patch_config().is_some()
                    {
                        info!("processing patch configuration");

                        // Build residency container for checking local file presence
                        let data_path = self.config.install_path.join("Data").join("data");
                        let mut residency = ResidencyContainer::new(
                            self.config.product.clone(),
                            AccessMode::ReadOnly,
                            data_path,
                        );
                        if let Err(e) = residency.initialize().await {
                            warn!("failed to init residency for patch resolution: {e}");
                            PatchPlan {
                                chains: HashMap::new(),
                                resolver: PatchResolver::new(vec![], vec![]),
                            }
                        } else {
                            match patch_resolution::resolve_patch_plan(
                                &base_manifests,
                                &target_manifests,
                                cdn.as_ref(),
                                &endpoints,
                                &residency,
                            )
                            .await
                            {
                                Ok(plan) => plan,
                                Err(e) => {
                                    warn!(
                                        "patch resolution failed, proceeding without patches: {e}"
                                    );
                                    PatchPlan {
                                        chains: HashMap::new(),
                                        resolver: PatchResolver::new(vec![], vec![]),
                                    }
                                }
                            }
                        }
                    } else {
                        debug!("patching disabled or no patch config present");
                        PatchPlan {
                            chains: HashMap::new(),
                            resolver: PatchResolver::new(vec![], vec![]),
                        }
                    };

                    UpdateState::FetchingArchiveIndices {
                        target_manifests,
                        artifact_set,
                        patch_plan,
                        obsolete,
                        checkpoint,
                    }
                }

                UpdateState::FetchingArchiveIndices {
                    target_manifests,
                    artifact_set,
                    patch_plan,
                    obsolete,
                    checkpoint,
                } => {
                    let indices_dir = self.config.install_path.join("Data").join("indices");
                    tokio::fs::create_dir_all(&indices_dir).await?;

                    if endpoints.is_empty() {
                        return Err(InstallationError::InvalidConfig("no endpoints".to_string()));
                    }

                    let archive_keys: Vec<String> = target_manifests
                        .cdn_config
                        .archives()
                        .iter()
                        .map(|a| a.content_key.clone())
                        .collect();

                    let indices_downloaded = download::download_archive_indices(
                        &cdn,
                        &endpoints,
                        &archive_keys,
                        &indices_dir,
                        self.config.index_batch_size,
                        &progress,
                    )
                    .await?;

                    let archive_lookup =
                        download::load_archive_indices(&indices_dir, &archive_keys)?;

                    UpdateState::Downloading {
                        target_manifests,
                        artifact_set,
                        patch_plan,
                        indices_downloaded,
                        archive_lookup,
                        obsolete,
                        checkpoint,
                    }
                }

                UpdateState::Downloading {
                    target_manifests,
                    artifact_set,
                    patch_plan,
                    indices_downloaded,
                    archive_lookup,
                    obsolete,
                    checkpoint,
                } => {
                    // Build download requests from artifact set
                    let mut requests = Vec::new();

                    // Partial files first (resume)
                    for artifact in &artifact_set.partial {
                        requests.push(DownloadRequest {
                            ekey: *artifact.descriptor.encoding_key.as_bytes(),
                            path: artifact.descriptor.path.clone(),
                            size: u64::from(artifact.descriptor.file_size),
                            priority: DownloadPriority::PARTIAL,
                            offset: artifact.bytes_present,
                        });
                    }

                    // Inflight / patchable files
                    for artifact in &artifact_set.inflight {
                        requests.push(DownloadRequest {
                            ekey: *artifact.descriptor.encoding_key.as_bytes(),
                            path: artifact.descriptor.path.clone(),
                            size: u64::from(artifact.descriptor.file_size),
                            priority: DownloadPriority::PATCH,
                            offset: 0,
                        });
                    }

                    // Required files (normal priority)
                    for artifact in &artifact_set.required {
                        requests.push(DownloadRequest {
                            ekey: *artifact.descriptor.encoding_key.as_bytes(),
                            path: artifact.descriptor.path.clone(),
                            size: u64::from(artifact.descriptor.file_size),
                            priority: DownloadPriority::NORMAL,
                            offset: 0,
                        });
                    }

                    // Open alternate source for leeching during download
                    let alternate = if let Some(ref alt_path) = self.config.alternate_install_path {
                        AlternateSource::open(alt_path.clone()).await.ok()
                    } else {
                        None
                    };

                    let total_artifacts = requests.len();
                    let mut checkpoint_state = Checkpoint::new(
                        self.config.product.clone(),
                        self.config.target_build_config.clone(),
                        self.config.target_cdn_config.clone(),
                        total_artifacts,
                    );

                    // Merge checkpoint loaded during classification
                    if let Some(existing) = checkpoint {
                        checkpoint_state.completed_keys = existing.completed_keys;
                    }

                    // Build encoding key -> file path map from target install manifest
                    let install_manifest_paths: HashMap<[u8; 16], String> = artifact_set
                        .required
                        .iter()
                        .chain(artifact_set.partial.iter())
                        .chain(artifact_set.inflight.iter())
                        .map(|a| {
                            (
                                *a.descriptor.encoding_key.as_bytes(),
                                a.descriptor.path.clone(),
                            )
                        })
                        .collect();

                    // Initialize loose file handler if subfolder is configured
                    let loose_handler = match &self.config.game_subfolder {
                        Some(subfolder) => {
                            let handler = loose::LooseFileHandler::new(
                                subfolder.clone(),
                                self.config.install_path.clone(),
                            )?;
                            Some(Arc::new(tokio::sync::Mutex::new(handler)))
                        }
                        None => None,
                    };

                    let handler_ref = loose_handler.clone();
                    let key_store_ref = self.config.key_store.clone();
                    let on_file_written = |ekey: &[u8; 16], inst: &Arc<Installation>| {
                        let handler = handler_ref.clone();
                        let ekey = *ekey;
                        let path = install_manifest_paths.get(&ekey).cloned();
                        let inst = Arc::clone(inst);
                        let ks = key_store_ref.clone();
                        async move {
                            if let (Some(handler), Some(file_path)) = (handler, path) {
                                let mut h = handler.lock().await;
                                let key_ref = ks.as_ref().map(|k| {
                                    &**k as &(dyn cascette_crypto::TactKeyProvider + Send + Sync)
                                });
                                if let Err(e) =
                                    h.on_file_complete(&ekey, &file_path, &inst, key_ref).await
                                {
                                    warn!(
                                        path = %file_path,
                                        error = %e,
                                        "loose file placement failed"
                                    );
                                }
                            }
                        }
                    };

                    let download_report = coordinator::execute_update_downloads(
                        &self.config,
                        cdn.clone(),
                        installation.clone(),
                        &endpoints,
                        requests,
                        alternate.as_ref(),
                        &patch_plan,
                        &archive_lookup,
                        &NoopCallbacks,
                        &mut checkpoint_state,
                        &progress,
                        on_file_written,
                    )
                    .await?;

                    UpdateState::WritingLayout {
                        target_manifests,
                        download_report,
                        patch_plan,
                        indices_downloaded,
                        obsolete,
                    }
                }

                UpdateState::WritingLayout {
                    target_manifests,
                    download_report,
                    patch_plan,
                    indices_downloaded,
                    obsolete,
                } => {
                    // Remove obsolete files before writing the new layout.
                    // Matches agent.exe Phase 7: "Failed to remove obsolete loose file %s"
                    let mut obsolete_removed: usize = 0;
                    for file in &obsolete {
                        let file_path = self.config.install_path.join(&file.path);
                        if file_path.exists() {
                            match tokio::fs::remove_file(&file_path).await {
                                Ok(()) => {
                                    debug!(path = %file.path, "removed obsolete file");
                                    obsolete_removed += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        path = %file.path,
                                        error = %e,
                                        "failed to remove obsolete file"
                                    );
                                }
                            }
                        }
                    }

                    if obsolete_removed > 0 {
                        info!(removed = obsolete_removed, "obsolete file cleanup complete");
                    }

                    // Build an InstallConfig for layout writing with target hashes
                    let layout_config = self.to_install_config(
                        &self.config.target_build_config,
                        &self.config.target_cdn_config,
                    );
                    layout::write_layout(&layout_config, &target_manifests).await?;

                    // Clear checkpoint on success
                    Checkpoint::clear(&self.config.install_path).await?;

                    UpdateState::Complete {
                        report: UpdateReport {
                            missing_files: download_report.downloaded
                                + download_report.failed
                                + download_report.skipped,
                            missing_bytes: download_report.bytes_downloaded,
                            written_bytes: download_report.bytes_downloaded,
                            leech_count: download_report.leech_count,
                            leech_bytes: download_report.leech_bytes,
                            leech_failed: download_report.leech_failed,
                            downloaded_bytes: download_report.bytes_downloaded,
                            retried: download_report.retried,
                            patchable: patch_plan.chains.len(),
                            patch_applied: download_report.patch_applied,
                            patch_failed: download_report.patch_failed,
                            indices_downloaded,
                            obsolete_removed,
                            failed_files: download_report.failed_files,
                        },
                    }
                }

                UpdateState::Complete { report } => return Ok(report),
            };
        }
    }

    /// Build an `InstallConfig` from this update config for reuse with
    /// metadata resolution and layout writing.
    fn to_install_config(&self, build_config: &str, cdn_config: &str) -> InstallConfig {
        let mut config = InstallConfig::new(
            self.config.product.clone(),
            self.config.install_path.clone(),
            self.config.cdn_path.clone(),
        );
        config.region.clone_from(&self.config.region);
        config.platform_tags.clone_from(&self.config.platform_tags);
        config.locale.clone_from(&self.config.locale);
        config.build_config = Some(build_config.to_string());
        config.cdn_config = Some(cdn_config.to_string());
        config.endpoints.clone_from(&self.config.endpoints);
        config.max_connections_per_host = self.config.max_connections_per_host;
        config.max_connections_global = self.config.max_connections_global;
        config.index_batch_size = self.config.index_batch_size;
        config.checkpoint_interval = self.config.checkpoint_interval;
        config.resume = self.config.resume;
        config
            .game_subfolder
            .clone_from(&self.config.game_subfolder);
        config.key_store.clone_from(&self.config.key_store);
        config
    }
}
