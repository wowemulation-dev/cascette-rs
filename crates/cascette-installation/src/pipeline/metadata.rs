//! Metadata resolution: Ribbit -> BuildConfig -> CdnConfig -> manifests.
//!
//! Resolves all manifest files needed for installation from CDN endpoints.

use std::io::Cursor;

use binrw::BinRead;
use tracing::{debug, info};

use cascette_crypto::{ContentKey, TactKeyProvider};
use cascette_formats::CascFormat;
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

    info!(hash = %build_config_hash, "fetching build config");
    let build_config_key = hex::decode(build_config_hash)?;
    let build_config_data =
        download_with_fallback(cdn, endpoints, ContentType::Config, &build_config_key).await?;
    let build_config = BuildConfig::parse(&build_config_data[..])
        .map_err(|e| InstallationError::Format(format!("failed to parse build config: {e}")))?;

    // Step 2: Fetch CDN config
    let cdn_config_hash = config
        .cdn_config
        .as_ref()
        .ok_or_else(|| InstallationError::InvalidConfig("cdn_config hash required".to_string()))?;

    info!(hash = %cdn_config_hash, "fetching CDN config");
    let cdn_config_key = hex::decode(cdn_config_hash)?;
    let cdn_config_data =
        download_with_fallback(cdn, endpoints, ContentType::Config, &cdn_config_key).await?;
    let cdn_config = CdnConfig::parse(&cdn_config_data[..])
        .map_err(|e| InstallationError::Format(format!("failed to parse CDN config: {e}")))?;

    // Step 3: Fetch encoding file
    let encoding_info = build_config.encoding().ok_or_else(|| {
        InstallationError::Format("build config missing encoding reference".to_string())
    })?;

    let encoding_ekey = encoding_info.encoding_key.as_deref().ok_or_else(|| {
        InstallationError::Format("build config missing encoding key".to_string())
    })?;

    info!(ekey = %encoding_ekey, "fetching encoding file");
    let encoding_key_bytes = hex::decode(encoding_ekey)?;
    let encoding_data =
        download_with_fallback(cdn, endpoints, ContentType::Data, &encoding_key_bytes).await?;

    // Encoding file is BLTE-encoded on CDN
    let encoding = EncodingFile::parse_blte(&encoding_data)
        .map_err(|e| InstallationError::Format(format!("failed to parse encoding file: {e}")))?;

    debug!(
        ckeys = encoding.ckey_count(),
        ekeys = encoding.ekey_count(),
        "encoding file loaded"
    );

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

    info!("fetching install manifest");
    let install_data =
        download_with_fallback(cdn, endpoints, ContentType::Data, install_ekey.as_bytes()).await?;
    let install_decoded = blte_decompress(&install_data, "install manifest", key_store)?;
    let install = InstallManifest::parse(&install_decoded)
        .map_err(|e| InstallationError::Format(format!("failed to parse install manifest: {e}")))?;

    debug!(
        entries = install.entries.len(),
        tags = install.tags.len(),
        "install manifest loaded"
    );

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

    info!("fetching download manifest");
    let download_data =
        download_with_fallback(cdn, endpoints, ContentType::Data, download_ekey.as_bytes()).await?;
    let download_decoded = blte_decompress(&download_data, "download manifest", key_store)?;
    let download = DownloadManifest::parse(&download_decoded).map_err(|e| {
        InstallationError::Format(format!("failed to parse download manifest: {e}"))
    })?;

    debug!(entries = download.entries.len(), "download manifest loaded");

    // Step 6: Fetch root file
    let root_ckey_hex = build_config.root().ok_or_else(|| {
        InstallationError::Format("build config missing root reference".to_string())
    })?;

    let root_ckey = ContentKey::from_hex(root_ckey_hex)
        .map_err(|e| InstallationError::Format(format!("invalid root content key: {e}")))?;

    let root_ekey = encoding.find_encoding(&root_ckey).ok_or_else(|| {
        InstallationError::NotFound("root file encoding key not found".to_string())
    })?;

    info!("fetching root file");
    let root_data =
        download_with_fallback(cdn, endpoints, ContentType::Data, root_ekey.as_bytes()).await?;
    let root_decoded = blte_decompress(&root_data, "root file", key_store)?;
    let root = RootFile::parse(&root_decoded)
        .map_err(|e| InstallationError::Format(format!("failed to parse root file: {e}")))?;

    debug!(files = root.total_files(), "root file loaded");

    // Step 7: Fetch size manifest (optional, not present in all builds)
    let size = fetch_size_manifest(&build_config, &encoding, cdn, endpoints, key_store).await;

    // Step 8: Fetch patch index (optional, null hash in many builds)
    let patch_index = fetch_patch_index(&build_config, &encoding, cdn, endpoints, key_store).await;

    let manifests = BuildManifests {
        build_config,
        cdn_config,
        encoding,
        root,
        install,
        download,
        size,
        patch_index,
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
    key_store: Option<&(dyn TactKeyProvider + Send + Sync)>,
) -> Option<SizeManifest> {
    let size_info = build_config.size()?;
    let size_ckey = ContentKey::from_hex(&size_info.content_key).ok()?;
    let size_ekey = encoding.find_encoding(&size_ckey)?;

    info!("fetching size manifest");
    let size_data =
        match download_with_fallback(cdn, endpoints, ContentType::Data, size_ekey.as_bytes()).await
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
    key_store: Option<&(dyn TactKeyProvider + Send + Sync)>,
) -> Option<PatchIndex> {
    let pi_info = build_config.patch_index().into_iter().next()?;

    let pi_ckey = ContentKey::from_hex(&pi_info.content_key).ok()?;
    let pi_ekey = encoding.find_encoding(&pi_ckey)?;

    info!("fetching patch index");
    let pi_data =
        match download_with_fallback(cdn, endpoints, ContentType::Data, pi_ekey.as_bytes()).await {
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
