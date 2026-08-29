//! Metadata resolution: Ribbit -> BuildConfig -> CdnConfig -> manifests.
//!
//! Resolves all manifest files needed for installation from CDN endpoints.
//!
//! Archive indices are downloaded lazily on first loose download failure and
//! used as fallback for subsequent downloads. This avoids downloading hundreds
//! of index files for products where all metadata is available as loose blobs
//! (e.g. WoW game content), while still supporting products like `bts`
//! (bootstrapper) that pack content exclusively into archives.

use std::io::Cursor;

use binrw::BinRead;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

use cascette_crypto::{ContentKey, TactKeyProvider};
use cascette_formats::CascFormat;
use cascette_formats::archive::ArchiveIndex;
use cascette_formats::blte::BlteFile;
use cascette_formats::config::{BuildConfig, CdnConfig};
use cascette_formats::download::DownloadManifest;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_formats::patch_index::PatchIndex;
use cascette_formats::root::RootFile;
use cascette_formats::size::SizeManifest;
use cascette_protocol::{CdnEndpoint, ContentType};

use crate::cdn_source::CdnSource;
use crate::config::InstallConfig;
use crate::error::{InstallationError, InstallationResult};
use crate::pipeline::download::ArchiveLookup;
use crate::pipeline::manifests::BuildManifests;
use crate::progress::ProgressEvent;

/// Download from CDN trying each endpoint in order until one succeeds.
async fn download_with_fallback<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    content_type: ContentType,
    key: &[u8],
) -> InstallationResult<Vec<u8>> {
    let mut last_err = None;
    for ep in endpoints {
        match cdn.download(ep, content_type, key).await {
            Ok(data) => return Ok(data),
            Err(e) => {
                debug!(host = %ep.host, error = %e, "endpoint failed, trying next");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        InstallationError::InvalidConfig("no CDN endpoints configured".to_string())
    }))
}

/// Download from CDN with lazy archive byte-range fallback.
///
/// For each endpoint, tries the loose blob first. If the loose download
/// fails, lazily builds the archive lookup and attempts a byte-range
/// fetch from the same endpoint's archive. This avoids unnecessary
/// failover to remote CDN endpoints when the data is available in
/// local archives.
///
/// The `archive_lookup_cell` is a `OnceCell` shared across all metadata
/// downloads in one `resolve_manifests` call. It is initialized at most once,
/// only when a loose download fails — avoiding the cost of downloading
/// hundreds of archive indices for products where all metadata is loose.
async fn download_with_archive_fallback<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    content_type: ContentType,
    key: &[u8],
    archive_lookup_cell: &OnceCell<ArchiveLookup>,
    cdn_config: &CdnConfig,
) -> InstallationResult<Vec<u8>> {
    let mut last_err = None;

    for ep in endpoints {
        // Try loose blob on this endpoint
        match cdn.download(ep, content_type, key).await {
            Ok(data) => return Ok(data),
            Err(e) => {
                debug!(host = %ep.host, error = %e, "loose blob failed, trying archive");
                last_err = Some(e);
            }
        }

        // Loose failed — try archive-based resolution on the same endpoint
        let archive_lookup = archive_lookup_cell
            .get_or_init(|| build_archive_lookup(cdn, endpoints, cdn_config))
            .await;

        if let Some(loc) = archive_lookup.get(key) {
            let archive_key_hex = hex::encode(&loc.archive_key);
            debug!(
                ekey = %hex::encode(key),
                archive = %archive_key_hex,
                offset = loc.offset,
                size = loc.size,
                host = %ep.host,
                "resolving from archive"
            );
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
                Ok(data) => return Ok(data),
                Err(e) => {
                    debug!(
                        host = %ep.host,
                        error = %e,
                        "archive range download failed, trying next endpoint"
                    );
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        InstallationError::InvalidConfig("no CDN endpoints configured".to_string())
    }))
}

/// Download archive indices from CDN into memory and build a lookup map.
///
/// Unlike the install pipeline's `download_archive_indices()` which writes
/// to disk, this builds an in-memory lookup. Used by the metadata phase
/// (lazy fallback for pruned loose blobs) and by the loose-only install
/// pipeline (some older builds have all install-manifest blobs pruned
/// from CDN as loose objects but still served via archive byte-ranges).
pub(crate) async fn build_archive_lookup<S: CdnSource>(
    cdn: &S,
    endpoints: &[CdnEndpoint],
    cdn_config: &CdnConfig,
) -> ArchiveLookup {
    let mut lookup = ArchiveLookup::new();
    let archives = cdn_config.archives();

    if archives.is_empty() {
        return lookup;
    }

    info!(
        archives = archives.len(),
        "downloading archive indices for metadata resolution"
    );

    for archive in archives {
        let key = &archive.content_key;
        let mut downloaded = false;
        for ep in endpoints {
            match cdn.download_archive_index(ep, key).await {
                Ok(data) => {
                    let cursor = Cursor::new(&data);
                    match ArchiveIndex::parse(cursor) {
                        Ok(index) => {
                            let archive_key_bytes = hex::decode(key).unwrap_or_default();
                            for entry in &index.entries {
                                if entry.is_zero() {
                                    continue;
                                }
                                lookup.insert(
                                    entry.encoding_key.clone(),
                                    super::download::ArchiveLocation {
                                        archive_key: archive_key_bytes.clone(),
                                        offset: entry.offset,
                                        size: entry.size,
                                    },
                                );
                            }
                            debug!(
                                archive = %key,
                                entries = index.entries.len(),
                                "parsed archive index for metadata resolution"
                            );
                            downloaded = true;
                        }
                        Err(e) => {
                            warn!(
                                archive = %key,
                                error = %e,
                                "failed to parse archive index"
                            );
                        }
                    }
                    break;
                }
                Err(e) => {
                    debug!(
                        host = %ep.host,
                        archive = %key,
                        error = %e,
                        "archive index download failed, trying next endpoint"
                    );
                }
            }
        }
        if !downloaded {
            warn!(
                archive = %key,
                "failed to download archive index from any endpoint"
            );
        }
    }

    info!(
        entries = lookup.len(),
        "archive lookup ready for metadata resolution"
    );
    lookup
}

/// Resolve all build manifests from CDN.
///
/// Resolution order:
/// 1. Fetch build config and CDN config (from hashes or Ribbit)
/// 2. Fetch encoding file using the encoding key from build config
/// 3. Fetch install manifest, download manifest, and root file
///
/// Each download tries all endpoints in order (fallback on 403/errors).
pub async fn resolve_manifests<S: CdnSource>(
    config: &InstallConfig,
    cdn: &S,
    endpoints: &[CdnEndpoint],
    progress: &(impl Fn(ProgressEvent) + Send + Sync),
) -> InstallationResult<BuildManifests> {
    progress(ProgressEvent::MetadataResolving {
        product: config.product.clone(),
    });

    if endpoints.is_empty() {
        return Err(InstallationError::InvalidConfig(
            "no CDN endpoints configured".to_string(),
        ));
    }

    let key_store: Option<&(dyn TactKeyProvider + Send + Sync)> = config.key_store.as_deref();

    // Step 1: Fetch build config
    let build_config_hash = config.build_config.as_ref().ok_or_else(|| {
        InstallationError::InvalidConfig("build_config hash required".to_string())
    })?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "build config".to_string(),
    });
    info!(hash = %build_config_hash, "fetching build config");
    let build_config_key = hex::decode(build_config_hash)?;
    let build_config_data =
        download_with_fallback(cdn, endpoints, ContentType::Config, &build_config_key).await?;
    let build_config = BuildConfig::parse(&build_config_data[..])
        .map_err(|e| InstallationError::Format(format!("failed to parse build config: {e}")))?;
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "build config".to_string(),
        entries: 0,
    });

    // Step 2: Fetch CDN config
    let cdn_config_hash = config
        .cdn_config
        .as_ref()
        .ok_or_else(|| InstallationError::InvalidConfig("cdn_config hash required".to_string()))?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "CDN config".to_string(),
    });
    info!(hash = %cdn_config_hash, "fetching CDN config");
    let cdn_config_key = hex::decode(cdn_config_hash)?;
    let cdn_config_data =
        download_with_fallback(cdn, endpoints, ContentType::Config, &cdn_config_key).await?;
    let cdn_config = CdnConfig::parse(&cdn_config_data[..])
        .map_err(|e| InstallationError::Format(format!("failed to parse CDN config: {e}")))?;
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "CDN config".to_string(),
        entries: 0,
    });

    // Lazy archive lookup: initialized on first loose download failure.
    // Products where all metadata is loose (e.g. WoW game content) never pay
    // the cost of downloading archive indices. Products like bts that pack
    // content into archives trigger the lookup on first miss.
    let archive_lookup_cell: OnceCell<ArchiveLookup> = OnceCell::new();

    // Step 3: Fetch encoding file
    let encoding_info = build_config.encoding().ok_or_else(|| {
        InstallationError::Format("build config missing encoding reference".to_string())
    })?;

    let encoding_ekey = encoding_info.encoding_key.as_deref().ok_or_else(|| {
        InstallationError::Format("build config missing encoding key".to_string())
    })?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "encoding table".to_string(),
    });
    info!(ekey = %encoding_ekey, "fetching encoding file");
    let encoding_key_bytes = hex::decode(encoding_ekey)?;
    let encoding_data = download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        &encoding_key_bytes,
        &archive_lookup_cell,
        &cdn_config,
    )
    .await?;

    // Encoding file is BLTE-encoded on CDN
    let encoding = EncodingFile::parse_blte(&encoding_data)
        .map_err(|e| InstallationError::Format(format!("failed to parse encoding file: {e}")))?;

    info!(
        ckeys = encoding.ckey_count(),
        ekeys = encoding.ekey_count(),
        "encoding file loaded"
    );
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "encoding table".to_string(),
        entries: encoding.ckey_count(),
    });

    // Step 4: Fetch install manifest
    let install_infos = build_config.install();
    let install_info = install_infos.first().ok_or_else(|| {
        InstallationError::Format("build config missing install reference".to_string())
    })?;

    let install_ckey = ContentKey::from_hex(&install_info.content_key)
        .map_err(|e| InstallationError::Format(format!("invalid install content key: {e}")))?;

    let install_ekey = encoding.find_encoding(&install_ckey).ok_or_else(|| {
        InstallationError::NotFound("install manifest encoding key not found".to_string())
    })?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "install manifest".to_string(),
    });
    info!("fetching install manifest");
    let install_data = download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        install_ekey.as_bytes(),
        &archive_lookup_cell,
        &cdn_config,
    )
    .await?;
    let install_decoded = blte_decompress(&install_data, "install manifest", key_store)?;
    let install = InstallManifest::parse(&install_decoded)
        .map_err(|e| InstallationError::Format(format!("failed to parse install manifest: {e}")))?;

    info!(
        entries = install.entries.len(),
        tags = install.tags.len(),
        "install manifest loaded"
    );
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "install manifest".to_string(),
        entries: install.entries.len(),
    });

    // Step 5: Fetch download manifest
    let download_infos = build_config.download();
    let download_info = download_infos.first().ok_or_else(|| {
        InstallationError::Format("build config missing download reference".to_string())
    })?;

    let download_ckey = ContentKey::from_hex(&download_info.content_key)
        .map_err(|e| InstallationError::Format(format!("invalid download content key: {e}")))?;

    let download_ekey = encoding.find_encoding(&download_ckey).ok_or_else(|| {
        InstallationError::NotFound("download manifest encoding key not found".to_string())
    })?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "download manifest".to_string(),
    });
    info!("fetching download manifest");
    let download_data = download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        download_ekey.as_bytes(),
        &archive_lookup_cell,
        &cdn_config,
    )
    .await?;
    let download_decoded = blte_decompress(&download_data, "download manifest", key_store)?;
    let download = DownloadManifest::parse(&download_decoded).map_err(|e| {
        InstallationError::Format(format!("failed to parse download manifest: {e}"))
    })?;

    info!(
        entries = download.entries.len(),
        tags = download.header.tag_count(),
        "download manifest loaded"
    );
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "download manifest".to_string(),
        entries: download.entries.len(),
    });

    // Step 6: Fetch root file
    let root_ckey_hex = build_config.root().ok_or_else(|| {
        InstallationError::Format("build config missing root reference".to_string())
    })?;

    let root_ckey = ContentKey::from_hex(root_ckey_hex)
        .map_err(|e| InstallationError::Format(format!("invalid root content key: {e}")))?;

    let root_ekey = encoding.find_encoding(&root_ckey).ok_or_else(|| {
        InstallationError::NotFound("root file encoding key not found".to_string())
    })?;

    progress(ProgressEvent::ManifestDownloading {
        manifest_type: "root file".to_string(),
    });
    info!("fetching root file");
    let root_data = download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        root_ekey.as_bytes(),
        &archive_lookup_cell,
        &cdn_config,
    )
    .await?;
    let root_decoded = blte_decompress(&root_data, "root file", key_store)?;
    let root = RootFile::parse(&root_decoded)
        .map_err(|e| InstallationError::Format(format!("failed to parse root file: {e}")))?;

    info!(files = root.total_files(), "root file loaded");
    progress(ProgressEvent::ManifestComplete {
        manifest_type: "root file".to_string(),
        entries: root.total_files() as usize,
    });

    // Step 7: Fetch size manifest (optional, not present in all builds)
    let size = fetch_size_manifest(
        &build_config,
        &encoding,
        cdn,
        endpoints,
        &archive_lookup_cell,
        &cdn_config,
        key_store,
    )
    .await;

    // Step 8: Fetch patch index (optional, null hash in many builds)
    let patch_index = fetch_patch_index(
        &build_config,
        &encoding,
        cdn,
        endpoints,
        &archive_lookup_cell,
        &cdn_config,
        key_store,
    )
    .await;

    // Step 9: Fetch patch config for local caching (prevents update dialog)
    let patch_config_data = if let Some(hash) = build_config.patch_config() {
        info!("fetching patch config for local cache");
        let key = hex::decode(hash).unwrap_or_default();
        match download_with_fallback(cdn, endpoints, ContentType::Config, &key).await {
            Ok(data) => Some(data),
            Err(e) => {
                debug!(error = %e, "failed to fetch patch config (non-fatal)");
                None
            }
        }
    } else {
        None
    };

    // Collect raw BLTE bytes for bootstrap files that must be written
    // to local CASC storage. The client expects these indexed in the
    // local IDX so it can resolve content keys at startup.
    // Use the ekeys from the build config / encoding resolution, not
    // MD5(blte_data), because CDN re-encoding can change the hash.
    let bootstrap_blte = vec![
        ("encoding", encoding_ekey.to_string(), encoding_data),
        (
            "install",
            hex::encode(install_ekey.as_bytes()),
            install_data,
        ),
        (
            "download",
            hex::encode(download_ekey.as_bytes()),
            download_data,
        ),
        ("root", hex::encode(root_ekey.as_bytes()), root_data),
    ];

    let manifests = BuildManifests {
        build_config,
        cdn_config,
        encoding,
        root,
        install,
        download,
        size,
        patch_index,
        patch_config_data,
        bootstrap_blte,
    };

    Ok(manifests)
}

/// Fetch the optional size manifest from CDN.
///
/// Returns `None` if the build config has no size reference, the encoding
/// key cannot be resolved, or the download/parse fails.
async fn fetch_size_manifest<S: CdnSource>(
    build_config: &BuildConfig,
    encoding: &EncodingFile,
    cdn: &S,
    endpoints: &[CdnEndpoint],
    archive_lookup_cell: &OnceCell<ArchiveLookup>,
    cdn_config: &CdnConfig,
    key_store: Option<&(dyn TactKeyProvider + Send + Sync)>,
) -> Option<SizeManifest> {
    let size_info = build_config.size()?;
    let size_ckey = ContentKey::from_hex(&size_info.content_key).ok()?;
    let size_ekey = encoding.find_encoding(&size_ckey)?;

    info!("fetching size manifest");
    let size_data = match download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        size_ekey.as_bytes(),
        archive_lookup_cell,
        cdn_config,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("failed to download size manifest: {e}");
            return None;
        }
    };
    let size_decoded = match blte_decompress(&size_data, "size manifest", key_store) {
        Ok(decoded) => decoded,
        Err(e) => {
            tracing::warn!("failed to decompress size manifest: {e}");
            return None;
        }
    };
    match SizeManifest::parse(&size_decoded) {
        Ok(manifest) => {
            debug!(
                entries = manifest.entries.len(),
                tags = manifest.tags.len(),
                "size manifest loaded"
            );
            Some(manifest)
        }
        Err(e) => {
            tracing::warn!("failed to parse size manifest: {e}");
            None
        }
    }
}

/// Fetch the optional patch index from CDN.
///
/// The patch index maps patch blobs to source/target file pairs for delta
/// updates. Returns `None` if the build config has no `patch-index` entry,
/// the encoding key cannot be resolved, or the download/parse fails.
///
/// Blizzard Agent fetches this in `InitPatchIndex` (state 0xc) but skips
/// when the hash is null (all-zero bytes), which is common for fresh installs.
async fn fetch_patch_index<S: CdnSource>(
    build_config: &BuildConfig,
    encoding: &EncodingFile,
    cdn: &S,
    endpoints: &[CdnEndpoint],
    archive_lookup_cell: &OnceCell<ArchiveLookup>,
    cdn_config: &CdnConfig,
    key_store: Option<&(dyn TactKeyProvider + Send + Sync)>,
) -> Option<PatchIndex> {
    let pi_info = build_config.patch_index().into_iter().next()?;

    let pi_ckey = ContentKey::from_hex(&pi_info.content_key).ok()?;
    let pi_ekey = encoding.find_encoding(&pi_ckey)?;

    info!("fetching patch index");
    let pi_data = match download_with_archive_fallback(
        cdn,
        endpoints,
        ContentType::Data,
        pi_ekey.as_bytes(),
        archive_lookup_cell,
        cdn_config,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("failed to download patch index: {e}");
            return None;
        }
    };
    let pi_decoded = match blte_decompress(&pi_data, "patch index", key_store) {
        Ok(decoded) => decoded,
        Err(e) => {
            tracing::warn!("failed to decompress patch index: {e}");
            return None;
        }
    };
    match PatchIndex::parse(&pi_decoded) {
        Ok(pi) => {
            info!(entries = pi.total_entry_count(), "patch index loaded");
            Some(pi)
        }
        Err(e) => {
            tracing::warn!("failed to parse patch index: {e}");
            None
        }
    }
}

/// Parse BLTE data and decompress it, optionally decrypting encrypted chunks.
fn blte_decompress(
    data: &[u8],
    context: &str,
    key_store: Option<&(dyn TactKeyProvider + Send + Sync)>,
) -> InstallationResult<Vec<u8>> {
    let mut cursor = Cursor::new(data);
    let blte = BlteFile::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| {
        InstallationError::Format(format!("failed to parse {context} BLTE container: {e}"))
    })?;
    match key_store {
        Some(keys) => blte.decompress_with_keys(keys),
        None => blte.decompress(),
    }
    .map_err(|e| InstallationError::Format(format!("failed to decompress {context}: {e}")))
}
