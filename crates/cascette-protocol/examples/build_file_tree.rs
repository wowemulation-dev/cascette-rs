//! Build file tree: walk a build's CDN content and report presence.
//!
//! Given a product code, build config hash, CDN config hash, and either a
//! local mirror path or a CDN base URL, this example:
//!
//!   1. Reads the build config and CDN config (config namespace).
//!   2. Parses the encoding file (BLTE-compressed, data namespace).
//!   3. Resolves EKeys for all manifests referenced by the build config:
//!      - encoding  (EKey is in build config directly)
//!      - root      (EKey looked up via encoding table, CKey is in build config)
//!      - install   (EKey is in build config directly)
//!      - download  (EKey is in build config directly)
//!      - size      (EKey is in build config directly, optional)
//!      - patch     (EKey is in build config directly, optional, patch namespace)
//!   4. Reads every data archive index to enumerate the full EKey inventory.
//!   5. Reads every patch archive index.
//!   6. For every file tracked by the build, checks whether it exists and
//!      reports its status grouped by CDN namespace.
//!
//! This replicates the file-enumeration part of BuildBackup and TACTSharp:
//! instead of downloading missing files it shows what is present and what
//! is absent, grouped by CDN content type (config / data / patch).
//!
//! Reference tools:
//! - BuildBackup: `BuildBackup <product> <buildconfig> <cdnconfig>`
//! - TACTSharp:   `--mode verify` — walks the same file graph
//! - wow.tools.local: "Check CDN" — file-by-file presence check
//!
//! CDN path layout (local mirror mirrors CDN URL structure exactly):
//!
//! ```text
//! {base}/{cdn_path}/config/{hash[0:2]}/{hash[2:4]}/{full_hash}
//! {base}/{cdn_path}/data/{hash[0:2]}/{hash[2:4]}/{full_hash}
//! {base}/{cdn_path}/data/{hash[0:2]}/{hash[2:4]}/{full_hash}.index
//! {base}/{cdn_path}/patch/{hash[0:2]}/{hash[2:4]}/{full_hash}
//! {base}/{cdn_path}/patch/{hash[0:2]}/{hash[2:4]}/{full_hash}.index
//! ```
//!
//! Usage:
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     <product> <build_config_hash> <cdn_config_hash> <source> [cdn_path] [--paths]
//!
//! <source> is either:
//!   - A local filesystem path to a CDN mirror directory
//!     e.g. /run/media/user/NGDP/mirrors/cdn.blizzard.com
//!   - An HTTP/HTTPS base URL
//!     e.g. https://casc.wago.tools
//!
//! Flags:
//!   --paths   Print one path/URL per tracked file and exit.
//!             In local mode: absolute filesystem paths.
//!             In online mode: full HTTPS URLs.
//!             Suppresses all step/tree output. Suitable for piping.
//!
//! Example (local mirror):
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
//!     /run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com tpr/wow
//!
//! Example (online CDN):
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
//!     https://casc.wago.tools tpr/wow
//!
//! Print all URLs (pipe-friendly, online):
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
//!     https://casc.wago.tools tpr/wow --paths

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cascette_crypto::ContentKey;
use cascette_formats::CascFormat;
use cascette_formats::archive::ArchiveIndex;
use cascette_formats::blte::BlteFile;
use cascette_formats::config::{BuildConfig, CdnConfig as FormatCdnConfig};
use cascette_formats::download::DownloadManifest;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_formats::root::RootFile;
use cascette_formats::size::SizeManifest;
use cascette_protocol::{CdnClient, CdnConfig, CdnEndpoint, ContentType};
use futures::future::join_all;

// ── Content source abstraction ─────────────────────────────────────────────

/// Whether to read from a local mirror directory or a remote CDN base URL.
enum ContentSource {
    /// Local filesystem CDN mirror rooted at `mirror_root`.
    Local {
        mirror_root: PathBuf,
        cdn_path: String,
    },
    /// Remote CDN accessed via HTTP. `base_url` is e.g. `https://casc.wago.tools`.
    Remote {
        cdn_client: CdnClient,
        endpoint: CdnEndpoint,
    },
}

impl ContentSource {
    /// Fetch a file by content type and hex hash string.
    async fn fetch(&self, content_type: &str, hash: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => {
                let path = cdn_file_path(mirror_root, cdn_path, content_type, hash);
                std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))
            }
            Self::Remote {
                cdn_client,
                endpoint,
            } => {
                let ct = match content_type {
                    "config" => ContentType::Config,
                    "data" => ContentType::Data,
                    "patch" => ContentType::Patch,
                    other => return Err(format!("unknown content type: {other}")),
                };
                let key_bytes = hex::decode(hash).map_err(|e| format!("hex decode: {e}"))?;
                cdn_client
                    .download(endpoint, ct, &key_bytes)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// Check whether a file exists (local: path check; remote: HEAD request).
    async fn exists(&self, content_type: &str, hash: &str) -> bool {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => cdn_file_path(mirror_root, cdn_path, content_type, hash).exists(),
            Self::Remote {
                cdn_client,
                endpoint,
            } => {
                let ct = match content_type {
                    "config" => ContentType::Config,
                    "data" => ContentType::Data,
                    "patch" => ContentType::Patch,
                    _ => return false,
                };
                let Ok(key_bytes) = hex::decode(hash) else {
                    return false;
                };
                matches!(
                    cdn_client.get_file_size(endpoint, ct, &key_bytes).await,
                    Ok(Some(_))
                )
            }
        }
    }

    /// Fetch a `.index` file for an archive (data or patch namespace).
    async fn fetch_index(&self, namespace: &str, hash: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => {
                let path = cdn_index_path(mirror_root, cdn_path, namespace, hash);
                std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))
            }
            Self::Remote {
                cdn_client,
                endpoint,
            } => {
                if namespace == "data" {
                    cdn_client
                        .download_archive_index(endpoint, hash)
                        .await
                        .map_err(|e| e.to_string())
                } else {
                    let scheme = endpoint.scheme.as_deref().unwrap_or("https");
                    let base_path = normalize_cdn_path(&endpoint.path);
                    let url = format!(
                        "{}://{}/{}/patch/{}/{}/{}.index",
                        scheme,
                        endpoint.host,
                        base_path,
                        &hash[..2],
                        &hash[2..4],
                        hash
                    );
                    reqwest::get(&url)
                        .await
                        .map_err(|e| e.to_string())?
                        .error_for_status()
                        .map_err(|e| e.to_string())?
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(|e| e.to_string())
                }
            }
        }
    }

    /// Check whether an index file exists.
    async fn index_exists(&self, namespace: &str, hash: &str) -> bool {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => cdn_index_path(mirror_root, cdn_path, namespace, hash).exists(),
            Self::Remote {
                cdn_client,
                endpoint,
            } => {
                if namespace == "data" {
                    matches!(cdn_client.get_index_size(endpoint, hash).await, Ok(Some(_)))
                } else {
                    let scheme = endpoint.scheme.as_deref().unwrap_or("https");
                    let base_path = normalize_cdn_path(&endpoint.path);
                    let url = format!(
                        "{}://{}/{}/patch/{}/{}/{}.index",
                        scheme,
                        endpoint.host,
                        base_path,
                        &hash[..2],
                        &hash[2..4],
                        hash
                    );
                    matches!(reqwest::Client::new().head(&url).send().await, Ok(r) if r.status().is_success())
                }
            }
        }
    }

    /// Build the display path or URL for a file (used by --paths mode).
    fn display_path(&self, content_type: &str, hash: &str) -> String {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => cdn_file_path(mirror_root, cdn_path, content_type, hash)
                .display()
                .to_string(),
            Self::Remote { endpoint, .. } => {
                let scheme = endpoint.scheme.as_deref().unwrap_or("https");
                let base_path = normalize_cdn_path(&endpoint.path);
                format!(
                    "{}://{}/{}/{}/{}/{}/{}",
                    scheme,
                    endpoint.host,
                    base_path,
                    content_type,
                    &hash[..2],
                    &hash[2..4],
                    hash
                )
            }
        }
    }

    /// Build the display path or URL for an index file.
    fn display_index_path(&self, namespace: &str, hash: &str) -> String {
        match self {
            Self::Local {
                mirror_root,
                cdn_path,
            } => cdn_index_path(mirror_root, cdn_path, namespace, hash)
                .display()
                .to_string(),
            Self::Remote { endpoint, .. } => {
                let scheme = endpoint.scheme.as_deref().unwrap_or("https");
                let base_path = normalize_cdn_path(&endpoint.path);
                format!(
                    "{}://{}/{}/{}/{}/{}/{}.index",
                    scheme,
                    endpoint.host,
                    base_path,
                    namespace,
                    &hash[..2],
                    &hash[2..4],
                    hash
                )
            }
        }
    }
}

// ── CDN path helpers ───────────────────────────────────────────────────────

fn normalize_cdn_path(path: &str) -> &str {
    path.trim_matches('/')
}

// ── Archive scan helpers ────────────────────────────────────────────────────

type ArchiveResult = (String, bool, bool, u64); // (hash, exists, index_present, entry_count)

async fn check_archive(src: &ContentSource, hash: String) -> ArchiveResult {
    let archive_exists = src.exists("data", &hash).await;
    let (index_present, entry_count) = match src.fetch_index("data", &hash).await {
        Ok(raw) => {
            let count = ArchiveIndex::parse(std::io::Cursor::new(raw))
                .map_or(0, |idx| idx.entry_count() as u64);
            (true, count)
        }
        Err(_) => (false, 0),
    };
    (hash, archive_exists, index_present, entry_count)
}

async fn check_patch_archive(src: &ContentSource, hash: String) -> (String, bool, bool) {
    let archive_exists = src.exists("patch", &hash).await;
    let index_exists = src.index_exists("patch", &hash).await;
    (hash, archive_exists, index_exists)
}

async fn check_loose_index(src: &ContentSource, hash: String) -> (String, bool) {
    let exists = src.index_exists("data", &hash).await;
    (hash, exists)
}

async fn check_patch_index(src: &ContentSource, hash: String) -> (String, bool) {
    let exists = src.index_exists("patch", &hash).await;
    (hash, exists)
}

// ── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 5 {
        eprintln!(
            "Usage: build_file_tree <product> <build_config_hash> <cdn_config_hash> <source> [cdn_path] [--paths]"
        );
        eprintln!();
        eprintln!("<source>:");
        eprintln!("  Local mirror:  /run/media/user/NGDP/mirrors/cdn.blizzard.com");
        eprintln!("  Online CDN:    https://casc.wago.tools");
        eprintln!();
        eprintln!("Flags:");
        eprintln!("  --paths   Print one path/URL per tracked file and exit.");
        eprintln!();
        eprintln!("Example (local):");
        eprintln!("  build_file_tree wow_classic 2c915a... c54b41... /mnt/cdn tpr/wow");
        eprintln!("Example (online):");
        eprintln!(
            "  build_file_tree wow_classic 2c915a... c54b41... https://casc.wago.tools tpr/wow"
        );
        std::process::exit(1);
    }

    let paths_only = args.iter().any(|a| a == "--paths");

    let product = &args[1];
    let build_config_hash = args[2].to_lowercase();
    let cdn_config_hash = args[3].to_lowercase();
    let source_arg = &args[4];
    let cdn_path = args
        .get(5)
        .filter(|a| *a != "--paths")
        .map_or("tpr/wow", String::as_str)
        .to_string();

    // Detect online vs local mode from the source argument.
    let online = source_arg.starts_with("http://") || source_arg.starts_with("https://");

    let source: ContentSource = if online {
        let base_url = source_arg.trim_end_matches('/').to_string();
        // Strip scheme to get the host; the CdnEndpoint stores host + path separately.
        let host = base_url
            .split_once("://")
            .map_or(source_arg.as_str(), |(_, h)| h)
            .to_string();
        let scheme = base_url.split_once("://").map(|(s, _)| s.to_string());
        let endpoint = CdnEndpoint {
            host,
            path: cdn_path.clone(),
            product_path: None,
            scheme,
            is_fallback: false,
            strict: false,
            max_hosts: None,
        };
        let cache = Arc::new(
            cascette_protocol::cache::ProtocolCache::new(&cascette_protocol::CacheConfig::default())
                .unwrap_or_else(|e| {
                    eprintln!("cache init failed: {e}");
                    std::process::exit(1)
                }),
        );
        let cdn_client = CdnClient::new(cache, CdnConfig::default()).unwrap_or_else(|e| {
            eprintln!("CDN client init failed: {e}");
            std::process::exit(1)
        });
        ContentSource::Remote {
            cdn_client,
            endpoint,
        }
    } else {
        ContentSource::Local {
            mirror_root: PathBuf::from(source_arg),
            cdn_path: cdn_path.clone(),
        }
    };

    // In paths-only mode run a dedicated fast path that streams output
    // immediately without any existence checks.
    if paths_only {
        print_paths(&source, &build_config_hash, &cdn_config_hash).await;
        return;
    }

    println!("=== Build File Tree ===");
    println!("Product:      {product}");
    println!("BuildConfig:  {build_config_hash}");
    println!("CDNConfig:    {cdn_config_hash}");
    println!(
        "Source:       {} ({})",
        source_arg,
        if online { "online CDN" } else { "local mirror" }
    );
    println!("CDN path:     {cdn_path}");
    println!();

    // ── Step 1: Read and parse the build config ────────────────────────────
    println!("Step 1: Reading build config ...");

    let build_config_data = match source.fetch("config", &build_config_hash).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  ERROR: cannot read build config: {e}");
            std::process::exit(1);
        }
    };
    let build_config = match BuildConfig::parse(build_config_data.as_slice()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("  ERROR: failed to parse build config: {e}");
            std::process::exit(1);
        }
    };

    let build_name = build_config.build_name().unwrap_or("(unknown)");
    println!("  Build name: {build_name}");

    // Encoding: CKey + EKey are both in the build config.
    let encoding_info = build_config.encoding().unwrap_or_else(|| {
        eprintln!("  ERROR: build config has no encoding entry");
        std::process::exit(1);
    });
    let encoding_ckey_str = encoding_info.content_key.clone();
    let encoding_ekey_str = encoding_info
        .encoding_key
        .clone()
        .unwrap_or_else(|| encoding_ckey_str.clone());

    // Root: only CKey in build config. EKey must be resolved via encoding table.
    let root_ckey_str = build_config.root().unwrap_or("").to_string();

    // Install manifests: CKey + EKey pairs, stored in data namespace.
    let install_entries = build_config.install();
    // Download manifests: CKey + EKey pairs, stored in data namespace.
    let download_entries = build_config.download();
    // Size manifest: CKey + EKey, stored in data namespace (optional).
    let size_info = build_config.size();
    // Patch manifest: CKey + EKey, stored in patch namespace (optional).
    let patch_info = build_config.patch();

    println!("  Encoding EKey:  {encoding_ekey_str}");
    println!("  Root CKey:      {root_ckey_str}");
    println!("  Install count:  {}", install_entries.len());
    println!("  Download count: {}", download_entries.len());
    println!(
        "  Size manifest:  {}",
        size_info.as_ref().map_or("(none)", |_| "(present)")
    );
    println!(
        "  Patch manifest: {}",
        patch_info.as_ref().map_or("(none)", |_| "(present)")
    );

    // ── Step 2: Read and parse the CDN config ─────────────────────────────
    println!();
    println!("Step 2: Reading CDN config ...");

    let cdn_config_data = match source.fetch("config", &cdn_config_hash).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  ERROR: cannot read CDN config: {e}");
            std::process::exit(1);
        }
    };
    let cdn_config = match FormatCdnConfig::parse(cdn_config_data.as_slice()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("  ERROR: failed to parse CDN config: {e}");
            std::process::exit(1);
        }
    };

    let archives = cdn_config.archives();
    let patch_archives = cdn_config.patch_archives();
    let archive_group = cdn_config.archive_group();
    let file_indices = cdn_config.file_indices();

    println!("  Data archives:   {}", archives.len());
    println!("  Patch archives:  {}", patch_archives.len());
    println!("  Archive group:   {}", archive_group.unwrap_or("(none)"));
    println!("  File indices:    {}", file_indices.len());

    // ── Step 3: Build the config-namespace inventory ───────────────────────
    let mut config_files: BTreeMap<String, bool> = BTreeMap::new();
    let mut config_labels: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    let mut add_config = |hash: &str, label: &str, exists: bool| {
        let h = hash.to_lowercase();
        config_files.insert(h.clone(), exists);
        config_labels.insert(h, label.to_string());
    };

    add_config(
        &build_config_hash,
        "BuildConfig",
        source.exists("config", &build_config_hash).await,
    );
    add_config(
        &cdn_config_hash,
        "CDNConfig",
        source.exists("config", &cdn_config_hash).await,
    );

    for (field, label) in [
        ("patch-config", "PatchConfig"),
        ("keyring", "KeyringConfig"),
    ] {
        if let Some(vals) = build_config.get(field)
            && let Some(h) = vals.first()
            && h.len() == 32
        {
            let exists = source.exists("config", h).await;
            add_config(h, label, exists);
        }
    }

    // ── Step 4: Parse encoding file ───────────────────────────────────────
    println!();
    println!("Step 3: Parsing encoding file ...");

    let mut data_files: BTreeMap<String, bool> = BTreeMap::new();
    let mut data_labels: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut data_index_files: BTreeMap<String, bool> = BTreeMap::new();

    let encoding_exists = source.exists("data", &encoding_ekey_str).await;
    data_files.insert(encoding_ekey_str.clone(), encoding_exists);
    data_labels.insert(encoding_ekey_str.clone(), "encoding".to_string());

    let encoding_file = if encoding_exists {
        match source.fetch("data", &encoding_ekey_str).await {
            Ok(raw) => {
                let result = EncodingFile::parse_blte(raw.as_slice())
                    .or_else(|_| EncodingFile::parse(raw.as_slice()));
                match result {
                    Ok(f) => {
                        println!(
                            "  {} CKey entries, {} EKey entries",
                            f.ckey_count(),
                            f.ekey_count()
                        );
                        Some(f)
                    }
                    Err(e) => {
                        eprintln!("  WARNING: encoding file parse failed: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("  WARNING: encoding file fetch failed: {e}");
                None
            }
        }
    } else {
        eprintln!("  WARNING: encoding file not present (EKey={encoding_ekey_str})");
        None
    };

    let total_ckey_entries = encoding_file.as_ref().map_or(0, EncodingFile::ckey_count);

    // ── Step 5: Resolve manifest EKeys and track data-namespace files ──────
    println!();
    println!("Step 4: Resolving manifest EKeys ...");

    // Root file: CKey -> EKey via encoding table.
    let root_ekey_str = if root_ckey_str.is_empty() {
        None
    } else if let Some(enc) = &encoding_file {
        let ckey_bytes = hex::decode(&root_ckey_str).unwrap_or_default();
        if ckey_bytes.len() == 16 {
            let ckey = ContentKey::from_bytes(ckey_bytes.try_into().unwrap_or([0u8; 16]));
            enc.find_encoding(&ckey)
                .map(|ekey| hex::encode(ekey.as_bytes()))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(ref ekey) = root_ekey_str {
        println!("  Root EKey (via encoding): {ekey}");
        let exists = source.exists("data", ekey).await;
        data_files.insert(ekey.clone(), exists);
        data_labels.insert(ekey.clone(), "root".to_string());
    } else if !root_ckey_str.is_empty() {
        println!("  Root EKey: (could not resolve — encoding file missing or parse failed)");
    } else {
        println!("  Root: (not present in build config)");
    }

    // Install manifests
    for (i, info) in install_entries.iter().enumerate() {
        if let Some(ref ekey) = info.encoding_key {
            let label = if install_entries.len() == 1 {
                "install".to_string()
            } else {
                format!("install[{i}]")
            };
            let exists = source.exists("data", ekey).await;
            data_files.insert(ekey.clone(), exists);
            data_labels.insert(ekey.clone(), label);
            println!("  Install[{i}] EKey: {ekey}");
        }
    }

    // Download manifests
    for (i, info) in download_entries.iter().enumerate() {
        if let Some(ref ekey) = info.encoding_key {
            let label = if download_entries.len() == 1 {
                "download".to_string()
            } else {
                format!("download[{i}]")
            };
            let exists = source.exists("data", ekey).await;
            data_files.insert(ekey.clone(), exists);
            data_labels.insert(ekey.clone(), label);
            println!("  Download[{i}] EKey: {ekey}");
        }
    }

    // Size manifest
    if let Some(ref info) = size_info
        && let Some(ref ekey) = info.encoding_key
    {
        let exists = source.exists("data", ekey).await;
        data_files.insert(ekey.clone(), exists);
        data_labels.insert(ekey.clone(), "size".to_string());
        println!("  Size EKey: {ekey}");
    }

    // ── Step 6: Parse manifests for statistics ─────────────────────────────
    println!();
    println!("Step 5: Parsing manifests ...");

    // Root file
    if let Some(ref ekey) = root_ekey_str {
        if data_files.get(ekey).copied().unwrap_or(false) {
            match fetch_and_decompress(&source, "data", ekey).await {
                Ok(raw) => match RootFile::parse(raw.as_slice()) {
                    Ok(root) => println!(
                        "  Root:     {} total files, {} named",
                        root.total_files(),
                        root.named_files()
                    ),
                    Err(e) => eprintln!("  Root parse failed: {e}"),
                },
                Err(e) => eprintln!("  Root fetch/decompress failed: {e}"),
            }
        } else {
            println!("  Root:     (not present)");
        }
    }

    // Install manifests
    for (i, info) in install_entries.iter().enumerate() {
        if let Some(ref ekey) = info.encoding_key {
            if data_files.get(ekey).copied().unwrap_or(false) {
                match fetch_and_decompress(&source, "data", ekey).await {
                    Ok(raw) => match InstallManifest::parse(raw.as_slice()) {
                        Ok(manifest) => {
                            let stats = manifest.stats();
                            println!(
                                "  Install[{i}]: {} files, {} tags, {} bytes",
                                stats.total_files, stats.total_tags, stats.total_size
                            );
                        }
                        Err(e) => eprintln!("  Install[{i}] parse failed: {e}"),
                    },
                    Err(e) => eprintln!("  Install[{i}] fetch/decompress failed: {e}"),
                }
            } else {
                println!("  Install[{i}]: (not present)");
            }
        }
    }

    // Download manifests
    for (i, info) in download_entries.iter().enumerate() {
        if let Some(ref ekey) = info.encoding_key {
            if data_files.get(ekey).copied().unwrap_or(false) {
                match fetch_and_decompress(&source, "data", ekey).await {
                    Ok(raw) => match DownloadManifest::parse(raw.as_slice()) {
                        Ok(manifest) => {
                            let stats = manifest.stats();
                            println!(
                                "  Download[{i}]: {} entries, {} bytes total",
                                stats.entry_count, stats.total_size
                            );
                        }
                        Err(e) => eprintln!("  Download[{i}] parse failed: {e}"),
                    },
                    Err(e) => eprintln!("  Download[{i}] fetch/decompress failed: {e}"),
                }
            } else {
                println!("  Download[{i}]: (not present)");
            }
        }
    }

    // Size manifest
    if let Some(ref info) = size_info
        && let Some(ref ekey) = info.encoding_key
    {
        if data_files.get(ekey).copied().unwrap_or(false) {
            match fetch_and_decompress(&source, "data", ekey).await {
                Ok(raw) => match SizeManifest::parse(raw.as_slice()) {
                    Ok(manifest) => println!(
                        "  Size manifest: {} entries, {} bytes total",
                        manifest.header.num_files, manifest.header.total_size,
                    ),
                    Err(e) => eprintln!("  Size manifest parse failed: {e}"),
                },
                Err(e) => eprintln!("  Size manifest fetch/decompress failed: {e}"),
            }
        } else {
            println!("  Size manifest: (not present)");
        }
    }

    // ── Step 7: Walk data archive indices ──────────────────────────────────
    println!();
    println!(
        "Step 6: Scanning data archive indices ({} archives) ...",
        archives.len()
    );

    // Run all archive checks concurrently: exists + fetch_index in one future per archive.
    let archive_results: Vec<ArchiveResult> = join_all(
        archives
            .iter()
            .map(|a| check_archive(&source, a.content_key.to_lowercase())),
    )
    .await;

    let mut total_index_entries: u64 = 0;
    let mut readable_indices: u32 = 0;
    for (hash, archive_exists, index_present, entry_count) in archive_results {
        let archive_label = format!("archive:{}", &hash[..8]);
        data_files.insert(hash.clone(), archive_exists);
        data_labels.entry(hash.clone()).or_insert(archive_label);
        data_index_files.insert(hash.clone(), index_present);
        if index_present {
            total_index_entries += entry_count;
            readable_indices += 1;
        }
    }

    // Archive group (single entry, no need to parallelize)
    if let Some(group_hash) = archive_group {
        let group = group_hash.to_lowercase();
        let group_exists = source.exists("data", &group).await;
        let group_index_exists = source.index_exists("data", &group).await;
        data_files.insert(group.clone(), group_exists);
        data_labels.insert(group.clone(), "archive-group".to_string());
        data_index_files.insert(group.clone(), group_index_exists);
    }

    // Loose file indices
    for (hash, exists) in join_all(
        file_indices
            .iter()
            .map(|idx_info| check_loose_index(&source, idx_info.content_key.to_lowercase())),
    )
    .await
    {
        data_index_files.insert(hash, exists);
    }

    println!(
        "  Archives:      {} total, {} indices readable",
        archives.len(),
        readable_indices
    );
    println!("  Index entries: {total_index_entries}");

    // ── Step 8: Walk patch archives ────────────────────────────────────────
    let mut patch_files: BTreeMap<String, bool> = BTreeMap::new();
    let mut patch_labels: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut patch_index_files: BTreeMap<String, bool> = BTreeMap::new();

    // Patch manifest (from build config): lives in patch namespace, not data.
    if let Some(ref info) = patch_info
        && let Some(ref ekey) = info.encoding_key
    {
        let exists = source.exists("patch", ekey).await;
        patch_files.insert(ekey.clone(), exists);
        patch_labels.insert(ekey.clone(), "patch-manifest".to_string());
        println!();
        println!("Step 7: Tracking patch manifest ...");
        println!("  Patch manifest EKey: {ekey}");
    }

    if !patch_archives.is_empty() {
        println!();
        println!(
            "Step 8: Scanning patch archive indices ({} archives) ...",
            patch_archives.len()
        );

        for (hash, archive_exists, index_exists) in join_all(
            patch_archives
                .iter()
                .map(|patch| check_patch_archive(&source, patch.content_key.to_lowercase())),
        )
        .await
        {
            patch_files.insert(hash.clone(), archive_exists);
            patch_labels
                .entry(hash.clone())
                .or_insert_with(|| format!("patch-archive:{}", &hash[..8]));
            patch_index_files.insert(hash, index_exists);
        }

        if let Some(patch_group) = cdn_config.patch_archive_group() {
            let hash = patch_group.to_lowercase();
            let exists = source.exists("patch", &hash).await;
            let index_exists = source.index_exists("patch", &hash).await;
            patch_files.insert(hash.clone(), exists);
            patch_labels.insert(hash.clone(), "patch-archive-group".to_string());
            patch_index_files.insert(hash.clone(), index_exists);
        }

        for (hash, exists) in join_all(
            cdn_config
                .patch_file_indices()
                .iter()
                .map(|idx_info| check_patch_index(&source, idx_info.content_key.to_lowercase())),
        )
        .await
        {
            patch_index_files.insert(hash, exists);
        }

        let present_patch_archives = patch_files.values().filter(|&&v| v).count();
        println!(
            "  Patch archives: {} total, {} present",
            patch_archives.len(),
            present_patch_archives
        );
    }

    // ── Step 9: Print the file tree ────────────────────────────────────────
    println!();
    println!("=== File Tree ===");
    println!("Legend: [+] present  [-] missing");
    println!();

    // Config namespace
    println!("config/");
    for (hash, present) in &config_files {
        let marker = if *present { "+" } else { "-" };
        let short = &hash[..8];
        let label = config_labels.get(hash.as_str()).map_or("", String::as_str);
        println!("  [{marker}] {short}...  {label}");
    }

    // Data namespace — manifests
    println!();
    println!("data/");
    let manifest_keys: Vec<&String> = data_files
        .keys()
        .filter(|h| {
            data_labels
                .get(*h)
                .is_some_and(|l| !l.starts_with("archive:"))
        })
        .collect();
    for hash in &manifest_keys {
        let present = data_files[*hash];
        let marker = if present { "+" } else { "-" };
        let short = &hash[..8];
        let label = data_labels.get(*hash).map_or("", String::as_str);
        println!("  [{marker}] {short}...  {label}");
    }

    let present_archives = data_files
        .iter()
        .filter(|(h, _)| {
            data_labels
                .get(*h)
                .is_some_and(|l| l.starts_with("archive:"))
        })
        .filter(|(_, v)| **v)
        .count();
    let total_archives_count = data_files
        .keys()
        .filter(|h| {
            data_labels
                .get(*h)
                .is_some_and(|l| l.starts_with("archive:"))
        })
        .count();

    println!();
    println!("data/  (archives + archive-group)");
    println!("  {present_archives}/{total_archives_count} archives present");

    if let Some(group_hash) = archive_group {
        let group = group_hash.to_lowercase();
        let group_present = data_files.get(&group).copied().unwrap_or(false);
        let group_index_present = data_index_files.get(&group).copied().unwrap_or(false);
        let gm = if group_present { "+" } else { "-" };
        let gim = if group_index_present { "+" } else { "-" };
        println!("  [{gm}] {}...  archive-group", &group[..8]);
        println!("  [{gim}] {}....index  archive-group index", &group[..8]);
    }

    let missing_archives: Vec<&String> = data_files
        .iter()
        .filter(|(h, v)| {
            !**v && data_labels
                .get(*h)
                .is_some_and(|l| l.starts_with("archive:"))
        })
        .map(|(h, _)| h)
        .collect();
    if !missing_archives.is_empty() {
        println!("  Missing archives ({}):", missing_archives.len());
        for hash in missing_archives.iter().take(10) {
            println!("    [-] {}...", &hash[..8]);
        }
        if missing_archives.len() > 10 {
            println!("    ... and {} more", missing_archives.len() - 10);
        }
    }

    // Data namespace — indices
    println!();
    println!("data/  (indices)");
    let present_indices = data_index_files.values().filter(|&&v| v).count();
    let total_indices = data_index_files.len();
    println!("  {present_indices}/{total_indices} index files present");
    let missing_indices: Vec<&String> = data_index_files
        .iter()
        .filter(|&(_, &v)| !v)
        .map(|(h, _)| h)
        .collect();
    if !missing_indices.is_empty() {
        println!("  Missing ({}):", missing_indices.len());
        for hash in missing_indices.iter().take(10) {
            println!("    [-] {}....index", &hash[..8]);
        }
        if missing_indices.len() > 10 {
            println!("    ... and {} more", missing_indices.len() - 10);
        }
    }

    // Patch namespace
    if !patch_files.is_empty() || !patch_index_files.is_empty() {
        println!();
        println!("patch/");
        for (hash, present) in &patch_files {
            let label = patch_labels.get(hash.as_str()).map_or("", String::as_str);
            if label == "patch-manifest" {
                let marker = if *present { "+" } else { "-" };
                println!("  [{marker}] {}...  {label}", &hash[..8]);
            }
        }
        let present_patches = patch_files.values().filter(|&&v| v).count();
        let total_patches = patch_files.len();
        println!("  {present_patches}/{total_patches} patch files present (archives + manifest)");

        println!();
        println!("patch/  (indices)");
        let present_pidx = patch_index_files.values().filter(|&&v| v).count();
        let total_pidx = patch_index_files.len();
        println!("  {present_pidx}/{total_pidx} patch index files present");
    }

    // ── Step 10: Summary ───────────────────────────────────────────────────
    let total_files = config_files.len()
        + data_files.len()
        + data_index_files.len()
        + patch_files.len()
        + patch_index_files.len();
    let present_total = config_files.values().filter(|&&v| v).count()
        + data_files.values().filter(|&&v| v).count()
        + data_index_files.values().filter(|&&v| v).count()
        + patch_files.values().filter(|&&v| v).count()
        + patch_index_files.values().filter(|&&v| v).count();
    let missing_total = total_files - present_total;

    println!();
    println!("=== Summary ===");
    println!("Product:        {product}");
    println!("Build:          {build_name}");
    println!("BuildConfig:    {build_config_hash}");
    println!("CDNConfig:      {cdn_config_hash}");
    println!(
        "Source:         {} ({})",
        source_arg,
        if online { "online CDN" } else { "local mirror" }
    );
    println!();
    println!(
        "CKey entries in encoding:  {}",
        if total_ckey_entries > 0 {
            total_ckey_entries.to_string()
        } else {
            "(encoding not read)".to_string()
        }
    );
    println!("Archive index entries:     {total_index_entries}");
    println!();
    println!("Files tracked by build:    {total_files}");
    println!("Files present:             {present_total}");
    println!("Files missing:             {missing_total}");

    if missing_total == 0 {
        println!();
        println!("All files present.");
    } else {
        #[allow(clippy::cast_precision_loss)]
        let pct = present_total as f64 / total_files as f64 * 100.0;
        println!();
        println!("Coverage: {pct:.1}%");
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Fast paths-only mode: enumerate all file hashes from build+CDN configs and
/// print one path/URL per line immediately, with no existence checks.
///
/// This avoids issuing hundreds of HEAD requests before producing any output.
async fn print_paths(source: &ContentSource, build_config_hash: &str, cdn_config_hash: &str) {
    // Config files
    println!("{}", source.display_path("config", build_config_hash));
    println!("{}", source.display_path("config", cdn_config_hash));

    // Build config: needed to find all other manifest EKeys.
    let build_config_data = match source.fetch("config", build_config_hash).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read build config: {e}");
            std::process::exit(1);
        }
    };
    let build_config = match BuildConfig::parse(build_config_data.as_slice()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("ERROR: failed to parse build config: {e}");
            std::process::exit(1);
        }
    };

    // Optional config files referenced by build config
    for field in ["patch-config", "keyring"] {
        if let Some(vals) = build_config.get(field)
            && let Some(h) = vals.first()
            && h.len() == 32
        {
            println!("{}", source.display_path("config", h));
        }
    }

    // Encoding (data namespace)
    if let Some(enc) = build_config.encoding() {
        let ekey = enc.encoding_key.as_ref().unwrap_or(&enc.content_key);
        println!("{}", source.display_path("data", ekey));
    }

    // Install, download, size (data namespace) — EKeys directly in build config
    for info in build_config.install() {
        if let Some(ref ekey) = info.encoding_key {
            println!("{}", source.display_path("data", ekey));
        }
    }
    for info in build_config.download() {
        if let Some(ref ekey) = info.encoding_key {
            println!("{}", source.display_path("data", ekey));
        }
    }
    if let Some(ref info) = build_config.size()
        && let Some(ref ekey) = info.encoding_key
    {
        println!("{}", source.display_path("data", ekey));
    }

    // Patch manifest (patch namespace)
    if let Some(ref info) = build_config.patch()
        && let Some(ref ekey) = info.encoding_key
    {
        println!("{}", source.display_path("patch", ekey));
    }

    // CDN config: needed to enumerate archives and patch archives.
    let cdn_config_data = match source.fetch("config", cdn_config_hash).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read CDN config: {e}");
            std::process::exit(1);
        }
    };
    let cdn_config = match FormatCdnConfig::parse(cdn_config_data.as_slice()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("ERROR: failed to parse CDN config: {e}");
            std::process::exit(1);
        }
    };

    // Data archives + indices
    for archive in cdn_config.archives() {
        let hash = archive.content_key.to_lowercase();
        println!("{}", source.display_path("data", &hash));
        println!("{}", source.display_index_path("data", &hash));
    }
    if let Some(group) = cdn_config.archive_group() {
        let hash = group.to_lowercase();
        println!("{}", source.display_path("data", &hash));
        println!("{}", source.display_index_path("data", &hash));
    }
    for idx_info in cdn_config.file_indices() {
        let hash = idx_info.content_key.to_lowercase();
        println!("{}", source.display_index_path("data", &hash));
    }

    // Patch archives + indices
    for patch in cdn_config.patch_archives() {
        let hash = patch.content_key.to_lowercase();
        println!("{}", source.display_path("patch", &hash));
        println!("{}", source.display_index_path("patch", &hash));
    }
    if let Some(group) = cdn_config.patch_archive_group() {
        let hash = group.to_lowercase();
        println!("{}", source.display_path("patch", &hash));
        println!("{}", source.display_index_path("patch", &hash));
    }
    for idx_info in cdn_config.patch_file_indices() {
        let hash = idx_info.content_key.to_lowercase();
        println!("{}", source.display_index_path("patch", &hash));
    }
}

/// Fetch a file and BLTE-decompress it.
async fn fetch_and_decompress(
    source: &ContentSource,
    content_type: &str,
    hash: &str,
) -> Result<Vec<u8>, String> {
    let raw = source.fetch(content_type, hash).await?;
    let blte = BlteFile::parse(raw.as_slice()).map_err(|e| format!("BLTE parse: {e}"))?;
    blte.decompress()
        .map_err(|e| format!("BLTE decompress: {e}"))
}

/// Build the filesystem path for a CDN file.
fn cdn_file_path(mirror_root: &Path, cdn_path: &str, content_type: &str, hash: &str) -> PathBuf {
    let hash = hash.to_lowercase();
    mirror_root
        .join(cdn_path)
        .join(content_type)
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(&hash)
}

/// Build the filesystem path for a CDN index file (same as data path + `.index`).
fn cdn_index_path(mirror_root: &Path, cdn_path: &str, content_type: &str, hash: &str) -> PathBuf {
    let base = cdn_file_path(mirror_root, cdn_path, content_type, hash);
    let mut s = base.into_os_string();
    s.push(".index");
    PathBuf::from(s)
}
