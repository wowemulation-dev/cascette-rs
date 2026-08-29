//! Loose-only install pipeline.
//!
//! Variant of the full install pipeline that writes only loose files (the
//! install manifest's filesystem entries: Wow.exe, DLLs, locale packs) plus
//! root layout metadata (`.build.info`, `.product.db`, `.patch.result`,
//! `<subfolder>/.flavor.info`) and an empty `Data/{config,data,indices}`
//! directory tree.
//!
//! The wow client bootstraps CASC content from the configured CDN on first
//! launch. End-users see this as the "performing initial setup of required
//! data files" UI for one to ten minutes depending on download bandwidth.
//!
//! Compared with the full pipeline this skips:
//!
//! - Encoding-table parsing for download-manifest entries
//! - Download manifest classification
//! - Archive index downloads (`*.index`)
//! - BLTE blob writing into `data.NNN`
//! - IDX file writing
//! - Local config blob mirroring
//!
//! For wow_classic / wow_classic_era this reduces a ~4 GiB CASC write down
//! to ~150 MiB of loose files, an order of magnitude less disk and time.

use std::sync::Arc;

use tracing::info;

use cascette_client_storage::Installation;
use cascette_protocol::CdnEndpoint;

use crate::cdn_source::CdnSource;
use crate::checkpoint::Checkpoint;
use crate::config::InstallConfig;
use crate::error::{InstallationError, InstallationResult};
use crate::layout;
use crate::pipeline::classify;
use crate::pipeline::download;
use crate::pipeline::install::InstallReport;
use crate::pipeline::loose;
use crate::pipeline::metadata;
use crate::progress::ProgressEvent;

/// Run the loose-only install pipeline.
///
/// Writes only the loose filesystem files referenced by the install manifest
/// plus the root layout metadata. CASC content (`Data/data/data.NNN`,
/// `Data/data/<bucket>NNNNNNNN.idx`, `Data/indices/*.index`,
/// `Data/config/*`) is left for the wow client to bootstrap from the CDN.
///
/// The empty `Data/{config,data,indices}` directories are created so the
/// client's CAS init code finds the expected layout shape.
pub async fn run<S: CdnSource + 'static>(
    config: InstallConfig,
    cdn: Arc<S>,
    endpoints: Vec<CdnEndpoint>,
    progress: impl Fn(ProgressEvent) + Send + Sync,
) -> InstallationResult<InstallReport> {
    info!(
        product = %config.product,
        install_path = %config.install_path.display(),
        "running loose-only install pipeline"
    );

    if endpoints.is_empty() {
        return Err(InstallationError::InvalidConfig("no endpoints".to_string()));
    }

    // Phase 1: Resolve manifests.
    //
    // Loose-only mode still needs the install manifest (to know which
    // filesystem files to write) and the encoding table (to translate
    // content keys to encoding keys), but does not need the download
    // manifest, root, or size manifest. resolve_manifests fetches all of
    // them; the unused ones cost a few extra HTTP requests but keep this
    // path simple and aligned with the full pipeline.
    let manifests =
        metadata::resolve_manifests(&config, cdn.as_ref(), &endpoints, &progress).await?;

    // Phase 2: Classify install-manifest entries against the tag query.
    let checkpoint_existing = if config.resume {
        Checkpoint::read(&config.install_path).await?
    } else {
        None
    };
    let known_keys = download::collect_known_keys(&checkpoint_existing);

    let tag_names = config.tag_query.tag_names();
    let tags: Vec<&str> = tag_names.iter().map(String::as_str).collect();
    info!(
        tags = ?tags,
        product = %config.product,
        region = %config.region,
        locale = %config.tag_query.locale,
        build_info_tags = %config.tag_query.build_info_tags(),
        "classifying install manifest with tag query (loose-only)"
    );

    let install_artifacts = classify::classify_artifacts(
        &manifests.install,
        &manifests.encoding,
        &manifests.download,
        &tags,
        &known_keys,
    )?;
    info!(
        install_total_entries = manifests.install.entries.len(),
        install_required = install_artifacts.required.len(),
        install_present = install_artifacts.already_present,
        install_unresolved = install_artifacts.unresolved,
        install_bytes = install_artifacts.total_download_bytes(),
        "install manifest classification complete (loose-only)"
    );

    let total_required = install_artifacts.required.len();
    let total_bytes = install_artifacts.total_download_bytes();
    progress(ProgressEvent::MetadataResolved {
        artifacts: total_required,
        total_bytes,
    });

    // Phase 3: Open a minimal Installation handle.
    //
    // execute_downloads requires an `Installation` even when the path
    // through it is loose-only (the CASC `write_raw_blte` branch is taken
    // only for `is_loose_file = false` artifacts, which loose-only filters
    // out). Opening the handle does create empty `Data/data/`, which is
    // exactly what the client's CAS init expects to find on disk.
    let installation = Installation::open(config.install_path.join("Data"))?;
    installation.initialize().await?;
    let installation = Arc::new(installation);

    // The client also looks for `Data/config/` and `Data/indices/`. Make
    // them up-front so the client's directory-walk during initial setup
    // finds the layout shape it expects.
    tokio::fs::create_dir_all(config.install_path.join("Data").join("config")).await?;
    tokio::fs::create_dir_all(config.install_path.join("Data").join("indices")).await?;

    // Phase 4: Build the install-manifest path lookup the loose handler uses.
    //
    // One encoding key can map to multiple paths when the same content is
    // referenced from multiple install-manifest entries (e.g. an `Info.plist`
    // file deduplicated across N locale subdirectories of a .app bundle).
    // All paths must be materialized to match Agent.exe output.
    use std::collections::HashMap;
    let mut install_manifest_paths: HashMap<[u8; 16], Vec<String>> = HashMap::new();
    for artifact in &install_artifacts.required {
        install_manifest_paths
            .entry(*artifact.encoding_key.as_bytes())
            .or_default()
            .push(artifact.path.clone());
    }

    // Phase 5: Set up the loose file handler.
    //
    // The full pipeline supports `game_subfolder = None` (placing files at
    // the install root) for bts/launcher products. For loose-only mode we
    // require a subfolder because the wow client expects its binaries
    // under `<install_dir>/<subfolder>/` and the empty `Data/` shape we
    // generate is meaningless without it.
    let subfolder = config.game_subfolder.clone().unwrap_or_default();
    let loose_handler = loose::LooseFileHandler::new(subfolder, config.install_path.clone())?;
    let loose_handler = Arc::new(tokio::sync::Mutex::new(loose_handler));

    let handler_ref = loose_handler.clone();
    let on_file_written = |artifact: &classify::ArtifactDescriptor,
                           _inst: &Arc<Installation>,
                           blte_data: Option<Vec<u8>>| {
        let handler = handler_ref.clone();
        let ekey = *artifact.encoding_key.as_bytes();
        let paths = install_manifest_paths.get(&ekey).cloned();
        async move {
            let Some(paths) = paths else { return };
            let mut h = handler.lock().await;
            if let Some(data) = blte_data {
                for file_path in &paths {
                    if let Err(e) = h.write_loose_file(&ekey, file_path, &data).await {
                        tracing::warn!(
                            path = %file_path,
                            error = %e,
                            "loose file write failed"
                        );
                    }
                }
            }
            // No CASC-write branch: loose-only mode has no archive
            // storage to hardlink from, and download::execute_downloads
            // never sends `Some(_)` data with `is_loose_file = false`
            // here because we only feed it loose artifacts.
        }
    };

    // Phase 6: Download and place loose files.
    let build_config_hash = config.build_config.as_deref().unwrap_or("").to_string();
    let cdn_config_hash = config.cdn_config.as_deref().unwrap_or("").to_string();
    let mut checkpoint = Checkpoint::new(
        config.product.clone(),
        build_config_hash,
        cdn_config_hash,
        total_required,
    );
    if config.resume
        && let Some(existing) = checkpoint_existing
    {
        checkpoint.completed_keys = existing.completed_keys;
    }

    let loose_report = if install_artifacts.required.is_empty() {
        download::DownloadReport::default()
    } else {
        progress(ProgressEvent::LooseFilePhaseStarted {
            files: install_artifacts.required.len(),
            total_bytes,
        });
        info!(
            loose_files = install_artifacts.required.len(),
            loose_bytes = total_bytes,
            "downloading loose files (loose-only mode)"
        );

        // execute_downloads dispatches per artifact: archive byte-range when
        // the encoding key has an entry in `archive_lookup`, loose blob via
        // `/data/<aa>/<bb>/<hash>` otherwise.
        //
        // For older builds (e.g. wow_classic 2.5.x BCC) the loose-blob
        // objects have been pruned from every CDN we know about (Blizzard
        // level3, archive.wow.tools, casc.wago.tools, cdn.arctium.tools).
        // The archive byte-range path is the only way to retrieve their
        // install-manifest content. Build the lookup here from the CDN
        // archive indices — this download is small (~2-3 MiB total) and
        // happens entirely in memory; we never persist the indices to disk
        // because the wow client will rebuild them itself on first launch.
        let archive_lookup =
            metadata::build_archive_lookup(cdn.as_ref(), &endpoints, &manifests.cdn_config).await;
        info!(
            archive_lookup_entries = archive_lookup.len(),
            "archive lookup built for loose-only mode"
        );

        let report = download::execute_downloads(
            &config,
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

        progress(ProgressEvent::LooseFilePhaseComplete {
            files_placed: report.downloaded,
        });
        report
    };

    info!(
        downloaded = loose_report.downloaded,
        failed = loose_report.failed,
        bytes = loose_report.bytes_downloaded,
        "loose-only install pipeline downloads complete"
    );

    // Phase 7: Write layout metadata.
    layout::write_layout(&config, &manifests, loose_report.bytes_downloaded).await?;

    // Drop the Installation so its IDX file handles flush. We don't write
    // any CASC data, so the resulting Data/data/ contains only the empty
    // `_01.idx` files Installation::initialize() created. That's fine —
    // the client overwrites them with its own freshly-bootstrapped IDX
    // versions on first launch.
    drop(installation);

    Ok(InstallReport {
        downloaded: loose_report.downloaded,
        failed: loose_report.failed,
        skipped: loose_report.skipped,
        bytes_downloaded: loose_report.bytes_downloaded,
        indices_downloaded: 0,
        loose_files_placed: loose_report.downloaded,
        failed_files: loose_report.failed_files,
    })
}
