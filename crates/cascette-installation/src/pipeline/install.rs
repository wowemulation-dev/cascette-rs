//! Install pipeline: orchestrates the full CASC installation from CDN.

use cascette_crypto::ContentKey;
use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

use cascette_client_storage::Installation;
use cascette_protocol::CdnEndpoint;

use crate::cdn_source::CdnSource;
use crate::checkpoint::Checkpoint;
use crate::config::InstallConfig;
use crate::error::{InstallationError, InstallationResult};
use crate::layout;
use crate::pipeline::classify;
use crate::pipeline::download;
use crate::pipeline::loose;
use crate::pipeline::manifests::BuildManifests;
use crate::pipeline::metadata;
use crate::pipeline::root_content;
use crate::progress::ProgressEvent;

/// Report from a completed installation.
#[derive(Debug)]
pub struct InstallReport {
    /// Number of files downloaded.
    pub downloaded: usize,
    /// Number of files that failed.
    pub failed: usize,
    /// Number of files skipped (already present).
    pub skipped: usize,
    /// Total bytes downloaded.
    pub bytes_downloaded: u64,
    /// Number of archive indices downloaded.
    pub indices_downloaded: usize,
    /// Number of loose files placed in product subfolder.
    pub loose_files_placed: usize,
    /// Structured failure details.
    pub failed_files: Vec<super::download::FailedFile>,
}

/// Mapping from encoding key bytes to install manifest file path.
/// Used to identify which downloaded files need loose file placement.
/// Maps an encoding key to every install-manifest path that references it.
///
/// Multiple manifest entries can share one encoding key when their content
/// is identical (e.g. an `Info.plist` file deduplicated across N locale
/// subdirectories inside a macOS .app bundle). Each path must still be
/// materialized on disk to match Agent.exe's output, so the value is a
/// `Vec` rather than a single `String`.
type InstallManifestPaths = HashMap<[u8; 16], Vec<String>>;

/// Pipeline state machine states.
enum PipelineState {
    /// Fetching build and CDN configs.
    FetchingConfigs,
    /// Classifying artifacts from manifests.
    ClassifyingArtifacts { manifests: BuildManifests },
    /// Downloading archive indices.
    FetchingArchiveIndices {
        manifests: BuildManifests,
        install_artifacts: classify::ArtifactSet,
        download_artifacts: classify::ArtifactSet,
        install_manifest_paths: InstallManifestPaths,
    },
    /// Downloading artifact data (loose files first, then CASC blobs).
    Downloading {
        manifests: BuildManifests,
        install_artifacts: classify::ArtifactSet,
        download_artifacts: classify::ArtifactSet,
        indices_downloaded: usize,
        archive_lookup: download::ArchiveLookup,
        install_manifest_paths: InstallManifestPaths,
    },
    /// Writing Battle.net directory layout.
    WritingLayout {
        manifests: BuildManifests,
        download_report: download::DownloadReport,
        indices_downloaded: usize,
        loose_report: loose::LooseFileReport,
    },
    /// Installation complete.
    Complete { report: InstallReport },
}

/// The install pipeline.
///
/// Populates CASC storage (`Data/data/`, `Data/indices/`, `Data/config/`,
/// `.build.info`) from CDN.
pub struct InstallPipeline {
    config: InstallConfig,
}

impl InstallPipeline {
    /// Create a new install pipeline.
    #[must_use]
    pub fn new(config: InstallConfig) -> Self {
        Self { config }
    }

    /// Resolve manifests without running the full pipeline.
    ///
    /// This is the integration point for `cascette-maintenance` to get
    /// manifest data for preservation set building.
    pub async fn resolve_manifests<S: CdnSource>(
        &self,
        cdn: &S,
        endpoints: &[CdnEndpoint],
    ) -> InstallationResult<BuildManifests> {
        metadata::resolve_manifests(&self.config, cdn, endpoints, &|_| {}).await
    }

    /// Run the full install pipeline.
    ///
    /// Progresses through states: FetchingConfigs -> ClassifyingArtifacts ->
    /// FetchingArchiveIndices -> Downloading -> WritingLayout -> Complete.
    pub async fn run<S: CdnSource + 'static>(
        self,
        cdn: Arc<S>,
        endpoints: Vec<CdnEndpoint>,
        progress: impl Fn(ProgressEvent) + Send + Sync,
    ) -> InstallationResult<InstallReport> {
        // Loose-only mode: skip CASC writes entirely. Fetch only the install
        // manifest, download the loose files (Wow.exe, DLLs, locale packs),
        // write layout metadata, and return. The wow client will bootstrap
        // CASC content from the CDN on first launch.
        if self.config.mode == crate::config::InstallMode::LooseOnly {
            return super::install_loose::run(self.config, cdn, endpoints, progress).await;
        }

        let mut state = PipelineState::FetchingConfigs;

        loop {
            state = match state {
                PipelineState::FetchingConfigs => {
                    let manifests = metadata::resolve_manifests(
                        &self.config,
                        cdn.as_ref(),
                        &endpoints,
                        &progress,
                    )
                    .await?;
                    PipelineState::ClassifyingArtifacts { manifests }
                }

                PipelineState::ClassifyingArtifacts { manifests } => {
                    // Load checkpoint if resuming
                    let checkpoint = if self.config.resume {
                        Checkpoint::read(&self.config.install_path).await?
                    } else {
                        None
                    };

                    let known_keys = download::collect_known_keys(&checkpoint);

                    let tag_names = self.config.tag_query.tag_names();
                    let tags: Vec<&str> = tag_names.iter().map(String::as_str).collect();

                    info!(
                        tags = ?tags,
                        product = %self.config.product,
                        region = %self.config.region,
                        locale = %self.config.tag_query.locale,
                        build_info_tags = %self.config.tag_query.build_info_tags(),
                        "classifying artifacts with tag query"
                    );

                    // Phase 1: Install manifest entries (filesystem files)
                    let install_artifacts = classify::classify_artifacts(
                        &manifests.install,
                        &manifests.encoding,
                        &manifests.download,
                        &tags,
                        &known_keys,
                    )?;

                    info!(
                        install_total_entries = manifests.install.entries.len(),
                        install_tag_matched = install_artifacts.tag_matched,
                        install_required = install_artifacts.required.len(),
                        install_present = install_artifacts.already_present,
                        install_unresolved = install_artifacts.unresolved,
                        install_bytes = install_artifacts.total_download_bytes(),
                        install_tags = manifests.install.tags.len(),
                        "install manifest classification complete"
                    );

                    // Phase 2: Download manifest entries (CASC archive data).
                    // Backfill promotes all remaining files to highest priority
                    // to prioritise partially-downloaded files.
                    let download_artifacts = if self.config.backfill_mode {
                        classify::classify_backfill_artifacts(
                            &manifests.download,
                            &tags,
                            &known_keys,
                        )?
                    } else {
                        classify::classify_download_artifacts(
                            &manifests.download,
                            &tags,
                            &known_keys,
                        )?
                    };

                    info!(
                        download_total_entries = manifests.download.entries.len(),
                        download_tag_matched = download_artifacts.tag_matched,
                        download_required = download_artifacts.required.len(),
                        download_present = download_artifacts.already_present,
                        download_bytes = download_artifacts.total_download_bytes(),
                        download_tags = manifests.download.header.tag_count(),
                        "download manifest classification complete"
                    );

                    // Build encoding key -> file paths map for install manifest entries.
                    // Used by the loose file handler to know which downloads need
                    // placement in the product subfolder. One encoding key can map
                    // to multiple paths when the same content is referenced from
                    // multiple manifest entries; all paths must be materialized.
                    let mut install_manifest_paths: InstallManifestPaths = HashMap::new();
                    for artifact in &install_artifacts.required {
                        install_manifest_paths
                            .entry(*artifact.encoding_key.as_bytes())
                            .or_default()
                            .push(artifact.path.clone());
                    }

                    // Keep install and download artifacts separate so loose files
                    // (install manifest) are downloaded before CASC blobs (download
                    // manifest) are downloaded before CASC blobs (download manifest).
                    let total_required =
                        install_artifacts.required.len() + download_artifacts.required.len();
                    let total_present =
                        install_artifacts.already_present + download_artifacts.already_present;
                    let total_bytes = install_artifacts.total_download_bytes()
                        + download_artifacts.total_download_bytes();

                    // The size manifest's total_size is used for disk space
                    // pre-allocation and esize values as progress fallback when cSize
                    // is unavailable. The download manifest bytes represent the actual
                    // download volume. Log both for diagnostics.
                    if let Some(ref size_manifest) = manifests.size {
                        info!(
                            size_manifest_total = size_manifest.header.total_size,
                            size_manifest_entries = size_manifest.entries.len(),
                            download_manifest_bytes = total_bytes,
                            "size manifest loaded (install footprint estimate)"
                        );
                    }

                    progress(ProgressEvent::MetadataResolved {
                        artifacts: total_required,
                        total_bytes,
                    });

                    info!(
                        total_required,
                        total_present,
                        total_bytes,
                        loose_files = install_artifacts.required.len(),
                        casc_files = download_artifacts.required.len(),
                        "artifact classification complete"
                    );

                    PipelineState::FetchingArchiveIndices {
                        manifests,
                        install_artifacts,
                        download_artifacts,
                        install_manifest_paths,
                    }
                }

                PipelineState::FetchingArchiveIndices {
                    manifests,
                    install_artifacts,
                    download_artifacts,
                    install_manifest_paths,
                } => {
                    let indices_dir = self.config.install_path.join("Data").join("indices");
                    tokio::fs::create_dir_all(&indices_dir).await?;

                    if endpoints.is_empty() {
                        return Err(InstallationError::InvalidConfig("no endpoints".to_string()));
                    }

                    // Regular archive indices (from /data/ path).
                    let archive_keys: Vec<String> = manifests
                        .cdn_config
                        .archives()
                        .iter()
                        .map(|a| a.content_key.clone())
                        .collect();

                    // Patch archive indices (from /patch/ path).
                    let patch_keys: Vec<String> = manifests
                        .cdn_config
                        .patch_archives()
                        .iter()
                        .map(|a| a.content_key.clone())
                        .collect();

                    // file-index and patch-file-index (loose file indices).
                    let file_index_key = manifests.cdn_config.file_index().map(String::from);
                    let patch_file_index_key =
                        manifests.cdn_config.patch_file_index().map(String::from);

                    // Data-path keys: regular archives + file-index
                    let mut data_keys = archive_keys.clone();
                    if let Some(ref fi) = file_index_key {
                        data_keys.push(fi.clone());
                    }

                    // Patch-path keys: patch archives + patch-file-index
                    let mut patch_path_keys = patch_keys.clone();
                    if let Some(ref pfi) = patch_file_index_key {
                        patch_path_keys.push(pfi.clone());
                    }

                    info!(
                        archives = archive_keys.len(),
                        patch_archives = patch_keys.len(),
                        file_index = file_index_key.is_some(),
                        patch_file_index = patch_file_index_key.is_some(),
                        "downloading CDN archive indices"
                    );

                    // Download regular archive indices (data path)
                    let indices_downloaded = download::download_archive_indices(
                        &cdn,
                        &endpoints,
                        &data_keys,
                        &indices_dir,
                        self.config.index_batch_size,
                        &progress,
                    )
                    .await?;

                    // Download patch archive indices (patch path)
                    if !patch_path_keys.is_empty() {
                        let patch_downloaded = download::download_patch_archive_indices(
                            &cdn,
                            &endpoints,
                            &patch_path_keys,
                            &indices_dir,
                            self.config.index_batch_size,
                            &progress,
                        )
                        .await?;
                        info!(
                            patch_indices = patch_downloaded,
                            "patch archive indices downloaded"
                        );
                    }

                    // Generate archive-group index (combined index of all
                    // individual archive indices). The client tries this
                    // first for faster lookups; falls back to individual
                    // indices if missing.
                    if let Some(group_hash) = manifests.cdn_config.archive_group() {
                        download::generate_group_index(
                            &indices_dir,
                            &archive_keys,
                            group_hash,
                            false,
                        )?;
                    }
                    if let Some(group_hash) = manifests.cdn_config.patch_archive_group() {
                        download::generate_group_index(
                            &indices_dir,
                            &patch_keys,
                            group_hash,
                            true,
                        )?;
                    }

                    // Parse downloaded indices into a lookup map for archive
                    // byte-range fallback when loose blob downloads fail.
                    // Only regular archive indices are used for the lookup;
                    // patch and file indices are consumed by the client.
                    let archive_lookup =
                        download::load_archive_indices(&indices_dir, &archive_keys)?;

                    PipelineState::Downloading {
                        manifests,
                        install_artifacts,
                        download_artifacts,
                        indices_downloaded,
                        archive_lookup,
                        install_manifest_paths,
                    }
                }

                PipelineState::Downloading {
                    mut manifests,
                    install_artifacts,
                    download_artifacts,
                    indices_downloaded,
                    archive_lookup,
                    install_manifest_paths,
                } => {
                    // Open local installation
                    let installation = Installation::open(self.config.install_path.join("Data"))?;
                    installation.initialize().await?;

                    // Write bootstrap files (encoding, install, download, root)
                    // to local CASC storage. The client expects these in the
                    // data archives, indexed via the local IDX files.
                    {
                        use cascette_crypto::EncodingKey;

                        for (label, ekey_hex, blte_data) in
                            std::mem::take(&mut manifests.bootstrap_blte)
                        {
                            let size = blte_data.len();
                            let ekey = match EncodingKey::from_hex(&ekey_hex) {
                                Ok(k) => k,
                                Err(e) => {
                                    warn!(
                                        file = label,
                                        error = %e,
                                        "invalid bootstrap ekey, skipping"
                                    );
                                    continue;
                                }
                            };
                            match installation
                                .write_raw_blte_with_ekey(blte_data, &ekey)
                                .await
                            {
                                Ok(()) => {
                                    info!(
                                        file = label,
                                        size,
                                        ekey = %ekey_hex,
                                        "bootstrap file written to CASC storage"
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        file = label,
                                        error = %e,
                                        "failed to write bootstrap file to CASC storage"
                                    );
                                }
                            }
                        }
                        installation.flush_indices().await?;
                    }

                    let installation = Arc::new(installation);

                    let build_config_hash = self
                        .config
                        .build_config
                        .as_deref()
                        .unwrap_or("")
                        .to_string();
                    let cdn_config_hash =
                        self.config.cdn_config.as_deref().unwrap_or("").to_string();

                    let total_artifacts =
                        install_artifacts.required.len() + download_artifacts.required.len();

                    let mut checkpoint = Checkpoint::new(
                        self.config.product.clone(),
                        build_config_hash,
                        cdn_config_hash,
                        total_artifacts,
                    );

                    // Merge any existing checkpoint
                    if self.config.resume
                        && let Some(existing) = Checkpoint::read(&self.config.install_path).await?
                    {
                        checkpoint.completed_keys = existing.completed_keys;
                    }

                    // Initialize loose file handler. When game_subfolder is None
                    // (e.g. launcher/bts install), use empty string to place loose
                    // files directly in the install root.
                    let subfolder = self.config.game_subfolder.clone().unwrap_or_default();
                    let loose_handler = {
                        let handler = loose::LooseFileHandler::new(
                            subfolder,
                            self.config.install_path.clone(),
                        )?;
                        Some(Arc::new(tokio::sync::Mutex::new(handler)))
                    };

                    let handler_ref = loose_handler.clone();

                    let key_store_ref = self.config.key_store.clone();
                    let on_file_written =
                        |artifact: &classify::ArtifactDescriptor,
                         inst: &Arc<Installation>,
                         blte_data: Option<Vec<u8>>| {
                            let handler = handler_ref.clone();
                            let ekey = *artifact.encoding_key.as_bytes();
                            let paths = install_manifest_paths.get(&ekey).cloned();
                            let inst = Arc::clone(inst);
                            let ks = key_store_ref.clone();
                            async move {
                                let Some(handler) = handler else { return };
                                let Some(paths) = paths else { return };
                                let mut h = handler.lock().await;
                                for file_path in &paths {
                                    if let Some(ref data) = blte_data {
                                        // Loose file: decode BLTE and write directly to filesystem.
                                        if let Err(e) =
                                            h.write_loose_file(&ekey, file_path, data).await
                                        {
                                            warn!(
                                                path = %file_path,
                                                error = %e,
                                                "loose file write failed"
                                            );
                                        }
                                    } else {
                                        // CASC file: hardlink or copy from archive storage.
                                        let key_ref = ks.as_ref().map(|k| {
                                            &**k as &(
                                                 dyn cascette_crypto::TactKeyProvider + Send + Sync
                                             )
                                        });
                                        if let Err(e) = h
                                            .on_file_complete(&ekey, file_path, &inst, key_ref)
                                            .await
                                        {
                                            warn!(
                                                path = %file_path,
                                                error = %e,
                                                "loose file placement failed"
                                            );
                                        }
                                    }
                                }
                            }
                        };

                    // Phase 1: Download install manifest entries (loose files) first.
                    // These are filesystem files (exe, dll, pak) written directly
                    // before processing CASC archive blobs.
                    let loose_report = if install_artifacts.required.is_empty() {
                        download::DownloadReport::default()
                    } else {
                        let loose_bytes = install_artifacts.total_download_bytes();
                        progress(ProgressEvent::LooseFilePhaseStarted {
                            files: install_artifacts.required.len(),
                            total_bytes: loose_bytes,
                        });
                        info!(
                            loose_files = install_artifacts.required.len(),
                            loose_bytes, "downloading loose files (install manifest)"
                        );

                        let loose_report = download::execute_downloads(
                            &self.config,
                            cdn.clone(),
                            Arc::clone(&installation),
                            &endpoints,
                            &install_artifacts.required,
                            &archive_lookup,
                            &mut checkpoint,
                            &progress,
                            &on_file_written,
                        )
                        .await?;

                        info!(
                            downloaded = loose_report.downloaded,
                            failed = loose_report.failed,
                            bytes = loose_report.bytes_downloaded,
                            "loose file download phase complete"
                        );

                        progress(ProgressEvent::LooseFilePhaseComplete {
                            files_placed: loose_report.downloaded,
                        });

                        loose_report
                    };

                    // Phase 2: Download download manifest entries (CASC archive blobs).
                    let casc_report = if download_artifacts.required.is_empty() {
                        download::DownloadReport::default()
                    } else {
                        info!(
                            casc_files = download_artifacts.required.len(),
                            "downloading CASC archive data (download manifest)"
                        );

                        let casc_report = download::execute_batched_downloads(
                            &self.config,
                            cdn.clone(),
                            Arc::clone(&installation),
                            &endpoints,
                            &download_artifacts.required,
                            &archive_lookup,
                            &mut checkpoint,
                            &progress,
                            &on_file_written,
                        )
                        .await?;

                        info!(
                            downloaded = casc_report.downloaded,
                            failed = casc_report.failed,
                            bytes = casc_report.bytes_downloaded,
                            "CASC download phase complete"
                        );

                        casc_report
                    };

                    // Merge reports from both phases
                    let mut combined_failed = loose_report.failed_files;
                    combined_failed.extend(casc_report.failed_files);

                    let download_report = download::DownloadReport {
                        downloaded: loose_report.downloaded + casc_report.downloaded,
                        failed: loose_report.failed + casc_report.failed,
                        skipped: loose_report.skipped + casc_report.skipped,
                        bytes_downloaded: loose_report.bytes_downloaded
                            + casc_report.bytes_downloaded,
                        failed_files: combined_failed,
                    };

                    // Phase 3: Build-config loose files + root-content pass.
                    // The client reads TVFS manifests, the patch index, and the
                    // patched VFS shards on a fresh install; the download
                    // manifest does not include them. The root manifest
                    // enumerates the real content set (the download manifest
                    // only tag-filters it). Both must be present or the client
                    // re-fetches from the CDN on first start.
                    {
                        use std::collections::HashSet;
                        // Tag-selected ekeys from the download manifest.
                        let selected: HashSet<Vec<u8>> = download_artifacts
                            .required
                            .iter()
                            .map(|a| a.encoding_key.as_bytes().to_vec())
                            .collect();
                        let loose_written = root_content::fetch_loose_build_files(
                            cdn.as_ref(),
                            &endpoints,
                            &manifests.build_config,
                            &installation,
                        )
                        .await;
                        let root_report = if let Some(root_ekey) = manifests
                            .build_config
                            .root()
                            .and_then(|h| ContentKey::from_hex(h).ok())
                            .and_then(|ck| manifests.encoding.find_encoding(&ck))
                        {
                            root_content::fetch_root_content(
                                cdn.as_ref(),
                                &endpoints,
                                &root_ekey,
                                &manifests.encoding,
                                &selected,
                                &archive_lookup,
                                &installation,
                            )
                            .await
                        } else {
                            root_content::RootContentReport::default()
                        };
                        info!(
                            loose_files_written = loose_written,
                            root_content_written = root_report.root_content_written,
                            unresolved_root = root_report.unresolved_root_entries,
                            "build-config loose + root content passes complete"
                        );
                    }
                    // Flush index entries to disk so the client can read them
                    // without a full index rebuild on first startup.
                    installation.flush_indices().await?;

                    let loose_file_report = match loose_handler {
                        Some(h) => h.lock().await.report(),
                        None => loose::LooseFileReport::default(),
                    };

                    PipelineState::WritingLayout {
                        manifests,
                        download_report,
                        indices_downloaded,
                        loose_report: loose_file_report,
                    }
                }

                PipelineState::WritingLayout {
                    manifests,
                    download_report,
                    indices_downloaded,
                    loose_report,
                } => {
                    // Write .build.info, .product.db, config files, .flavor.info
                    layout::write_layout(
                        &self.config,
                        &manifests,
                        download_report.bytes_downloaded,
                    )
                    .await?;
                    progress(ProgressEvent::LayoutWritten);

                    // Clear checkpoint on success
                    Checkpoint::clear(&self.config.install_path).await?;

                    PipelineState::Complete {
                        report: InstallReport {
                            downloaded: download_report.downloaded,
                            failed: download_report.failed,
                            skipped: download_report.skipped,
                            bytes_downloaded: download_report.bytes_downloaded,
                            indices_downloaded,
                            loose_files_placed: loose_report.linked + loose_report.copied,
                            failed_files: download_report.failed_files,
                        },
                    }
                }

                PipelineState::Complete { report } => return Ok(report),
            };
        }
    }
}
