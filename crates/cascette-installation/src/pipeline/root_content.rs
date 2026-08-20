//! Root-content and loose build-config file resolution for the install pipeline.
//!
//! Mirrors cascette-py's Step 7.4 / Step 7.5 (commits `dc0dfb8` and `17339be`),
//! grounded in the battle.net-agent install model (`install_operation.cpp`):
//! the ROOT manifest drives the content set; the download manifest only
//! tag-filters it.
//!
//! Two passes live here:
//!
//! - [`fetch_loose_build_files`] fetches the build-config loose files the
//!   client reads on a fresh install but which the download manifest does not
//!   include: the TVFS manifests (`vfs-root`, `vfs-N`, 1.14.4+), the
//!   patch-index manifest, and the patched VFS shards named by the patch
//!   config's `patch-entry = vfs:` records (1.15.4+). Without these the
//!   client re-fetches them from the CDN on first start (149 requests on
//!   1.14.4.51001, 5382 on 1.15.4.56738).
//!
//! - [`fetch_root_content`] resolves the root manifest's content set (TVFS CFT
//!   ekeys, or MFST/TSFM content keys through the encoding table), intersects
//!   with the tag-selected download ekeys, and fetches anything missing from
//!   the local store. On 1.15.4.56738 the root enumerates 214k content keys;
//!   without this pass the client re-fetches ~544 files (WDC5/BLP2/REVM) from
//!   the CDN on first start (7200 requests, no login).

use crate::pipeline::download::ArchiveLookup;
use crate::CdnSource;
use cascette_client_storage::Installation;
use cascette_crypto::EncodingKey;
use cascette_formats::blte::BlteFile;
use cascette_formats::config::BuildConfig;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::root::RootFile;
use cascette_formats::tvfs::TvfsFile;
use cascette_formats::CascFormat;
use cascette_protocol::{CdnEndpoint, ContentType};
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// Result of the loose build-file + root-content passes.
#[derive(Debug, Default)]
pub struct RootContentReport {
    /// Loose build-config files written (vfs-root, vfs-N, patch-index, patched shards).
    pub loose_files_written: usize,
    /// Root-content files written.
    pub root_content_written: usize,
    /// Root entries that could not be resolved to an encoding key.
    pub unresolved_root_entries: usize,
}

/// Fetch the build-config loose files (TVFS manifests, patch index, patched
/// VFS shards) into CASC storage.
///
/// Returns the number of files written. Failures are logged per-file, not
/// fatal: a missing optional loose file degrades the first-start experience
/// but does not invalidate the install.
pub async fn fetch_loose_build_files<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    build_config: &BuildConfig,
    installation: &Installation,
) -> usize {
    let mut targets: Vec<(&str, String)> = Vec::new();

    if let Some(vfs_root) = build_config.vfs_root() {
        if let Some(ekey) = &vfs_root.encoding_key {
            targets.push(("vfs-root", ekey.clone()));
        }
    }
    for (index, info) in build_config.vfs_entries() {
        if let Some(ekey) = &info.encoding_key {
            targets.push((Box::leak(format!("vfs-{index}").into_boxed_str()), ekey.clone()));
        }
    }
    for info in build_config.patch_index() {
        if let Some(ekey) = &info.encoding_key {
            targets.push(("patch-index", ekey.clone()));
        }
    }

    // 1.15.4+: the patch config carries `patch-entry = vfs:<shard>:` records.
    // Each record group names a PATCHED VFS shard: patched_ekey patched_size
    // patch_blob_ekey patch_blob_size. The client walks the patched VFS tree
    // on a fresh install; without the shards it re-fetches ~200 content files.
    if let Some(patch_config_hash) = build_config.patch_config() {
        match cdn
            .download(
                &endpoints[0],
                ContentType::Patch,
                patch_config_hash.as_bytes(),
            )
            .await
        {
            Ok(patch_config_data) => {
                let patched = parse_vfs_shards(&String::from_utf8_lossy(&patch_config_data));
                info!(shards = patched.len(), "patch config vfs shards");
                for (i, ekey_hex) in patched.iter().enumerate() {
                    targets.push((
                        Box::leak(format!("vfs-patched-{i}").into_boxed_str()),
                        ekey_hex.clone(),
                    ));
                }
            }
            Err(e) => {
                warn!(error = %e, "failed to fetch patch config for vfs shards");
            }
        }
    }
    if targets.is_empty() {
        return 0;
    }

    info!(files = targets.len(), "fetching build-config loose files");
    let mut written = 0usize;
    for (name, ekey_hex) in &targets {
        let ekey = match EncodingKey::from_hex(ekey_hex) {
            Ok(k) => k,
            Err(e) => {
                warn!(name, error = %e, "invalid loose ekey");
                continue;
            }
        };
        match cdn
            .download(&endpoints[0], ContentType::Data, ekey.as_bytes())
            .await
        {
            Ok(blte) => match installation.write_raw_blte_with_ekey(blte, &ekey).await {
                Ok(()) => {
                    written += 1;
                    debug!(name, "loose build file written");
                }
                Err(e) => warn!(name, error = %e, "failed to write loose build file"),
            },
            Err(e) => warn!(name, error = %e, "failed to fetch loose build file"),
        }
    }
    info!(written, total = targets.len(), "loose build files complete");
    written
}

/// Resolve the root manifest's content set and fetch anything missing from
/// the local store.
///
/// Root entries are TVFS CFT ekeys (1.14.4+ VFS root) or MFST/TSFM content
/// keys resolved through the encoding table. The tag-selected download
/// manifest ekeys narrow the candidate set; anything still missing from the
/// store is fetched via archive range request with a loose-blob fallback.
pub async fn fetch_root_content<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    root_ekey: &EncodingKey,
    encoding: &EncodingFile,
    download_selected_ekeys: &HashSet<Vec<u8>>,
    archive_lookup: &ArchiveLookup,
    installation: &Installation,
) -> RootContentReport {
    let mut report = RootContentReport::default();

    let root_blte = match cdn
        .download(&endpoints[0], ContentType::Data, root_ekey.as_bytes())
        .await
    {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "failed to fetch root manifest");
            return report;
        }
    };
    let root_data = match BlteFile::parse(&root_blte) {
        Ok(blte) => match blte.decompress() {
            Ok(d) => d,
            Err(e) => {
                warn!(error = %e, "failed to decompress root manifest");
                return report;
            }
        },
        Err(e) => {
            warn!(error = %e, "failed to parse root manifest");
            return report;
        }
    };

    // Enumerate root entries: (key bytes, is_ekey_direct).
    // TVFS root -> CFT ekeys directly; MFST/TSFM root -> content keys.
    let mut candidate_ekeys: HashSet<Vec<u8>> = HashSet::new();
    let mut unresolved = 0usize;
    if root_data.starts_with(b"TVFS") {
        match TvfsFile::parse(&root_data) {
            Ok(tvfs) => {
                for entry in &tvfs.container_table.entries {
                    candidate_ekeys.insert(entry.ekey.clone());
                }
            }
            Err(e) => {
                warn!(error = %e, "failed to parse TVFS root");
                return report;
            }
        }
    } else {
        match RootFile::parse(&root_data) {
            Ok(root) => {
                for block in &root.blocks {
                    for record in &block.records {
                        match encoding.find_encoding(&record.content_key) {
                            Some(ekey) => {
                                candidate_ekeys.insert(ekey.as_bytes().to_vec());
                            }
                            None => unresolved += 1,
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "failed to parse MFST/TSFM root");
                return report;
            }
        }
    }
    report.unresolved_root_entries = unresolved;
    info!(candidates = candidate_ekeys.len(), "root content candidates");

    // Tag filter: keep only ekeys the download manifest selected.
    if !download_selected_ekeys.is_empty() {
        candidate_ekeys.retain(|k| download_selected_ekeys.contains(k));
    }
    info!(filtered = candidate_ekeys.len(), "root content after tag filter");

    // Determine what is already in the store (KMT 9-byte prefixes).
    let mut missing: Vec<Vec<u8>> = Vec::new();
    for ekey in candidate_ekeys {
        let ekey_full = EncodingKey::from_bytes(
            ekey.as_slice().try_into().expect("16-byte root ekey"),
        );
        if installation.has_encoding_key(&ekey_full).await {
            continue; // already in the store
        }
        missing.push(ekey);
    }
    info!(missing = missing.len(), "root content to fetch");

    // Fetch each missing ekey: archive range request first, loose blob fallback.
    let mut written = 0usize;
    for ekey in missing {
        let ekey_full = EncodingKey::from_bytes(
            ekey.as_slice().try_into().expect("16-byte root ekey"),
        );
        let data = match fetch_one(cdn, endpoints, &ekey_full, archive_lookup).await {
            Some(d) => d,
            None => {
                warn!(ekey = %hex::encode(&ekey), "root content fetch failed");
                continue;
            }
        };
        match installation.write_raw_blte_with_ekey(data, &ekey_full).await {
            Ok(()) => written += 1,
            Err(e) => warn!(ekey = %hex::encode(&ekey), error = %e, "root content write failed"),
        }
    }
    report.root_content_written = written;
    info!(written, "root content pass complete");
    report
}

/// Fetch one content blob: archive range request, then loose CDN blob.
async fn fetch_one<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    ekey: &EncodingKey,
    archive_lookup: &ArchiveLookup,
) -> Option<Vec<u8>> {
    if let Some(loc) = archive_lookup.get(ekey.as_bytes().as_slice()) {
        for ep in endpoints {
            match cdn
                .download_range(
                    ep,
                    ContentType::Data,
                    &loc.archive_key,
                    loc.offset,
                    loc.size.into(),
                )
                .await
            {
                Ok(d) => return Some(d),
                Err(e) => debug!(host = %ep.host, error = %e, "archive range failed"),
            }
        }
    }
    for ep in endpoints {
        match cdn.download(ep, ContentType::Data, ekey.as_bytes()).await {
            Ok(d) => return Some(d),
            Err(e) => debug!(host = %ep.host, error = %e, "loose blob failed"),
        }
    }
    None
}

/// Extract the patched-VFS shard ekeys from patch config text.
///
/// Parses `patch-entry = vfs:<name>: ...` records. The first six tokens
/// after the prefix are the record header (`NAME:` ckey csize ekey esize
/// espec); each following group of 4 is a patched shard:
/// `patched_ekey patched_size patch_blob_ekey patch_blob_size`. Returns
/// the patched ekeys in order.
pub(crate) fn parse_vfs_shards(text: &str) -> Vec<String> {
    let mut patched: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("patch-entry = vfs:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            // parts: [NAME:, ckey, csize, ekey, esize, espec, then
            // record groups of 4: patched_ekey patched_size blob_ekey blob_size]
            if parts.len() >= 7 {
                let records = &parts[6..];
                for chunk in records.chunks(4) {
                    if chunk.len() == 4 {
                        patched.push(chunk[0].to_string());
                    }
                }
            }
        }
    }
    patched
}

#[cfg(test)]
mod tests {
    use super::parse_vfs_shards;

    #[test]
    fn test_parse_vfs_shards_extracts_patched_ekeys() {
        let text = "patch-entry = vfs:32: aaaa bbbb cccc dddd espec 11112222333344445555666677778888 100 9999aaaabbbbccccddddeeeeffff0000 50 aaaabbbbccccddddeeeeffff00001111 200 ccccddddeeeeffff0000111122223333 300";
        let shards = parse_vfs_shards(text);
        assert_eq!(shards.len(), 2);
        assert_eq!(shards[0], "11112222333344445555666677778888");
        assert_eq!(shards[1], "aaaabbbbccccddddeeeeffff00001111");
    }

    #[test]
    fn test_parse_vfs_shards_ignores_other_patch_entries() {
        let text = [
            "patch-entry = zzz: x y z w",
            "patch-entry = vfs:1: a b c d espec 1111 1 2222 2 3333 3 4444 4",
        ]
        .join("\n");
        let shards = parse_vfs_shards(&text);
        assert_eq!(shards, vec!["1111", "3333"]);
    }
}
