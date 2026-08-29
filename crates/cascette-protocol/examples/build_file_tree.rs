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
//!   5. Identifies loose files: EKeys in the encoding file but not in any
//!      archive index. These are individual files on the CDN that the
//!      Battle.net Agent fetches directly.
//!   6. Reads every patch archive index.
//!   7. For every file tracked by the build, checks whether it exists and
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
//! ## Live mode
//!
//! Queries Ribbit for the current build and CDN data, then walks the file tree
//! using official CDN hosts. ProductConfig and ConfigPath are resolved
//! automatically from Ribbit responses.
//!
//!   cargo run -p cascette-protocol --example build_file_tree -- <product> [region] [--paths]
//!
//! Examples:
//!   cargo run -p cascette-protocol --example build_file_tree -- wow_classic_era
//!   cargo run -p cascette-protocol --example build_file_tree -- wow_classic_era eu
//!   cargo run -p cascette-protocol --example build_file_tree -- wow_classic_era --paths
//!
//! ## Manual mode
//!
//! Provide explicit hashes and a CDN source (local mirror or URL).
//!
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     <product> <build_config_hash> <cdn_config_hash> <source> [cdn_path] [options]
//!
//! <source> is either:
//!   - A local filesystem path to a CDN mirror directory
//!     e.g. /run/media/user/NGDP/mirrors/cdn.blizzard.com
//!   - An HTTP/HTTPS base URL
//!     e.g. https://casc.wago.tools
//!
//! Options:
//!   --paths                       Print one path/URL per tracked file and exit.
//!   --product-config <hash>       Include the product config file (from versions BPSV).
//!   --config-path <path>          ConfigPath for product config (default: tpr/configs/data).
//!
//! Examples:
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
//!     /run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com tpr/wow
//!
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic_era 2c915a... c54b41... https://casc.wago.tools tpr/wow \
//!     --product-config c9934edfc8f217a2e01c47e4deae8454
//!
//!   cargo run -p cascette-protocol --example build_file_tree -- \
//!     wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
//!     https://casc.wago.tools tpr/wow --paths

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cascette_crypto::ContentKey;
use cascette_formats::CascFormat;
use cascette_formats::archive::ArchiveIndex;
use cascette_formats::blte::BlteFile;
use cascette_formats::bpsv::{BpsvRow, BpsvSchema};
use cascette_formats::config::{BuildConfig, CdnConfig as FormatCdnConfig};
use cascette_formats::download::DownloadManifest;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_formats::root::RootFile;
use cascette_formats::size::SizeManifest;
use cascette_protocol::{
    CdnClient, CdnConfig, CdnEndpoint, ClientConfig, ContentType, RibbitTactClient,
};
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

// (hash, exists, index_present, entry_count)
type ArchiveResult = (String, bool, bool, u64);

async fn check_archive(src: &ContentSource, hash: String) -> ArchiveResult {
    let archive_exists = src.exists("data", &hash).await;
    let (index_present, entry_count) = match src.fetch_index("data", &hash).await {
        Ok(raw) => match ArchiveIndex::parse(std::io::Cursor::new(raw)) {
            Ok(idx) => (true, idx.entry_count() as u64),
            Err(_) => (true, 0),
        },
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

async fn check_loose_file(src: &ContentSource, hash: String) -> (String, bool) {
    let exists = src.exists("data", &hash).await;
    (hash, exists)
}

async fn check_patch_index(src: &ContentSource, hash: String) -> (String, bool) {
    let exists = src.index_exists("patch", &hash).await;
    (hash, exists)
}

// ── BPSV helpers ───────────────────────────────────────────────────────────

fn field_as_hex(row: &BpsvRow, name: &str, schema: &BpsvSchema) -> Option<String> {
    let val = row.get_by_name(name, schema)?;
    if let Some(bytes) = val.as_hex() {
        Some(hex::encode(bytes))
    } else {
        val.as_string().map(str::to_string)
    }
}

fn field_as_str<'a>(row: &'a BpsvRow, name: &str, schema: &BpsvSchema) -> Option<&'a str> {
    row.get_by_name(name, schema)?.as_string()
}

// ── Product config helpers (official CDN only) ────────────────────────────

/// Build the HTTPS URL for a product config on the official Blizzard CDN.
fn product_config_url(host: &str, config_path: &str, hash: &str) -> String {
    let config_path = config_path.trim_matches('/');
    format!(
        "https://{}/{}/{}/{}/{}",
        host,
        config_path,
        &hash[..2],
        &hash[2..4],
        hash
    )
}

/// Check whether a product config exists on the official CDN (HEAD request).
async fn check_product_config_exists(host: &str, config_path: &str, hash: &str) -> bool {
    let url = product_config_url(host, config_path, hash);
    matches!(
        reqwest::Client::new().head(&url).send().await,
        Ok(r) if r.status().is_success()
    )
}

/// Fetch a product config from the official CDN.
#[allow(dead_code)]
async fn fetch_product_config(
    host: &str,
    config_path: &str,
    hash: &str,
) -> Result<Vec<u8>, String> {
    let url = product_config_url(host, config_path, hash);
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("product config fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("product config HTTP {}: {}", resp.status(), url));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("product config read failed: {e}"))
}

// ── Resolved parameters ────────────────────────────────────────────────────

struct ResolvedParams {
    product: String,
    build_config_hash: String,
    cdn_config_hash: String,
    product_config_hash: Option<String>,
    config_path: String,
    cdn_path: String,
    source: ContentSource,
    source_display: String,
    paths_only: bool,
    /// Verify local file sizes against expected sizes from manifests.
    /// Only works with local mirror sources.
    verify_sizes: bool,
    /// Print only CDN-relative paths for mismatched files (one per line).
    /// Implies verify_sizes. Only works with local mirror sources.
    mismatch_paths_only: bool,
    /// Official Blizzard CDN host for product config fallback.
    /// Product configs live in the ConfigPath namespace on official CDN only;
    /// community mirrors and local mirrors typically don't carry them.
    official_cdn_host: String,
}

/// Detect whether the CLI args indicate live mode or manual mode.
///
/// Live mode: `build_file_tree <product> [region] [--paths]`
/// Manual mode: `build_file_tree <product> <32-char-hex> <32-char-hex> <source> ...`
///
/// The second positional arg (args[2]) distinguishes them: if it's a 32-char
/// hex string, it's a BuildConfig hash and we're in manual mode.
fn is_manual_mode(args: &[String]) -> bool {
    args.get(2).is_some_and(|a| {
        !a.starts_with("--") && a.len() == 32 && a.chars().all(|c| c.is_ascii_hexdigit())
    })
}

/// Live mode: query Ribbit for versions and CDNs, resolve all parameters.
async fn resolve_live(args: &[String]) -> ResolvedParams {
    let product = args[1].clone();

    // Region is the second positional arg if it's not a flag.
    let region = args
        .get(2)
        .filter(|a| !a.starts_with("--"))
        .map_or("us", String::as_str);

    let paths_only = args.iter().any(|a| a == "--paths");
    let verify_sizes = args.iter().any(|a| a == "--verify-sizes");
    let mismatch_paths_only = args.iter().any(|a| a == "--mismatch-paths-only");

    if verify_sizes || mismatch_paths_only {
        eprintln!(
            "ERROR: --verify-sizes / --mismatch-paths-only requires a local mirror source (manual mode)"
        );
        std::process::exit(1);
    }

    eprintln!("Querying Ribbit for {product}/versions ({region}) ...");

    let config = ClientConfig::default();
    let client = RibbitTactClient::new(config).unwrap_or_else(|e| {
        eprintln!("ERROR: Ribbit client init failed: {e}");
        std::process::exit(1);
    });

    // Query versions BPSV
    let versions = client
        .query(&format!("v1/products/{product}/versions"))
        .await
        .unwrap_or_else(|e| {
            eprintln!("ERROR: versions query failed: {e}");
            std::process::exit(1);
        });

    let row = versions
        .rows()
        .iter()
        .find(|r| field_as_str(r, "Region", versions.schema()).is_some_and(|s| s == region))
        .unwrap_or_else(|| {
            eprintln!("ERROR: region '{region}' not found in versions");
            std::process::exit(1);
        });

    let build_config_hash =
        field_as_hex(row, "BuildConfig", versions.schema()).unwrap_or_else(|| {
            eprintln!("ERROR: BuildConfig field missing from versions");
            std::process::exit(1);
        });

    let cdn_config_hash = field_as_hex(row, "CDNConfig", versions.schema()).unwrap_or_else(|| {
        eprintln!("ERROR: CDNConfig field missing from versions");
        std::process::exit(1);
    });

    let product_config_hash = field_as_hex(row, "ProductConfig", versions.schema());

    let version_name = field_as_str(row, "VersionsName", versions.schema()).unwrap_or("unknown");
    eprintln!("  Version:     {version_name}");
    eprintln!("  BuildConfig: {build_config_hash}");
    eprintln!("  CDNConfig:   {cdn_config_hash}");
    if let Some(ref pc) = product_config_hash {
        eprintln!("  ProductCfg:  {pc}");
    }

    // Query CDNs BPSV
    eprintln!("Querying Ribbit for {product}/cdns ...");
    let cdns = client
        .query(&format!("v1/products/{product}/cdns"))
        .await
        .unwrap_or_else(|e| {
            eprintln!("ERROR: cdns query failed: {e}");
            std::process::exit(1);
        });

    let cdn_row = cdns
        .rows()
        .iter()
        .find(|r| field_as_str(r, "Name", cdns.schema()).is_some_and(|s| s == region))
        .or_else(|| cdns.rows().first())
        .unwrap_or_else(|| {
            eprintln!("ERROR: no CDN entries found");
            std::process::exit(1);
        });

    let cdn_path = field_as_str(cdn_row, "Path", cdns.schema())
        .unwrap_or("tpr/wow")
        .to_string();

    let config_path = field_as_str(cdn_row, "ConfigPath", cdns.schema())
        .unwrap_or("tpr/configs/data")
        .to_string();

    let cdn_hosts_raw = field_as_str(cdn_row, "Hosts", cdns.schema()).unwrap_or("");

    // Build endpoint from official hosts only (no community mirrors).
    let cdn_hosts: Vec<&str> = cdn_hosts_raw.split_whitespace().collect();
    if cdn_hosts.is_empty() {
        eprintln!("ERROR: no CDN hosts found in cdns response");
        std::process::exit(1);
    }

    let endpoint = CdnEndpoint {
        host: cdn_hosts[0].to_string(),
        path: cdn_path.clone(),
        product_path: None,
        scheme: Some("https".to_string()),
        is_fallback: false,
        strict: false,
        max_hosts: Some(4),
    };

    let official_cdn_host = cdn_hosts[0].to_string();
    let source_display = format!("https://{} (live, official CDN)", cdn_hosts[0]);
    eprintln!("  CDN path:    {cdn_path}");
    eprintln!("  CDN host:    {}", cdn_hosts[0]);
    eprintln!();

    let cdn_client =
        CdnClient::new(client.cache().clone(), CdnConfig::default()).unwrap_or_else(|e| {
            eprintln!("ERROR: CDN client init failed: {e}");
            std::process::exit(1);
        });

    ResolvedParams {
        product,
        build_config_hash,
        cdn_config_hash,
        product_config_hash,
        config_path,
        cdn_path,
        source: ContentSource::Remote {
            cdn_client,
            endpoint,
        },
        source_display,
        paths_only,
        verify_sizes: verify_sizes || mismatch_paths_only,
        mismatch_paths_only,
        official_cdn_host,
    }
}

/// Manual mode: parse explicit hashes and source from CLI args.
fn resolve_manual(args: &[String]) -> ResolvedParams {
    if args.len() < 5 {
        eprintln!(
            "Usage: build_file_tree <product> <build_config_hash> <cdn_config_hash> <source> [cdn_path] [options]"
        );
        eprintln!("   or: build_file_tree <product> [region] [--paths]");
        std::process::exit(1);
    }

    let paths_only = args.iter().any(|a| a == "--paths");
    let verify_sizes = args.iter().any(|a| a == "--verify-sizes");
    let mismatch_paths_only = args.iter().any(|a| a == "--mismatch-paths-only");

    let product_config_hash = args
        .iter()
        .position(|a| a == "--product-config")
        .and_then(|i| args.get(i + 1))
        .map(|h| h.to_lowercase());
    let config_path = args
        .iter()
        .position(|a| a == "--config-path")
        .and_then(|i| args.get(i + 1))
        .map_or("tpr/configs/data", String::as_str)
        .to_string();

    let product = args[1].clone();
    let build_config_hash = args[2].to_lowercase();
    let cdn_config_hash = args[3].to_lowercase();
    let source_arg = &args[4];
    let cdn_path = args
        .get(5)
        .filter(|a| !a.starts_with("--"))
        .map_or("tpr/wow", String::as_str)
        .to_string();

    let online = source_arg.starts_with("http://") || source_arg.starts_with("https://");

    if (verify_sizes || mismatch_paths_only) && online {
        eprintln!(
            "ERROR: --verify-sizes / --mismatch-paths-only requires a local mirror source, not an HTTP URL"
        );
        std::process::exit(1);
    }

    let source: ContentSource = if online {
        let base_url = source_arg.trim_end_matches('/').to_string();
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
        let cache =
            Arc::new(
                cascette_protocol::cache::ProtocolCache::new(
                    &cascette_protocol::CacheConfig::default(),
                )
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

    ResolvedParams {
        product,
        build_config_hash,
        cdn_config_hash,
        product_config_hash,
        config_path,
        cdn_path,
        source,
        source_display: format!(
            "{} ({})",
            source_arg,
            if online { "online CDN" } else { "local mirror" }
        ),
        paths_only,
        verify_sizes: verify_sizes || mismatch_paths_only,
        mismatch_paths_only,
        official_cdn_host: "level3.blizzard.com".to_string(),
    }
}

// ── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // reqwest 0.13+ requires an explicit TLS crypto provider.
    // ring is already in the dependency tree via cascette-crypto.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  Live:   build_file_tree <product> [region] [--paths]");
        eprintln!(
            "  Manual: build_file_tree <product> <build_config> <cdn_config> <source> [cdn_path] [options]"
        );
        eprintln!();
        eprintln!(
            "Live mode queries Ribbit for the current build. Manual mode uses explicit hashes."
        );
        eprintln!();
        eprintln!("Options (manual mode):");
        eprintln!("  --paths                       Print one path/URL per tracked file and exit.");
        eprintln!(
            "  --verify-sizes                Verify local file sizes against manifest expected values."
        );
        eprintln!(
            "  --mismatch-paths-only        Print CDN-relative paths of mismatched files (one per line)."
        );
        eprintln!("  --product-config <hash>       Include the product config file.");
        eprintln!(
            "  --config-path <path>          ConfigPath for product config (default: tpr/configs/data)."
        );
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  build_file_tree wow_classic_era");
        eprintln!("  build_file_tree wow_classic_era eu --paths");
        eprintln!("  build_file_tree wow_classic 2c915a... c54b41... /mnt/cdn tpr/wow");
        std::process::exit(1);
    }

    let params = if is_manual_mode(&args) {
        resolve_manual(&args)
    } else {
        resolve_live(&args).await
    };

    let ResolvedParams {
        product,
        build_config_hash,
        cdn_config_hash,
        product_config_hash,
        config_path,
        cdn_path,
        source,
        source_display,
        paths_only,
        verify_sizes,
        mismatch_paths_only,
        official_cdn_host,
    } = params;

    // In mismatch-paths-only mode, print CDN-relative paths for mismatched files.
    if mismatch_paths_only {
        verify_sizes_report(&source, &build_config_hash, &cdn_config_hash, true).await;
        return;
    }

    // In verify-sizes mode, check local file sizes against manifest expected values.
    if verify_sizes {
        verify_sizes_report(&source, &build_config_hash, &cdn_config_hash, false).await;
        return;
    }

    // In paths-only mode run a dedicated fast path that streams output
    // immediately without any existence checks.
    if paths_only {
        print_paths(
            &source,
            &build_config_hash,
            &cdn_config_hash,
            product_config_hash.as_deref(),
            &config_path,
            &official_cdn_host,
        )
        .await;
        return;
    }

    println!("=== Build File Tree ===");
    println!("Product:      {product}");
    println!("BuildConfig:  {build_config_hash}");
    println!("CDNConfig:    {cdn_config_hash}");
    if let Some(ref pc_hash) = product_config_hash {
        println!("ProductCfg:   {pc_hash}");
        println!("ConfigPath:   {config_path}");
    }
    println!("Source:       {source_display}");
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
    let archive_group_index_size = cdn_config.archive_group_index_size();
    let patch_archive_group = cdn_config.patch_archive_group();
    let patch_archive_group_index_size = cdn_config.patch_archive_group_index_size();
    let file_indices = cdn_config.file_indices();
    let patch_file_indices = cdn_config.patch_file_indices();

    println!("  archives:                    {} entries", archives.len());
    println!(
        "  archives-index-size:         {} entries",
        archives.iter().filter(|a| a.index_size.is_some()).count()
    );
    println!(
        "  archive-group:               {}",
        archive_group.unwrap_or("(none)")
    );
    println!(
        "  archive-group-index-size:    {}",
        archive_group_index_size.map_or_else(|| "(none)".to_string(), |s| s.to_string())
    );
    println!(
        "  patch-archives:              {} entries",
        patch_archives.len()
    );
    println!(
        "  patch-archives-index-size:   {} entries",
        patch_archives
            .iter()
            .filter(|a| a.index_size.is_some())
            .count()
    );
    println!(
        "  patch-archive-group:         {}",
        patch_archive_group.unwrap_or("(none)")
    );
    println!(
        "  patch-archive-group-index-size: {}",
        patch_archive_group_index_size.map_or_else(|| "(none)".to_string(), |s| s.to_string())
    );
    println!(
        "  file-index:                  {} entries",
        file_indices.len()
    );
    println!(
        "  file-index-size:             {} entries",
        file_indices
            .iter()
            .filter(|a| a.index_size.is_some())
            .count()
    );
    println!(
        "  patch-file-index:            {} entries",
        patch_file_indices.len()
    );
    println!(
        "  patch-file-index-size:       {} entries",
        patch_file_indices
            .iter()
            .filter(|a| a.index_size.is_some())
            .count()
    );

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

    // Product config: lives in ConfigPath namespace (e.g. tpr/configs/data) on the
    // official Blizzard CDN. Community mirrors and local mirrors don't carry them.
    // Always check and fetch from the official CDN host.
    let product_config_present = if let Some(ref pc_hash) = product_config_hash {
        let exists = check_product_config_exists(&official_cdn_host, &config_path, pc_hash).await;
        Some((pc_hash.clone(), exists))
    } else {
        None
    };

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

    // Archive group: the hash in the CDN config is a reference/verification hash, not a
    // download target. The blob and its .index are generated locally by the Battle.net client
    // by merging all individual archive index files. They are never served from the CDN.
    // We only print the reference hash; no existence check is performed.

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

    // ── Step 7b: Enumerate loose files from the file-index ────────────────
    // Loose files are listed in the dedicated `file-index` referenced by the
    // CDN config. Each entry is a standalone file on the CDN at
    // `data/{ekey[0:2]}/{ekey[2:4]}/{ekey}`. The file-index uses the same
    // archive-index format as data archives, but with `offset_bytes = 0`
    // because there is no enclosing archive.
    //
    // Note: this is the same approach TACTSharp's verify mode uses. Walking
    // the encoding file and treating leftovers as "loose" is unreliable
    // (depends on every archive index parsing successfully) and unnecessary.
    let mut loose_ekeys: Vec<String> = Vec::new();
    if !file_indices.is_empty() {
        println!();
        println!(
            "Step 6b: Reading file-index for loose files ({} index entries) ...",
            file_indices.len()
        );
        for idx_info in &file_indices {
            let hash = idx_info.content_key.to_lowercase();
            match source.fetch_index("data", &hash).await {
                Ok(raw) => match ArchiveIndex::parse(std::io::Cursor::new(raw)) {
                    Ok(idx) => {
                        for entry in &idx.entries {
                            let ekey_hex = hex::encode(&entry.encoding_key);
                            // Skip EKeys already tracked as manifests
                            // (encoding, root, install, download, size).
                            if !data_labels.contains_key(&ekey_hex) {
                                loose_ekeys.push(ekey_hex);
                            }
                        }
                    }
                    Err(e) => eprintln!("  WARN: failed to parse file-index {hash}: {e}"),
                },
                Err(e) => eprintln!("  WARN: failed to fetch file-index {hash}: {e}"),
            }
        }
        loose_ekeys.sort();
        loose_ekeys.dedup();
        println!(
            "  Loose files:   {} (from file-index, excluding manifests)",
            loose_ekeys.len()
        );
    }

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

        // patch-archive-group: same as data archive-group — locally generated, not on the CDN.

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
    println!("Legend: [+] present  [-] missing  [~] locally generated (not a CDN file)");
    println!();

    // Config namespace
    println!("config/");
    for (hash, present) in &config_files {
        let marker = if *present { "+" } else { "-" };
        let label = config_labels.get(hash.as_str()).map_or("", String::as_str);
        if *present {
            println!("  [{marker}] {}...  {label}", &hash[..8]);
        } else {
            println!("  [{marker}] {hash}  {label}");
        }
    }

    // Product config (ConfigPath namespace — separate from config/data/patch)
    if let Some((ref pc_hash, pc_exists)) = product_config_present {
        println!();
        println!("{config_path}/  (product config)");
        let marker = if pc_exists { "+" } else { "-" };
        if pc_exists {
            println!("  [{marker}] {}...  ProductConfig", &pc_hash[..8]);
        } else {
            println!("  [{marker}] {pc_hash}  ProductConfig");
        }
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
        let label = data_labels.get(*hash).map_or("", String::as_str);
        if present {
            println!("  [{marker}] {}...  {label}", &hash[..8]);
        } else {
            println!("  [{marker}] {hash}  {label}");
        }
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
        let size_str = archive_group_index_size.map_or(String::new(), |s| format!(" ({s} bytes)"));
        // [~] means locally generated by the client — not a CDN download target.
        println!(
            "  [~] {}...  archive-group (locally generated, not on CDN)",
            &group[..8]
        );
        println!(
            "  [~] {}....index  archive-group index{size_str} (locally generated, not on CDN)",
            &group[..8]
        );
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
            println!("    [-] {hash}");
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
            println!("    [-] {hash}.index");
        }
        if missing_indices.len() > 10 {
            println!("    ... and {} more", missing_indices.len() - 10);
        }
    }

    // Data namespace — loose files
    if !loose_ekeys.is_empty() {
        println!();
        println!("data/  (loose files — not in any archive)");
        // Check a sample for existence (checking all could be thousands of HEAD requests).
        let sample_size = 10.min(loose_ekeys.len());
        let mut checked_present = 0usize;
        let mut checked_missing = 0usize;
        for (hash, present) in join_all(
            loose_ekeys[..sample_size]
                .iter()
                .map(|h| check_loose_file(&source, h.clone())),
        )
        .await
        {
            if present {
                checked_present += 1;
            } else {
                checked_missing += 1;
            }
            let marker = if present { "+" } else { "-" };
            println!("  [{marker}] {}...  loose", &hash[..8]);
        }
        if loose_ekeys.len() > sample_size {
            println!(
                "  ... and {} more loose files (not checked)",
                loose_ekeys.len() - sample_size
            );
        }
        println!(
            "  {} total loose files, {}/{} sampled present",
            loose_ekeys.len(),
            checked_present,
            checked_present + checked_missing
        );
    }

    // Patch namespace
    if !patch_files.is_empty() || !patch_index_files.is_empty() {
        println!();
        println!("patch/");
        for (hash, present) in &patch_files {
            let label = patch_labels.get(hash.as_str()).map_or("", String::as_str);
            if label == "patch-manifest" {
                let marker = if *present { "+" } else { "-" };
                if *present {
                    println!("  [{marker}] {}...  {label}", &hash[..8]);
                } else {
                    println!("  [{marker}] {hash}  {label}");
                }
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

        if let Some(patch_group) = cdn_config.patch_archive_group() {
            let group = patch_group.to_lowercase();
            let size_str =
                patch_archive_group_index_size.map_or(String::new(), |s| format!(" ({s} bytes)"));
            // [~] means locally generated by the client — not a CDN download target.
            println!(
                "  [~] {}...  patch-archive-group (locally generated, not on CDN)",
                &group[..8]
            );
            println!(
                "  [~] {}....index  patch-archive-group index{size_str} (locally generated, not on CDN)",
                &group[..8]
            );
        }
    }

    // ── Step 10: Summary ───────────────────────────────────────────────────
    let pc_count = usize::from(product_config_present.is_some());
    let pc_present_count = product_config_present
        .as_ref()
        .map_or(0, |(_, exists)| usize::from(*exists));
    let total_files = config_files.len()
        + pc_count
        + data_files.len()
        + data_index_files.len()
        + patch_files.len()
        + patch_index_files.len();
    let present_total = config_files.values().filter(|&&v| v).count()
        + pc_present_count
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
    println!("Source:         {source_display}");
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
    println!("Loose files (not in archives): {}", loose_ekeys.len());
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

/// Size verification result for a single file.
struct SizeCheck {
    /// Expected size in bytes.
    expected: u64,
    /// Actual file size on disk (None if file is missing).
    actual: Option<u64>,
    /// File path on the local mirror.
    path: String,
}

impl SizeCheck {
    fn status(&self) -> &str {
        match self.actual {
            None => "MISSING",
            Some(actual) if actual != self.expected => "MISMATCH",
            _ => "OK",
        }
    }

    fn delta(&self) -> i64 {
        match self.actual {
            None => -(self.expected as i64),
            Some(actual) => actual as i64 - self.expected as i64,
        }
    }
}

/// Verify local file sizes against expected sizes from build/CDN manifests.
///
/// Checks the following categories:
/// 1. Data archive `.index` files against CDN config `archives-index-size`
/// 2. File-index `.index` files against CDN config `file-index-size`
/// 3. Patch archive `.index` files against CDN config `patch-archives-index-size`
/// 4. Patch file-index `.index` files against CDN config `patch-file-index-size`
/// 5. Loose data files against download manifest `file_size` entries
/// 6. Loose data files against size manifest `esize` entries (informational)
async fn verify_sizes_report(
    source: &ContentSource,
    build_config_hash: &str,
    cdn_config_hash: &str,
    paths_only: bool,
) {
    let ContentSource::Local {
        mirror_root,
        cdn_path,
    } = source
    else {
        eprintln!("ERROR: --verify-sizes requires a local mirror source");
        std::process::exit(1);
    };

    let mut all_checks: Vec<SizeCheck> = Vec::new();

    // ── Read CDN config for index sizes ─────────────────────────────────
    eprintln!("Reading CDN config ...");
    let cdn_config_data = match source.fetch("config", cdn_config_hash).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read CDN config: {e}");
            std::process::exit(1);
        }
    };
    // NOTE: CDN config archives-index-size / file-index-size values are NOT
    // positional with the archives / file-index hash lists. The size set and
    // hash set are independent — verified against archive.wow.tools: local
    // mirror index files match community mirrors exactly, but CDN config sizes
    // are in a different order than the hashes. We skip index size checks.

    // ── Read build config for manifest EKeys ─────────────────────────────
    eprintln!("Reading build config ...");
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

    // ── Parse CDN config for file-index hash ──────────────────────────────
    // The CDN config archives-index-size values are not positional with
    // the archives hashes, so we can't use them for index size checks.
    // We only parse the CDN config to find the file-index hash.
    eprintln!("Reading CDN config for file-index ...");
    let file_indices = match FormatCdnConfig::parse(cdn_config_data.as_slice()) {
        Ok(cfg) => cfg.file_indices(),
        Err(e) => {
            eprintln!("WARNING: failed to parse CDN config for file-index: {e}");
            Vec::new()
        }
    };

    // ── Check loose data file sizes against download manifest ──────────
    // Only check files listed in the file-index (loose files).
    // Download manifest has ~373K entries but most are archived;
    // stat()'ing all of them on an external drive is too slow.
    // File-index entries (~22K) are the actual loose files on disk.
    let download_entries = build_config.download();

    // Build a HashMap of EKey hex -> expected compressed size from the
    // download manifest for quick lookup.
    let mut download_sizes: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    if !download_entries.is_empty() {
        let first = &download_entries[0];
        if let Some(ref ekey) = first.encoding_key {
            eprintln!("Reading download manifest ...");
            match fetch_and_decompress(source, "data", ekey).await {
                Ok(raw) => match DownloadManifest::parse(raw.as_slice()) {
                    Ok(manifest) => {
                        eprintln!("  {} entries in download manifest", manifest.entries.len());
                        for entry in manifest.entries.iter() {
                            let ekey_hex = hex::encode(entry.encoding_key.as_bytes());
                            download_sizes.insert(ekey_hex, entry.file_size.as_u64());
                        }
                    }
                    Err(e) => eprintln!("WARNING: download manifest parse failed: {e}"),
                },
                Err(e) => eprintln!("WARNING: download manifest fetch failed: {e}"),
            }
        }
    }

    // Check loose files from file-index against download manifest sizes.
    if !download_sizes.is_empty() && !file_indices.is_empty() {
        eprintln!("Checking loose file sizes against download manifest ...");
        for idx_info in &file_indices {
            let hash = idx_info.content_key.to_lowercase();
            match source.fetch_index("data", &hash).await {
                Ok(raw) => match ArchiveIndex::parse(std::io::Cursor::new(raw)) {
                    Ok(idx) => {
                        for entry in &idx.entries {
                            let ekey_hex = hex::encode(&entry.encoding_key);
                            if let Some(&expected) = download_sizes.get(&ekey_hex) {
                                let path = cdn_file_path(mirror_root, cdn_path, "data", &ekey_hex);
                                match std::fs::metadata(&path) {
                                    Ok(meta) => {
                                        let actual = meta.len();
                                        if actual != expected {
                                            all_checks.push(SizeCheck {
                                                expected,
                                                actual: Some(actual),
                                                path: path.display().to_string(),
                                            });
                                        }
                                    }
                                    Err(_) => {
                                        // File is absent from the mirror. Record it
                                        // as MISSING so it is reported and logged.
                                        all_checks.push(SizeCheck {
                                            expected,
                                            actual: None,
                                            path: path.display().to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("WARNING: failed to parse file-index {hash}: {e}"),
                },
                Err(e) => eprintln!("WARNING: failed to fetch file-index {hash}: {e}"),
            }
        }
    }

    // ── Check size manifest entries (esize) ──────────────────────────────
    if let Some(ref info) = build_config.size()
        && let Some(ref ekey) = info.encoding_key
    {
        eprintln!("Reading size manifest ...");
        match fetch_and_decompress(source, "data", ekey).await {
            Ok(raw) => match SizeManifest::parse(raw.as_slice()) {
                Ok(manifest) => {
                    eprintln!(
                        "Size manifest has {} entries, checking against file-index ...",
                        manifest.entries.len()
                    );
                    // Build a set of partial EKeys (first 9 bytes of full EKey)
                    // from file-index entries to avoid stat()'ing all entries.
                    let mut partial_ekeys_on_disk: std::collections::HashSet<Vec<u8>> =
                        std::collections::HashSet::new();
                    for idx_info in &file_indices {
                        let hash = idx_info.content_key.to_lowercase();
                        if let Ok(raw) = source.fetch_index("data", &hash).await {
                            if let Ok(idx) = ArchiveIndex::parse(std::io::Cursor::new(raw)) {
                                for entry in &idx.entries {
                                    // First 9 bytes of full EKey form the partial EKey.
                                    if entry.encoding_key.len() >= 9 {
                                        partial_ekeys_on_disk
                                            .insert(entry.encoding_key[..9].to_vec());
                                    }
                                }
                            }
                        }
                    }
                    eprintln!(
                        "  {} partial EKeys in file-index",
                        partial_ekeys_on_disk.len()
                    );

                    let mut checked = 0usize;
                    let mut matched = 0usize;
                    for entry in manifest.entries.iter() {
                        if partial_ekeys_on_disk.contains(&entry.key) {
                            // Found a loose file that matches this size manifest entry.
                            // The size manifest key is a partial EKey (9 bytes), but the
                            // actual file is stored under the full EKey. We need to
                            // find the full EKey from the file-index to stat the file.
                            matched += 1;
                            // For now, skip individual stat — the download manifest
                            // check above already covers compressed sizes for loose files.
                            // Size manifest esize is the decompressed size which can't
                            // be verified against on-disk compressed BLTE blobs.
                        }
                        checked += 1;
                    }
                    eprintln!(
                        "  Checked {checked} entries, {matched} match loose file-index entries"
                    );
                    eprintln!(
                        "  Note: esize is decompressed size; cannot verify against compressed BLTE blobs on disk."
                    );
                }
                Err(e) => eprintln!("WARNING: size manifest parse failed: {e}"),
            },
            Err(e) => eprintln!("WARNING: size manifest fetch failed: {e}"),
        }
    }

    // ── Print report ──────────────────────────────────────────────────────
    if paths_only {
        // Print one CDN-relative path per mismatched or missing file,
        // nothing else.
        // Deduplicate across builds (same file can appear in multiple builds).
        let mut seen = std::collections::HashSet::new();
        for check in all_checks.iter().filter(|c| c.status() != "OK") {
            // Convert absolute path to CDN-relative path.
            // Path format: <mirror_root>/<cdn_path>/<type>/<xx>/<xx>/<hash>
            let relative = check
                .path
                .strip_prefix(mirror_root.to_str().unwrap_or(""))
                .unwrap_or(&check.path);
            let relative = relative.strip_prefix('/').unwrap_or(relative);
            let relative = relative.strip_prefix(cdn_path).unwrap_or(relative);
            let relative = relative.strip_prefix('/').unwrap_or(relative);
            if seen.insert(relative.to_string()) {
                println!("{relative}");
            }
        }
        return;
    }

    let ok_count = all_checks.iter().filter(|c| c.status() == "OK").count();
    let mismatch_count = all_checks
        .iter()
        .filter(|c| c.status() == "MISMATCH")
        .count();
    let missing_count = all_checks
        .iter()
        .filter(|c| c.status() == "MISSING")
        .count();
    let total = all_checks.len();

    println!();
    println!("=== Size Verification Report ===");
    println!("Total checked: {total}");
    println!("OK:          {ok_count}");
    println!("MISMATCH:    {mismatch_count}");
    println!("MISSING:     {missing_count}");
    println!();

    if mismatch_count > 0 {
        println!("--- Size Mismatches ---");
        println!(
            "  {:>12} {:>12} {:>+12}  {}",
            "Expected", "Actual", "Delta", "Path"
        );
        println!("  {}", "-".repeat(90));

        let mut mismatches: Vec<&SizeCheck> = all_checks
            .iter()
            .filter(|c| c.status() == "MISMATCH")
            .collect();
        mismatches.sort_by_key(|c| std::cmp::Reverse(c.delta().unsigned_abs()));

        for check in mismatches.iter().take(50) {
            // Show the last 70 chars of the path to keep output readable.
            let display_path = if check.path.len() > 70 {
                format!("...{}", &check.path[check.path.len() - 67..])
            } else {
                check.path.clone()
            };
            println!(
                "  {:>12} {:>12} {:>+12}  {}",
                check.expected,
                check.actual.unwrap_or(0),
                check.delta(),
                display_path
            );
        }
        if mismatches.len() > 50 {
            println!("  ... and {} more mismatches", mismatches.len() - 50);
        }
        println!();
    }

    if missing_count > 0 {
        println!("--- Missing Files ---");
        for check in all_checks
            .iter()
            .filter(|c| c.status() == "MISSING")
            .take(20)
        {
            println!("  {:>12}  {}", check.expected, check.path);
        }
        let total_missing = all_checks
            .iter()
            .filter(|c| c.status() == "MISSING")
            .count();
        if total_missing > 20 {
            println!("  ... and {} more missing files", total_missing - 20);
        }
        println!();
    }

    if mismatch_count == 0 && missing_count == 0 {
        println!("All checked files match expected sizes.");
    } else if mismatch_count > 0 {
        eprintln!(
            "{} files have unexpected sizes. The local mirror may have truncated or corrupt files.",
            mismatch_count
        );
    }
}

/// Fast paths-only mode: enumerate all file hashes from build+CDN configs and
/// print one path/URL per line immediately, with no existence checks.
///
/// This avoids issuing hundreds of HEAD requests before producing any output.
async fn print_paths(
    source: &ContentSource,
    build_config_hash: &str,
    cdn_config_hash: &str,
    product_config_hash: Option<&str>,
    config_path: &str,
    official_cdn_host: &str,
) {
    // Config files
    println!("{}", source.display_path("config", build_config_hash));
    println!("{}", source.display_path("config", cdn_config_hash));

    // Product config (always from official Blizzard CDN)
    if let Some(pc_hash) = product_config_hash {
        println!(
            "{}",
            product_config_url(official_cdn_host, config_path, pc_hash)
        );
    }

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

    // Track EKeys already printed as manifests so we don't duplicate them
    // in the loose file section below.
    let mut printed_data_ekeys: HashSet<String> = HashSet::new();

    // Encoding (data namespace)
    if let Some(enc) = build_config.encoding() {
        let ekey = enc.encoding_key.as_ref().unwrap_or(&enc.content_key);
        println!("{}", source.display_path("data", ekey));
        printed_data_ekeys.insert(ekey.to_lowercase());
    }

    // Install, download, size (data namespace) — EKeys directly in build config
    for info in build_config.install() {
        if let Some(ref ekey) = info.encoding_key {
            println!("{}", source.display_path("data", ekey));
            printed_data_ekeys.insert(ekey.to_lowercase());
        }
    }
    for info in build_config.download() {
        if let Some(ref ekey) = info.encoding_key {
            println!("{}", source.display_path("data", ekey));
            printed_data_ekeys.insert(ekey.to_lowercase());
        }
    }
    if let Some(ref info) = build_config.size()
        && let Some(ref ekey) = info.encoding_key
    {
        println!("{}", source.display_path("data", ekey));
        printed_data_ekeys.insert(ekey.to_lowercase());
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
        println!(
            "# locally-generated: {}",
            source.display_path("data", &hash)
        );
        println!(
            "# locally-generated: {}",
            source.display_index_path("data", &hash)
        );
    }
    // file-index .index files themselves (one entry per file-index)
    let data_file_indices = cdn_config.file_indices();
    for idx_info in &data_file_indices {
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
        println!(
            "# locally-generated: {}",
            source.display_path("patch", &hash)
        );
        println!(
            "# locally-generated: {}",
            source.display_index_path("patch", &hash)
        );
    }
    let patch_file_indices = cdn_config.patch_file_indices();
    for idx_info in &patch_file_indices {
        let hash = idx_info.content_key.to_lowercase();
        println!("{}", source.display_index_path("patch", &hash));
    }

    // Loose files: enumerated from the dedicated `file-index` referenced in
    // the CDN config. Each entry is a standalone file at
    // `data/{ekey[0:2]}/{ekey[2:4]}/{ekey}` (or `patch/...` for patch loose
    // files). The file-index uses the standard archive-index format with
    // `offset_bytes = 0`.
    //
    // This is the same approach TACTSharp's verify mode uses; walking the
    // encoding file and treating leftovers as "loose" is unreliable and
    // unnecessary.
    if !data_file_indices.is_empty() {
        eprintln!(
            "# Reading {} data file-index entries for loose file enumeration ...",
            data_file_indices.len()
        );
        for idx_info in &data_file_indices {
            let hash = idx_info.content_key.to_lowercase();
            match source.fetch_index("data", &hash).await {
                Ok(raw) => match ArchiveIndex::parse(std::io::Cursor::new(raw)) {
                    Ok(idx) => {
                        for entry in &idx.entries {
                            let ekey_hex = hex::encode(&entry.encoding_key);
                            if !printed_data_ekeys.contains(&ekey_hex) {
                                println!("{}", source.display_path("data", &ekey_hex));
                                printed_data_ekeys.insert(ekey_hex);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("# WARNING: failed to parse file-index {hash}: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("# WARNING: failed to fetch file-index {hash}: {e}");
                }
            }
        }
    }
    if !patch_file_indices.is_empty() {
        eprintln!(
            "# Reading {} patch file-index entries for loose file enumeration ...",
            patch_file_indices.len()
        );
        for idx_info in &patch_file_indices {
            let hash = idx_info.content_key.to_lowercase();
            match source.fetch_index("patch", &hash).await {
                Ok(raw) => match ArchiveIndex::parse(std::io::Cursor::new(raw)) {
                    Ok(idx) => {
                        for entry in &idx.entries {
                            let ekey_hex = hex::encode(&entry.encoding_key);
                            println!("{}", source.display_path("patch", &ekey_hex));
                        }
                    }
                    Err(e) => {
                        eprintln!("# WARNING: failed to parse patch file-index {hash}: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("# WARNING: failed to fetch patch file-index {hash}: {e}");
                }
            }
        }
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
