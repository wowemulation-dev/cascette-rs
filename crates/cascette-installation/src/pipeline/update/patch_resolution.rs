//! Patch chain resolution for the update pipeline.
//!
//! Downloads and parses the patch config and patch archive indices, then builds
//! `PatchChain` instances for files that can be patched rather than re-downloaded.

use std::collections::HashMap;
use std::io::Cursor;

use tracing::{debug, info, warn};

use cascette_client_storage::container::residency::ResidencyContainer;
use cascette_formats::CascFormat;
use cascette_formats::archive::ArchiveIndex;
use cascette_formats::patch_archive::PatchArchive;
use cascette_formats::patch_chain::{PatchChain, PatchEdge};
use cascette_protocol::{CdnEndpoint, ContentType};

use crate::cdn_source::CdnSource;
use crate::error::{InstallationError, InstallationResult};
use crate::patch::resolver::PatchResolver;
use crate::pipeline::manifests::BuildManifests;

use super::PatchPlan;

/// Resolve a patch plan from the target manifests.
///
/// Downloads the patch archive (PA) manifest from CDN, downloads patch archive
/// indices, and builds patch chains for files that have a resident base version.
///
/// Returns an empty plan if no patch config is present or if resolution fails
/// (patch resolution failures are non-fatal -- the pipeline falls back to CDN
/// downloads).
pub async fn resolve_patch_plan<S: CdnSource>(
    _base_manifests: &BuildManifests,
    target_manifests: &BuildManifests,
    cdn: &S,
    endpoints: &[CdnEndpoint],
    residency: &ResidencyContainer,
) -> InstallationResult<PatchPlan> {
    let endpoint = endpoints.first().ok_or_else(|| {
        InstallationError::InvalidConfig("no CDN endpoints for patch resolution".to_string())
    })?;

    // Step 1: Download and parse the patch archive (PA) manifest.
    // The build config's `patch` field gives us the encoding key for the PA file.
    let Some(patch_info) = target_manifests.build_config.patch() else {
        debug!("build config has no patch entry, skipping patch resolution");
        return Ok(empty_plan());
    };

    let patch_ekey = patch_info
        .encoding_key
        .as_ref()
        .unwrap_or(&patch_info.content_key);

    info!(patch_ekey = %patch_ekey, "downloading patch archive manifest");

    let patch_key = hex::decode(patch_ekey)
        .map_err(|e| InstallationError::Format(format!("invalid patch encoding key hex: {e}")))?;

    let patch_data = match cdn.download(endpoint, ContentType::Data, &patch_key).await {
        Ok(data) => data,
        Err(e) => {
            warn!("failed to download patch archive manifest: {e}, skipping patches");
            return Ok(empty_plan());
        }
    };

    // PA files are BLTE-wrapped
    let pa_decoded = cascette_formats::blte::BlteFile::parse(&patch_data)
        .and_then(|blte| {
            blte.decompress()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        })
        .map_err(|e| {
            InstallationError::Format(format!("failed to decode patch archive BLTE: {e}"))
        })?;

    let patch_archive = PatchArchive::parse(&pa_decoded)
        .map_err(|e| InstallationError::Format(format!("failed to parse patch archive: {e}")))?;

    info!(
        file_entries = patch_archive.total_file_entries(),
        "parsed patch archive manifest"
    );

    // Step 2: Download patch archive indices from CDN config.
    // These tell us where individual patch blobs are located within patch archives.
    let resolver =
        download_patch_archive_indices(cdn, endpoint, &target_manifests.cdn_config).await?;

    info!(
        archives = resolver.archive_count(),
        entries = resolver.entry_count(),
        "built patch resolver from archive indices"
    );

    // Step 3: Build patch chains for resident files.
    // For each file entry in the PA, check if the source file is locally resident.
    // If so, build a PatchEdge and attempt to construct a PatchChain.
    let chains = build_patch_chains(&patch_archive, residency);

    info!(
        chains = chains.len(),
        "resolved patch chains for resident files"
    );

    Ok(PatchPlan { chains, resolver })
}

/// Download and parse patch archive index files from CDN.
///
/// Patch archives use the same index format as data archives but are stored
/// under the `patch/` CDN path prefix.
async fn download_patch_archive_indices<S: CdnSource>(
    cdn: &S,
    endpoint: &CdnEndpoint,
    cdn_config: &cascette_formats::config::CdnConfig,
) -> InstallationResult<PatchResolver> {
    let patch_archives = cdn_config.patch_archives();
    if patch_archives.is_empty() {
        debug!("no patch archives in CDN config");
        return Ok(PatchResolver::new(vec![], vec![]));
    }

    let archive_hashes: Vec<String> = patch_archives
        .iter()
        .map(|a| a.content_key.clone())
        .collect();

    let mut all_entries: Vec<([u8; 16], u16, u64, u32)> = Vec::new();

    for (archive_idx, archive_info) in patch_archives.iter().enumerate() {
        let hash = &archive_info.content_key;
        let index_key = hex::decode(hash).map_err(|e| {
            InstallationError::Format(format!("invalid patch archive hash hex: {e}"))
        })?;

        // Patch archive indices are at patch/{xx}/{yy}/{hash}.index
        // Use ContentType::Patch for the CDN path prefix
        let index_data = match cdn.download(endpoint, ContentType::Patch, &index_key).await {
            Ok(data) => data,
            Err(e) => {
                warn!(
                    archive = %hash,
                    error = %e,
                    "failed to download patch archive index, skipping"
                );
                continue;
            }
        };

        let index = match ArchiveIndex::parse(Cursor::new(&index_data)) {
            Ok(idx) => idx,
            Err(e) => {
                warn!(
                    archive = %hash,
                    error = %e,
                    "failed to parse patch archive index, skipping"
                );
                continue;
            }
        };

        for entry in &index.entries {
            if entry.encoding_key.len() >= 16 {
                let mut ekey = [0u8; 16];
                ekey.copy_from_slice(&entry.encoding_key[..16]);
                #[allow(clippy::cast_possible_truncation)]
                let idx = archive_idx as u16;
                all_entries.push((ekey, idx, entry.offset, entry.size));
            }
        }

        debug!(
            archive = %hash,
            entries = index.entries.len(),
            "parsed patch archive index"
        );
    }

    Ok(PatchResolver::new(archive_hashes, all_entries))
}

/// Build patch chains from the patch archive manifest.
///
/// For each file entry in the PA, collects all available patch edges where
/// the source file is resident locally. Then attempts to build a chain from
/// the source EKey to the target (via `PatchChain::build`).
fn build_patch_chains(
    patch_archive: &PatchArchive,
    residency: &ResidencyContainer,
) -> HashMap<[u8; 16], PatchChain> {
    let mut chains: HashMap<[u8; 16], PatchChain> = HashMap::new();

    for file_entry in patch_archive.all_file_entries() {
        // Collect all edges for this file entry
        let mut edges: Vec<PatchEdge> = Vec::new();
        let mut resident_sources: Vec<[u8; 16]> = Vec::new();

        for patch in &file_entry.patches {
            edges.push(PatchEdge {
                from_ekey: patch.source_ekey,
                from_size: patch.source_decoded_size,
                to_ekey: file_entry.target_ckey,
                to_size: file_entry.decoded_size,
                patch_key: patch.patch_ekey,
                patch_size: u64::from(patch.patch_size),
            });

            // Track which source files we have locally
            if residency.is_resident(&patch.source_ekey) {
                resident_sources.push(patch.source_ekey);
            }
        }

        if edges.is_empty() || resident_sources.is_empty() {
            continue;
        }

        // Try to build a chain from each resident source to the target
        let target_key = file_entry.target_ckey;
        for source in &resident_sources {
            match PatchChain::build(&edges, *source, target_key) {
                Ok(chain) => {
                    debug!(
                        source = %hex::encode(source),
                        target = %hex::encode(target_key),
                        steps = chain.len(),
                        "built patch chain"
                    );
                    chains.insert(target_key, chain);
                    break; // One chain per target is sufficient
                }
                Err(e) => {
                    debug!(
                        source = %hex::encode(source),
                        target = %hex::encode(target_key),
                        error = ?e,
                        "failed to build patch chain, trying next source"
                    );
                }
            }
        }
    }

    chains
}

/// Create an empty patch plan.
fn empty_plan() -> PatchPlan {
    PatchPlan {
        chains: HashMap::new(),
        resolver: PatchResolver::new(vec![], vec![]),
    }
}
