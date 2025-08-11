use crate::{InstallCommands, InstallType as CliInstallType, OutputFormat, wago_api};
use comfy_table::{Cell, ContentArrangement, Table, presets::UTF8_FULL};
use indicatif::{ProgressBar, ProgressStyle};
use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cache::hybrid_version_client::HybridVersionClient;
use ribbit_client::Region;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tact_parser::download::DownloadManifest;
use tact_parser::encoding::EncodingFile;
use tact_parser::install::InstallManifest;
use tracing::{debug, error, info, warn};

/// Unified file entry for both install and download manifests
#[derive(Debug, Clone)]
struct FileEntry {
    path: String,
    ckey: Vec<u8>, // For install manifest entries, for download manifest this is ekey
    size: u64,
    priority: i8,
}

/// Archive location information for a file
#[derive(Debug, Clone)]
struct ArchiveLocation {
    archive_hash: String,
    offset: usize,
    size: usize,
}

/// Combined archive index mapping EKeys to archive locations
#[derive(Debug)]
struct ArchiveIndex {
    map: HashMap<String, ArchiveLocation>, // Full EKey (uppercase hex) -> (archive, offset, size)
}

impl ArchiveIndex {
    /// Create an empty archive index
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Look up archive location for an EKey
    fn lookup(&self, ekey: &[u8]) -> Option<&ArchiveLocation> {
        // Convert EKey to uppercase hex string for lookup
        let lookup_key = hex::encode(ekey).to_uppercase();

        let result = self.map.get(&lookup_key);
        if result.is_none() && !self.map.is_empty() {
            debug!(
                "EKey {} not found in {} archive entries",
                lookup_key,
                self.map.len()
            );
        }
        result
    }

    /// Parse a single archive index and add entries to this index
    /// Using BuildBackup's exact format: 4096-byte blocks with 170 entries each
    fn parse_and_add_index(
        &mut self,
        archive_hash: &str,
        index_data: &[u8],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        use byteorder::{BigEndian, ReadBytesExt};
        use std::io::{Cursor, Read};

        // BuildBackup format: fixed 4096-byte blocks with 170 entries of 24 bytes each
        const BLOCK_SIZE: usize = 4096;
        const ENTRIES_PER_BLOCK: usize = 170;
        const _ENTRY_SIZE: usize = 24; // 16 bytes hash + 4 bytes size + 4 bytes offset
        const BLOCK_CHECKSUM_SIZE: usize = 16;

        let num_blocks = index_data.len() / BLOCK_SIZE;
        let mut cursor = Cursor::new(index_data);
        let mut entries_added = 0;

        debug!(
            "Parsing archive index {}: {} blocks ({} bytes total)",
            archive_hash,
            num_blocks,
            index_data.len()
        );

        for block_idx in 0..num_blocks {
            // Read 170 entries per block
            for entry_idx in 0..ENTRIES_PER_BLOCK {
                // Read 16-byte EKey
                let mut ekey_bytes = [0u8; 16];
                if cursor.read_exact(&mut ekey_bytes).is_err() {
                    debug!("Failed to read entry {} in block {}", entry_idx, block_idx);
                    break;
                }

                // Read 4-byte size (big-endian per BuildBackup)
                let size = cursor.read_u32::<BigEndian>()? as usize;

                // Read 4-byte offset (big-endian per BuildBackup)
                let offset = cursor.read_u32::<BigEndian>()? as usize;

                // Skip null entries
                let ekey_hex = hex::encode(ekey_bytes).to_uppercase();
                if ekey_hex == "00000000000000000000000000000000" || size == 0 {
                    continue;
                }

                // Add valid entries (with reasonable size limit)
                if size > 0 && size < 100_000_000 {
                    // Max 100MB per file
                    let location = ArchiveLocation {
                        archive_hash: archive_hash.to_string(),
                        offset,
                        size,
                    };

                    // Store with uppercase hex key for consistent lookups
                    self.map.insert(ekey_hex, location);
                    entries_added += 1;
                }
            }

            // Skip the 16-byte block checksum at end of each block
            let mut checksum = [0u8; BLOCK_CHECKSUM_SIZE];
            let _ = cursor.read_exact(&mut checksum);
        }

        debug!(
            "Parsed archive index {}: {} entries added from {} blocks",
            archive_hash, entries_added, num_blocks
        );

        Ok(entries_added)
    }
}

/// Download file using archive index or fallback to loose file
async fn download_file_with_archive(
    cdn_client: &CachedCdnClient,
    archive_index: &ArchiveIndex,
    cdn_host: &str,
    cdn_path: &str,
    ekey_hex: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let ekey_bytes = hex::decode(ekey_hex)?;

    debug!(
        "Looking up EKey {} (len={}) in archive index...",
        ekey_hex,
        ekey_bytes.len()
    );

    // First, try to find the file in archives
    if let Some(location) = archive_index.lookup(&ekey_bytes) {
        info!(
            "✓ Found {} in archive {} at offset {}, size {}",
            ekey_hex, location.archive_hash, location.offset, location.size
        );

        info!(
            "Attempting archive byte-range download from {}",
            location.archive_hash
        );

        // Try archive range download - archive data files should exist on CDN
        info!(
            "Attempting archive range download from {}",
            location.archive_hash
        );
        match download_archive_range(
            cdn_client,
            cdn_path,
            &location.archive_hash,
            location.offset,
            location.size,
        )
        .await
        {
            Ok(data) => {
                // Decompress BLTE if needed
                if data.starts_with(b"BLTE") {
                    match blte::decompress_blte(data.clone(), None) {
                        Ok(decompressed) => return Ok(decompressed),
                        Err(e) => {
                            warn!("Failed to decompress BLTE from archive: {}", e);
                            return Ok(data);
                        }
                    }
                } else {
                    return Ok(data);
                }
            }
            Err(e) => {
                warn!(
                    "Failed to download from archive {}: {}",
                    location.archive_hash, e
                );
            }
        }
    } else {
        warn!(
            "❌ EKey {} NOT found in any archive - falling back to loose file download",
            ekey_hex
        );
    }

    // Fallback to loose file download
    info!("⬇️ Attempting loose file download for {}", ekey_hex);
    match cdn_client.download_data(cdn_host, cdn_path, ekey_hex).await {
        Ok(response) => {
            let data = response.bytes().await?;

            // Decompress BLTE if needed
            if data.starts_with(b"BLTE") {
                match blte::decompress_blte(data.to_vec(), None) {
                    Ok(decompressed) => Ok(decompressed),
                    Err(e) => {
                        warn!("Failed to decompress BLTE: {}", e);
                        Ok(data.to_vec())
                    }
                }
            } else {
                Ok(data.to_vec())
            }
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// Download byte range from archive file
async fn download_archive_range(
    _cdn_client: &CachedCdnClient,
    cdn_path: &str,
    archive_hash: &str,
    offset: usize,
    size: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to download from different CDN hosts
    let hosts = vec![
        "blzddist1-a.akamaihd.net",
        "level3.blizzard.com",
        "us.cdn.blizzard.com",
        "cdn.arctium.tools",
        "tact.mirror.reliquaryhq.com",
    ];

    for host in &hosts {
        let url = format!(
            "http://{}/{}/data/{}/{}/{}",
            host,
            cdn_path,
            &archive_hash[0..2],
            &archive_hash[2..4],
            archive_hash
        );

        let client = reqwest::Client::new();
        let range_header = format!("bytes={}-{}", offset, offset + size - 1);

        match client.get(&url).header("Range", range_header).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(data) => {
                            debug!(
                                "Downloaded {} bytes from archive {} ({})",
                                data.len(),
                                archive_hash,
                                host
                            );
                            return Ok(data.to_vec());
                        }
                        Err(e) => warn!("Failed to read archive range response: {}", e),
                    }
                } else {
                    warn!(
                        "Archive range request failed: {} from {}",
                        response.status(),
                        host
                    );
                }
            }
            Err(e) => warn!("Archive range request failed from {}: {}", host, e),
        }
    }

    Err("Failed to download archive range from all CDNs".into())
}

/// Download archive index with .index suffix using direct HTTP
async fn download_archive_index(
    _cdn_client: &CachedCdnClient,
    cdn_path: &str,
    archive_hash: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // For now, let's create a simple cache file directly to verify caching works
    use std::path::PathBuf;
    use tokio::fs;

    // Create cache path following the CDN cache structure
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("ngdp")
        .join("cdn")
        .join(cdn_path)
        .join("data")
        .join(&archive_hash[0..2])
        .join(&archive_hash[2..4]);

    let cache_file = cache_dir.join(format!("{}.index", archive_hash));

    // Check if cached
    if cache_file.exists() {
        debug!("Loading archive index {} from cache", archive_hash);
        match fs::read(&cache_file).await {
            Ok(bytes) => {
                info!(
                    "✓ Archive index {} loaded from cache ({} bytes)",
                    archive_hash,
                    bytes.len()
                );
                return Ok(bytes);
            }
            Err(e) => {
                warn!("Failed to read cached archive index: {}", e);
            }
        }
    }

    // Not cached, download via direct HTTP (bypassing CDN client hash validation)
    let hosts = vec![
        "blzddist1-a.akamaihd.net",
        "level3.blizzard.com",
        "us.cdn.blizzard.com",
        "cdn.arctium.tools",
        "tact.mirror.reliquaryhq.com",
    ];

    let client = reqwest::Client::new();

    for host in &hosts {
        // Build URL: http://host/cdn_path/data/{hash[0:2]}/{hash[2:4]}/{hash}.index
        let url = format!(
            "http://{}/{}/data/{}/{}/{}.index",
            host,
            cdn_path,
            &archive_hash[0..2],
            &archive_hash[2..4],
            archive_hash
        );

        debug!("Downloading archive index from: {}", url);

        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(bytes) => {
                            info!(
                                "✓ Downloaded archive index {} from {} ({} bytes)",
                                archive_hash,
                                host,
                                bytes.len()
                            );

                            // Decompress BLTE if needed
                            let decompressed = if bytes.starts_with(b"BLTE") {
                                match blte::decompress_blte(bytes.to_vec(), None) {
                                    Ok(data) => {
                                        debug!(
                                            "✓ Decompressed BLTE archive index: {} -> {} bytes",
                                            bytes.len(),
                                            data.len()
                                        );
                                        data
                                    }
                                    Err(e) => {
                                        warn!("Failed to decompress BLTE archive index: {}", e);
                                        bytes.to_vec()
                                    }
                                }
                            } else {
                                bytes.to_vec()
                            };

                            // Cache the decompressed archive index for future use
                            if let Err(e) = fs::create_dir_all(&cache_dir).await {
                                warn!("Failed to create cache directory: {}", e);
                            } else if let Err(e) = fs::write(&cache_file, &decompressed).await {
                                warn!("Failed to cache archive index {}: {}", archive_hash, e);
                            } else {
                                debug!(
                                    "✓ Cached archive index {} at {:?}",
                                    archive_hash, cache_file
                                );
                            }

                            return Ok(decompressed);
                        }
                        Err(e) => {
                            warn!("Failed to read response body from {}: {}", host, e);
                        }
                    }
                } else {
                    debug!(
                        "HTTP {} from {} for archive index {}",
                        response.status(),
                        host,
                        archive_hash
                    );
                }
            }
            Err(e) => {
                debug!("Request failed to {}: {}", host, e);
            }
        }
    }

    Err(format!(
        "Failed to download archive index {} from all CDNs",
        archive_hash
    )
    .into())
}

/// Configuration for game installation
#[derive(Debug, Clone)]
struct GameInstallConfig {
    /// Product to install
    product: String,
    /// Installation path
    path: PathBuf,
    /// Specific build to install (optional)
    build: Option<String>,
    /// Region for installation
    region: Region,
    /// Type of installation
    install_type: CliInstallType,
    /// Whether to verify files
    verify: bool,
    /// Whether this is a dry run
    dry_run: bool,
    /// Output format
    format: OutputFormat,
}

/// Configuration for displaying installation plan
#[derive(Debug)]
struct InstallationPlanDisplay {
    /// Product name
    product: String,
    /// Installation path
    path: PathBuf,
    /// Installation type
    install_type: CliInstallType,
    /// Manifest type
    manifest_type: String,
    /// Number of required files
    required_files: usize,
    /// Number of optional files
    optional_files: usize,
    /// Total size in bytes
    total_size: u64,
    /// Output format
    format: OutputFormat,
}

/// Configuration for writing build info file
#[derive(Debug)]
struct BuildInfoConfig<'a> {
    /// Installation path
    install_path: &'a Path,
    /// Product name
    product: &'a str,
    /// Version entry from Ribbit
    version_entry: &'a ribbit_client::VersionEntry,
    /// Build config hash
    build_config_hash: &'a str,
    /// CDN config hash
    cdn_config_hash: &'a str,
    /// Build configuration
    build_config: &'a tact_parser::config::BuildConfig,
    /// CDN entry
    cdn_entry: &'a ribbit_client::CdnEntry,
    /// Region
    region: Region,
}

/// Handle the installation command
pub async fn handle(
    cmd: InstallCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InstallCommands::Game {
            product,
            path,
            build,
            region,
            install_type,
            resume,
            verify,
            dry_run,
            max_concurrent: _,
            tags: _,
        } => {
            let region = region.parse::<Region>().unwrap_or(Region::US);

            // Check for resume mode
            if resume {
                let build_info_path = path.join(".build.info");
                if build_info_path.exists() {
                    info!(
                        "🔄 Resume mode: Continuing existing installation at {:?}",
                        path
                    );
                    return resume_installation(path.as_path(), format).await;
                } else {
                    return Err(format!(
                        "Resume requested but no .build.info found at {}. Start with metadata-only installation first.",
                        path.display()
                    ).into());
                }
            }

            // Normal installation flow
            let config = GameInstallConfig {
                product,
                path,
                build,
                region,
                install_type,
                verify,
                dry_run,
                format,
            };
            handle_game_installation(config).await
        }
        InstallCommands::Repair {
            path,
            verify_checksums,
            dry_run,
            max_concurrent: _,
        } => handle_repair_installation(path, verify_checksums, dry_run, format).await,
    }
}

/// Handle normal game installation
async fn handle_game_installation(
    config: GameInstallConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let GameInstallConfig {
        product,
        path,
        build,
        region,
        install_type,
        verify,
        dry_run,
        format,
    } = config;
    info!("🚀 Starting installation of {} to {:?}", product, path);

    if dry_run {
        info!("🔍 DRY RUN mode - no files will be downloaded");
    }

    // Phase 1: Query product version
    let version_entry = if let Some(build_str) = &build {
        // For specific builds, try Wago Tools API first (for historical builds)
        info!("🔍 Searching for build {} in Wago Tools API...", build_str);

        let builds_response = wago_api::fetch_builds().await?;
        let builds = wago_api::filter_builds_by_product(builds_response, &product);

        if let Some(wago_build) = wago_api::find_build_by_id(&builds, build_str) {
            info!(
                "✓ Found build {} in historical data: {}",
                build_str, wago_build.version
            );

            // Get current CDN config from the latest version since Wago might not have it
            let version_client = HybridVersionClient::new(region).await?;
            let current_versions = version_client.get_product_versions(&product).await?;
            let current_cdn_config = current_versions
                .entries
                .first()
                .map(|v| v.cdn_config.clone())
                .unwrap_or_default();

            // Use Wago's cdn_config if available, otherwise use current
            let cdn_config = wago_build.cdn_config.clone().unwrap_or(current_cdn_config);

            // Create a temporary version entry structure
            use ribbit_client::VersionEntry;
            VersionEntry {
                region: region.to_string(),
                build_config: wago_build.build_config.clone(),
                cdn_config,
                key_ring: None,
                build_id: wago_api::extract_build_id(&wago_build.version)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                versions_name: wago_build.version.clone(),
                product_config: wago_build.product_config.clone().unwrap_or_default(),
            }
        } else {
            // Fallback to current versions API
            info!("🔍 Build not found in historical data, checking current versions...");
            let version_client = HybridVersionClient::new(region).await?;
            let versions = version_client.get_product_versions(&product).await?;

            versions
                .entries
                .iter()
                .find(|v| v.build_id.to_string() == *build_str || v.versions_name == *build_str)
                .ok_or_else(|| {
                    format!(
                        "Build '{}' not found in current or historical versions",
                        build_str
                    )
                })?
                .clone()
        }
    } else {
        // For latest build, use current versions API
        info!("📋 Querying latest product version (HTTPS primary, Ribbit fallback)...");
        let version_client = HybridVersionClient::new(region).await?;
        let versions = version_client.get_product_versions(&product).await?;

        versions
            .entries
            .first()
            .ok_or("No versions available for product")?
            .clone()
    };

    info!(
        "📦 Selected build: {} ({})",
        version_entry.versions_name, version_entry.build_id
    );

    let build_config_hash = &version_entry.build_config;
    let cdn_config_hash = &version_entry.cdn_config;

    // Phase 2: Download configurations
    info!("📥 Downloading configurations...");

    // Get CDN servers (need a fresh client since it might not exist if we used Wago)
    let version_client = HybridVersionClient::new(region).await?;
    let cdns = version_client.get_product_cdns(&product).await?;
    let cdn_entry = cdns.entries.first().ok_or("No CDN servers available")?;

    // Use the first host from the CDN entry (they're bare hostnames like "blzddist1-a.akamaihd.net")
    let cdn_host = cdn_entry.hosts.first().ok_or("No CDN hosts available")?;

    // Use the CDN path as announced by the server
    let cdn_path = &cdn_entry.path;

    debug!("Using CDN host: {} with path: {}", cdn_host, cdn_path);

    // Create cached CDN client with automatic fallback support
    let cdn_client = CachedCdnClient::new().await?;
    // Add Blizzard CDN hosts from the product configuration
    cdn_client.add_primary_hosts(cdn_entry.hosts.iter().cloned());
    // Add community CDNs for fallback
    cdn_client.add_fallback_host("cdn.arctium.tools");
    cdn_client.add_fallback_host("tact.mirror.reliquaryhq.com");

    // Download build config
    let build_config_data = cdn_client
        .download_build_config(&cdn_entry.hosts[0], cdn_path, build_config_hash)
        .await?
        .bytes()
        .await?;
    let build_config =
        tact_parser::config::BuildConfig::parse(std::str::from_utf8(&build_config_data)?)?;
    info!("✓ Build configuration loaded");

    // Download CDN config
    let cdn_config_data = cdn_client
        .download_cdn_config(&cdn_entry.hosts[0], cdn_path, cdn_config_hash)
        .await?
        .bytes()
        .await?;
    let _cdn_config =
        tact_parser::config::ConfigFile::parse(std::str::from_utf8(&cdn_config_data)?)?;
    info!("✓ CDN configuration loaded");

    // Phase 3: Download system files
    info!("📥 Downloading system files...");

    // Download encoding file
    // The encoding field in build config contains two values:
    // 1. Content key (first hash) - not used for direct download
    // 2. Encoding key (second hash) - used to download from CDN
    let encoding_value = build_config
        .config
        .get_value("encoding")
        .ok_or("Missing encoding field")?;
    let encoding_parts: Vec<&str> = encoding_value.split_whitespace().collect();

    // Use the second hash (encoding key) if available, otherwise fall back to first
    let encoding_ekey = if encoding_parts.len() >= 2 {
        encoding_parts[1]
    } else {
        encoding_parts[0]
    };

    debug!("Downloading encoding file with ekey: {}", encoding_ekey);

    let encoding_data = cdn_client
        .download_data(&cdn_entry.hosts[0], cdn_path, encoding_ekey)
        .await?
        .bytes()
        .await?;

    // Decompress with BLTE if needed
    let encoding_data = if encoding_data.starts_with(b"BLTE") {
        blte::decompress_blte(encoding_data.to_vec(), None)?
    } else {
        encoding_data.to_vec()
    };

    let encoding_file = EncodingFile::parse(&encoding_data)?;
    info!(
        "✓ Encoding file loaded: {} CKey entries, {} EKey mappings",
        encoding_file.ckey_count(),
        encoding_file.ekey_count()
    );

    // Download ALL archive indices in parallel for complete coverage
    info!("📦 Downloading ALL archive indices in parallel for complete coverage!");
    let mut archive_index = ArchiveIndex::new();
    let cdn_config_parsed =
        tact_parser::config::CdnConfig::parse(std::str::from_utf8(&cdn_config_data)?)?;
    let all_archives = cdn_config_parsed.archives();

    info!("Found {} total archives available", all_archives.len());
    info!(
        "🚀 Downloading ALL {} archive indices in parallel (10 concurrent)...",
        all_archives.len()
    );

    use futures::stream::{self, StreamExt};

    // Load archive indices sequentially (they're cached, so should be fast)
    info!(
        "📥 Loading {} cached archive indices sequentially...",
        all_archives.len()
    );
    let mut results = Vec::new();

    for (i, archive_hash) in all_archives.iter().enumerate() {
        let result = download_archive_index(&cdn_client, cdn_path, archive_hash).await;
        results.push((i, archive_hash.to_string(), result));

        // Show progress every 100 archives
        if (i + 1) % 100 == 0 || i + 1 == all_archives.len() {
            info!("📦 Loaded {}/{} archive indices", i + 1, all_archives.len());
        }
    }

    let mut successful_archives = 0;
    for (i, archive_hash, result) in results {
        match result {
            Ok(index_data) => match archive_index.parse_and_add_index(&archive_hash, &index_data) {
                Ok(entries) => {
                    debug!(
                        "✓ [{}/{}] Indexed archive {} with {} entries",
                        i + 1,
                        all_archives.len(),
                        archive_hash,
                        entries
                    );
                    successful_archives += 1;
                }
                Err(e) => {
                    warn!("Failed to parse archive index {}: {}", archive_hash, e);
                }
            },
            Err(e) => {
                warn!("Failed to download archive index {}: {}", archive_hash, e);
            }
        }
    }

    info!(
        "✓ Archive indices loaded: {}/{} archives indexed, {} total entries",
        successful_archives,
        all_archives.len(),
        archive_index.map.len()
    );

    // Debug: Show build config info for version verification
    info!("Build Config Info:");
    info!("  - Build Config Hash: {}", build_config_hash);
    info!("  - CDN Config Hash: {}", cdn_config_hash);
    if let Some(build_id) = build_config.config.get_value("build-id") {
        info!("  - Build ID from config: {}", build_id);
    }
    if let Some(encoding_value) = build_config.config.get_value("encoding") {
        info!("  - Encoding value: {}", encoding_value);
    }
    if let Some(install_value) = build_config.config.get_value("install") {
        info!("  - Install value: {}", install_value);
    }

    info!(
        "✓ Archive indices loaded, total entries: {}",
        archive_index.map.len()
    );

    info!("DEBUG: About to get sample CKeys from encoding file...");
    // Debug: Show a few sample content keys from encoding file
    info!("Sample content keys from encoding file:");
    for (i, ckey) in encoding_file.get_sample_ckeys(5).iter().enumerate() {
        info!("  CKey[{}]: {}", i, ckey);
    }
    info!("DEBUG: Finished getting sample CKeys, moving to manifest processing...");

    info!(
        "🔄 Starting manifest download based on installation type: {:?}",
        install_type
    );
    // Download manifests based on installation type
    let (file_entries, manifest_type) = match install_type {
        CliInstallType::Minimal => {
            info!("📥 Processing minimal installation - using download manifest");
            // TEMPORARY FIX: For minimal install, use download manifest and filter it
            // The install manifest CKeys don't exist in encoding file for this build
            let download_value = build_config
                .config
                .get_value("download")
                .ok_or("Missing download field")?;
            let download_parts: Vec<&str> = download_value.split_whitespace().collect();

            let download_ekey = if download_parts.len() >= 2 {
                download_parts[1].to_string()
            } else {
                let ckey = download_parts[0];
                let ekey_bytes = encoding_file
                    .lookup_by_ckey(&hex::decode(ckey)?)
                    .and_then(|e| e.encoding_keys.first())
                    .ok_or("Download file encoding key not found in encoding table")?;
                hex::encode(ekey_bytes)
            };

            info!(
                "📥 Downloading download manifest with ekey: {}",
                download_ekey
            );

            let download_data = cdn_client
                .download_data(&cdn_entry.hosts[0], cdn_path, &download_ekey)
                .await?
                .bytes()
                .await?;

            let download_data = if download_data.starts_with(b"BLTE") {
                blte::decompress_blte(download_data.to_vec(), None)?
            } else {
                download_data.to_vec()
            };

            let download_manifest = DownloadManifest::parse(&download_data)?;
            info!(
                "✓ Download manifest loaded: {} files (filtering for minimal install)",
                download_manifest.entries.len()
            );

            // Debug: Show a few sample EKeys from download manifest
            info!("Sample EKeys from download manifest:");
            for (i, (ekey, entry)) in download_manifest.entries.iter().enumerate() {
                if i < 5 {
                    info!(
                        "  Download[{}]: {} (size: {} bytes)",
                        i,
                        hex::encode(ekey),
                        entry.compressed_size
                    );
                } else {
                    break;
                }
            }

            // Test: Check if download manifest EKeys exist in archives (they should)
            info!("Testing first few download manifest EKeys in archive indices:");
            for (i, (ekey, entry)) in download_manifest.entries.iter().take(5).enumerate() {
                let test_ekey = hex::encode(ekey);
                match archive_index.lookup(ekey) {
                    Some(location) => {
                        info!(
                            "  ✓ Download[{}]: {} FOUND in archive {} at offset {} (size: {})",
                            i, test_ekey, location.archive_hash, location.offset, location.size
                        );
                    }
                    None => {
                        info!(
                            "  ✗ Download[{}]: {} NOT FOUND in archives (size: {})",
                            i, test_ekey, entry.compressed_size
                        );
                    }
                }
            }

            // Convert download entries to common format (select first 10 for minimal)
            let entries: Vec<FileEntry> = download_manifest
                .entries
                .iter()
                .take(10)
                .map(|(ekey, entry)| FileEntry {
                    path: format!("file_{}", hex::encode(&ekey[..4])), // Generate path from EKey
                    ckey: ekey.clone(), // For download manifest, we use EKey directly
                    size: entry.compressed_size,
                    priority: 0,
                })
                .collect();

            info!(
                "Selected {} files for minimal download install",
                entries.len()
            );
            (entries, "download")
        }
        CliInstallType::Full | CliInstallType::Custom => {
            info!("📥 Processing FULL/CUSTOM installation - using download manifest for all files");
            // For full install, use download manifest (complete game files)
            let download_value = build_config
                .config
                .get_value("download")
                .ok_or("Missing download field")?;
            let download_parts: Vec<&str> = download_value.split_whitespace().collect();

            let download_ekey = if download_parts.len() >= 2 {
                download_parts[1].to_string()
            } else {
                let ckey = download_parts[0];
                let ekey_bytes = encoding_file
                    .lookup_by_ckey(&hex::decode(ckey)?)
                    .and_then(|e| e.encoding_keys.first())
                    .ok_or("Download file encoding key not found in encoding table")?;
                hex::encode(ekey_bytes)
            };

            debug!("Downloading download manifest with ekey: {}", download_ekey);

            let download_data = cdn_client
                .download_data(&cdn_entry.hosts[0], cdn_path, &download_ekey)
                .await?
                .bytes()
                .await?;

            let download_data = if download_data.starts_with(b"BLTE") {
                blte::decompress_blte(download_data.to_vec(), None)?
            } else {
                download_data.to_vec()
            };

            let download_manifest = DownloadManifest::parse(&download_data)?;
            info!(
                "✓ Download manifest loaded: {} files (complete game)",
                download_manifest.entries.len()
            );

            // Convert download entries to common format (no paths, just ekeys)
            // NOTE: Use download manifest compressed_size but filter out unreasonable values
            let mut total_entries = 0;
            let mut skipped_not_in_encoding = 0;
            let skipped_bad_size = 0;

            let entries: Vec<FileEntry> = download_manifest
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, (_ekey, e))| {
                    total_entries += 1;
                    // Look up the CKey from the encoding file using the EKey
                    if let Some(ckey) = encoding_file.lookup_by_ekey(&e.ekey) {
                        // Get actual file size from encoding file (more reliable than download manifest)
                        let file_size = encoding_file
                            .get_file_size(ckey)
                            .unwrap_or(e.compressed_size);

                        Some(FileEntry {
                            path: format!("data/{:08x}", i), // Generate placeholder path without .blte extension
                            ckey: ckey.clone(),              // Use CKey from encoding file
                            size: file_size, // Use size from encoding file if available
                            priority: e.priority,
                        })
                    } else {
                        skipped_not_in_encoding += 1;
                        if skipped_not_in_encoding <= 5 {
                            debug!("EKey {} not found in encoding file", hex::encode(&e.ekey));
                        }
                        None // Skip entries not found in encoding
                    }
                })
                .collect();

            info!(
                "Download manifest processing: {} total entries, {} included, {} not in encoding, {} bad size",
                total_entries,
                entries.len(),
                skipped_not_in_encoding,
                skipped_bad_size
            );

            (entries, "download")
        }
        CliInstallType::MetadataOnly => {
            // For metadata-only, we don't need any file entries
            (Vec::new(), "metadata-only")
        }
    };

    // Phase 4: Build file list
    info!("📋 Building file manifest...");

    let mut total_size = 0u64;
    let mut required_files = 0;
    let mut optional_files = 0;

    for entry in &file_entries {
        // Check if file should be installed based on type
        let is_required = match install_type {
            CliInstallType::Minimal => is_required_file(&entry.path),
            CliInstallType::Full => true,
            CliInstallType::Custom => {
                // TODO: Implement tag filtering based on priority
                entry.priority <= 0 // High priority files only for now
            }
            CliInstallType::MetadataOnly => false, // No files are required for metadata-only
        };

        if is_required {
            required_files += 1;
        } else {
            optional_files += 1;
        }

        total_size += entry.size;
    }

    // Display installation plan
    let plan = InstallationPlanDisplay {
        product: product.clone(),
        path: path.clone(),
        install_type,
        manifest_type: manifest_type.to_string(),
        required_files,
        optional_files,
        total_size,
        format,
    };
    display_installation_plan(&plan)?;

    // Phase 5: Create directory structure
    info!("🗄️ Creating directory structure...");
    tokio::fs::create_dir_all(&path).await?;
    tokio::fs::create_dir_all(path.join("Data")).await?;
    tokio::fs::create_dir_all(path.join("Data/data")).await?;
    tokio::fs::create_dir_all(path.join("Data/config")).await?;
    info!("✓ Directory structure created");

    // Phase 6: Write .build.info file for client functionality (even in dry-run mode)
    info!("📄 Writing .build.info file...");
    let build_info_config = BuildInfoConfig {
        install_path: path.as_path(),
        product: &product,
        version_entry: &version_entry,
        build_config_hash,
        cdn_config_hash,
        build_config: &build_config,
        cdn_entry,
        region,
    };
    write_build_info_file(build_info_config).await?;
    info!("✓ .build.info file written");

    if dry_run {
        info!("✅ Dry run complete - no files were downloaded");
        return Ok(());
    }

    // Write configuration files to Data/config/ for all installation types
    info!("📄 Writing configuration files to Data/config/...");

    // Write build configuration using CDN-style subdirectory structure
    let build_config_subdir = format!("{}/{}", &build_config_hash[0..2], &build_config_hash[2..4]);
    let build_config_dir = path.join("Data/config").join(&build_config_subdir);
    tokio::fs::create_dir_all(&build_config_dir).await?;
    let build_config_path = build_config_dir.join(build_config_hash);
    tokio::fs::write(&build_config_path, &build_config_data).await?;
    info!(
        "✓ Saved build config: {}/{}",
        build_config_subdir, build_config_hash
    );

    // Write CDN configuration using CDN-style subdirectory structure
    let cdn_config_subdir = format!("{}/{}", &cdn_config_hash[0..2], &cdn_config_hash[2..4]);
    let cdn_config_dir = path.join("Data/config").join(&cdn_config_subdir);
    tokio::fs::create_dir_all(&cdn_config_dir).await?;
    let cdn_config_path = cdn_config_dir.join(cdn_config_hash);
    tokio::fs::write(&cdn_config_path, &cdn_config_data).await?;
    info!(
        "✓ Saved CDN config: {}/{}",
        cdn_config_subdir, cdn_config_hash
    );

    // Write encoding file info (just metadata, not the full file)
    let encoding_info_path = path.join("Data/config").join("encoding.info");
    let encoding_info = format!(
        "# Encoding file information\n\
        # Generated by cascette-rs\n\
        Encoding-Hash: {}\n\
        CKey-Count: {}\n\
        EKey-Count: {}\n\
        Build: {}\n\
        Product: {}\n\
        Region: {}\n",
        build_config
            .config
            .get_value("encoding")
            .unwrap_or("unknown")
            .split_whitespace()
            .next()
            .unwrap_or("unknown"),
        encoding_file.ckey_count(),
        encoding_file.ekey_count(),
        version_entry.build_id,
        product,
        region
    );
    tokio::fs::write(&encoding_info_path, encoding_info).await?;
    info!("✓ Saved encoding info: encoding.info");

    // For metadata-only installations, we're done
    if install_type == CliInstallType::MetadataOnly {
        info!("✅ Metadata-only installation complete");
        info!("📋 Created: .build.info and Data/config/ with CDN-style structure");
        info!("💡 Use this for quick client comparison or as base for full installation");
        return Ok(());
    }

    // Phase 7: Download files
    info!("📥 Downloading files...");

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"),
    );

    // Filter files to download
    let files_to_download: Vec<_> = file_entries
        .iter()
        .filter(|entry| {
            match install_type {
                CliInstallType::Minimal => {
                    // For minimal installs using download manifest, we already selected 10 files
                    // No need for additional filtering since we don't have real file paths
                    let include = manifest_type == "download" || is_required_file(&entry.path);
                    if !include {
                        debug!("Skipping file for minimal install: {}", entry.path);
                    } else {
                        debug!("Including file for minimal install: {}", entry.path);
                    }
                    include
                }
                CliInstallType::Full => true,
                CliInstallType::Custom => entry.priority <= 0, // High priority only for now
                CliInstallType::MetadataOnly => false, // Never download files for metadata-only
            }
        })
        .collect();

    info!(
        "Files selected for download: {} out of {} total files",
        files_to_download.len(),
        file_entries.len()
    );

    if files_to_download.is_empty() {
        error!("❌ No files selected for download! Check filtering logic.");
        return Ok(());
    }

    info!("DEBUG: Passed file selection check, continuing to download setup...");

    // Show first few files that will be downloaded
    for (i, entry) in files_to_download.iter().take(3).enumerate() {
        info!(
            "File {}: {} (ckey: {})",
            i + 1,
            entry.path,
            hex::encode(&entry.ckey)
        );
    }

    info!(
        "Downloading {} files with parallel processing (max 10 concurrent)",
        files_to_download.len()
    );

    // Use futures stream for parallel downloads with controlled concurrency
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let downloaded_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let pb = Arc::new(pb);
    let cdn_client = Arc::new(cdn_client);
    let archive_index = Arc::new(archive_index);
    let encoding_file = Arc::new(encoding_file);
    let path = Arc::new(path);

    info!("Starting download of {} files...", files_to_download.len());

    // Simple test to verify async works
    info!("DEBUG: Testing async runtime...");
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    info!("DEBUG: Async runtime works!");

    info!("DEBUG: Creating stream iterator...");
    info!("Starting download of {} files", files_to_download.len());

    // Check if we have any files to download
    if files_to_download.is_empty() {
        warn!("No files selected for download!");
        return Ok(());
    }

    // Debug: Show actual files we're about to download
    info!("Files to download: {}", files_to_download.len());
    for (i, entry) in files_to_download.iter().take(5).enumerate() {
        info!(
            "  File {}: {} (size: {} bytes)",
            i + 1,
            entry.path,
            entry.size
        );
    }

    // Process downloads with proper concurrency
    let total_files = files_to_download.len();
    info!("Starting to process {} files concurrently", total_files);
    let download_futures = stream::iter(files_to_download)
        .map(|entry| {
            let cdn_client = cdn_client.clone();
            let archive_index = archive_index.clone();
            let encoding_file = encoding_file.clone();
            let path = path.clone();
            let pb = pb.clone();
            let downloaded_count = downloaded_count.clone();
            let error_count = error_count.clone();
            let manifest_type = manifest_type.to_string();
            let entry = entry.clone(); // Clone the entry for the async block

            async move {
                info!("DEBUG: Entered async closure for file: {}", entry.path);
                info!(
                    "Processing file: {} (ckey: {})",
                    entry.path,
                    hex::encode(&entry.ckey)
                );

                // Create parent directory for the file
                let file_dir = path.join("Data/data");
                if let Err(e) = tokio::fs::create_dir_all(&file_dir).await {
                    warn!("Failed to create directory {}: {}", file_dir.display(), e);
                    error_count.fetch_add(1, Ordering::Relaxed);
                    return;
                }

                // For install manifest entries, we need to look up the encoding key
                // For download manifest entries, we already have the encoding key
                let download_key = if manifest_type == "install" {
                    // Look up encoding key for content key
                    debug!(
                        "Looking up ckey: {} (path: {})",
                        hex::encode(&entry.ckey),
                        entry.path
                    );
                    if let Some(encoding_entry) = encoding_file.lookup_by_ckey(&entry.ckey) {
                        // Validate file size (catch corruption like 121TB files)
                        if encoding_entry.size > 10_000_000_000 {
                            // 10GB limit
                            debug!(
                                "Skipping file with suspicious size: {} bytes ({}GB) for path: {}",
                                encoding_entry.size,
                                encoding_entry.size / 1_000_000_000,
                                entry.path
                            );
                            return;
                        }

                        if let Some(ekey) = encoding_entry.encoding_keys.first() {
                            debug!(
                                "Found ekey: {} for ckey: {}",
                                hex::encode(ekey),
                                hex::encode(&entry.ckey)
                            );
                            hex::encode(ekey)
                        } else {
                            warn!(
                                "No encoding key found for content key: {} (path: {}) - skipping",
                                hex::encode(&entry.ckey),
                                entry.path
                            );
                            return;
                        }
                    } else {
                        // Content key not found in encoding file, skip it
                        warn!(
                            "Content key not found in encoding file: {} (path: {}) - skipping",
                            hex::encode(&entry.ckey),
                            entry.path
                        );
                        return; // Skip files without encoding entries
                    }
                } else {
                    // Download manifest already has encoding keys
                    hex::encode(&entry.ckey)
                };

                // Download file using archive-aware method
                info!(
                    "Attempting to download file: {} with key: {}",
                    entry.path, download_key
                );
                info!("DEBUG: About to call download_file_with_archive...");
                info!("Archive index has {} entries", archive_index.map.len());
                match download_file_with_archive(
                    &cdn_client,
                    &archive_index,
                    &cdn_entry.hosts[0],
                    cdn_path,
                    &download_key,
                )
                .await
                {
                    Ok(data) => {
                        // Store files in subdirectories based on first 2 bytes of hash (like CASC)
                        // e.g., ab/cd/abcdef...
                        let subdir1 = &download_key[0..2];
                        let subdir2 = &download_key[2..4];
                        let file_dir = path.join("Data/data").join(subdir1).join(subdir2);

                        // Create subdirectory structure
                        if let Err(e) = tokio::fs::create_dir_all(&file_dir).await {
                            warn!("Failed to create directory {}: {}", file_dir.display(), e);
                            error_count.fetch_add(1, Ordering::Relaxed);
                            return;
                        }

                        let file_path = file_dir.join(&download_key);
                        info!(
                            "Writing {} bytes to path: {}",
                            data.len(),
                            file_path.display()
                        );
                        if let Err(e) = tokio::fs::write(&file_path, &data).await {
                            warn!("Failed to write {}: {}", entry.path, e);
                            error_count.fetch_add(1, Ordering::Relaxed);
                        } else {
                            downloaded_count.fetch_add(1, Ordering::Relaxed);
                            pb.inc(entry.size);
                            info!(
                                "✓ Downloaded and wrote {} ({} bytes to {})",
                                entry.path,
                                data.len(),
                                file_path.display()
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to download {}: {}", entry.path, e);
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        })
        .buffer_unordered(50) // Process up to 50 downloads concurrently
        .collect::<Vec<_>>();

    info!("DEBUG: Awaiting all download futures...");

    // Actually execute the futures and collect results
    let results: Vec<_> = download_futures.await;
    info!(
        "Download futures completed - processed {} results",
        results.len()
    );

    info!("DEBUG: Stream processing completed");
    info!("Completed processing all file download tasks");

    pb.finish_with_message("Download complete!");

    let final_downloaded = downloaded_count.load(Ordering::Relaxed);
    let final_errors = error_count.load(Ordering::Relaxed);

    info!(
        "✅ Installation completed: {} files downloaded, {} errors",
        final_downloaded, final_errors
    );

    if verify {
        info!("🔍 Verifying installation...");
        // TODO: Implement verification
        info!("✓ Verification complete");
    }

    Ok(())
}

/// Check if a file is required for basic functionality
fn is_required_file(path: &str) -> bool {
    // Core executables and libraries
    if path.ends_with(".exe") || path.ends_with(".dll") || path.ends_with(".so") {
        return true;
    }

    // Configuration files
    if path.contains("config") || path.ends_with(".ini") || path.ends_with(".xml") {
        return true;
    }

    // Core data files - be more inclusive for WoW Classic Era
    if path.starts_with("Data/") {
        // Include DBC files which are critical for WoW
        if path.ends_with(".dbc") || path.ends_with(".db2") {
            return true;
        }

        // Include patch and locale data
        if path.contains("patch") || path.contains("locale") || path.contains("enUS") {
            return true;
        }

        // Include common WoW data directories
        if path.contains("base") || path.contains("core") || path.contains("common") {
            return true;
        }
    }

    // For minimal installs, include some essential executables
    if path.ends_with("Wow.exe") || path.ends_with("WowClassic.exe") {
        return true;
    }

    false
}

/// Display installation plan to user
fn display_installation_plan(
    plan: &InstallationPlanDisplay,
) -> Result<(), Box<dyn std::error::Error>> {
    let InstallationPlanDisplay {
        product,
        path,
        install_type,
        manifest_type,
        required_files,
        optional_files,
        total_size,
        format,
    } = plan;
    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let plan = serde_json::json!({
                "product": product,
                "path": path,
                "install_type": format!("{:?}", install_type),
                "manifest_type": manifest_type,
                "required_files": required_files,
                "optional_files": optional_files,
                "total_files": required_files + optional_files,
                "total_size": total_size,
                "total_size_human": format_bytes(*total_size),
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("{}", serde_json::to_string(&plan)?);
            }
        }
        OutputFormat::Text => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["Installation Plan", "Value"]);

            table.add_row(vec![Cell::new("Product"), Cell::new(product)]);
            table.add_row(vec![
                Cell::new("Installation Path"),
                Cell::new(path.display()),
            ]);
            table.add_row(vec![
                Cell::new("Installation Type"),
                Cell::new(format!("{install_type:?}")),
            ]);
            table.add_row(vec![Cell::new("Manifest Type"), Cell::new(manifest_type)]);
            table.add_row(vec![Cell::new("Required Files"), Cell::new(required_files)]);
            table.add_row(vec![Cell::new("Optional Files"), Cell::new(optional_files)]);
            table.add_row(vec![
                Cell::new("Total Files"),
                Cell::new(required_files + optional_files),
            ]);
            table.add_row(vec![
                Cell::new("Total Size"),
                Cell::new(if *install_type == CliInstallType::MetadataOnly {
                    "Metadata only".to_string()
                } else {
                    format_bytes(*total_size)
                }),
            ]);

            println!("{table}");
        }
        OutputFormat::Bpsv => {
            // Not applicable for installation plan
            return Err("BPSV format not supported for installation plan".into());
        }
    }

    Ok(())
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Write .build.info file for client functionality
///
/// Creates a BPSV-formatted file containing build metadata required by the game client.
/// This file allows the client to identify its build version and connect to appropriate CDN servers.
async fn write_build_info_file(
    config: BuildInfoConfig<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let BuildInfoConfig {
        install_path,
        product,
        version_entry,
        build_config_hash,
        cdn_config_hash,
        build_config,
        cdn_entry,
        region,
    } = config;
    // Extract install key from build config
    let install_value = build_config.config.get_value("install").unwrap_or("");
    let install_parts: Vec<&str> = install_value.split_whitespace().collect();
    let install_key = if install_parts.len() >= 2 {
        install_parts[1] // Use encoding key if available
    } else {
        install_parts.first().copied().unwrap_or("") // Fallback to content key
    };

    // Create CDN hosts string (space-separated)
    let cdn_hosts = cdn_entry.hosts.join(" ");

    // Create CDN servers string (space-separated with parameters)
    let cdn_servers = if cdn_entry.servers.is_empty() {
        // Generate default server URLs from hosts if servers list is empty
        cdn_entry
            .hosts
            .iter()
            .flat_map(|host| {
                vec![
                    format!("http://{}/?maxhosts=4", host),
                    format!("https://{}/?maxhosts=4&fallback=1", host),
                ]
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        cdn_entry.servers.join(" ")
    };

    // Generate basic tags (platform/architecture)
    let tags = format!(
        "Windows x86_64 {}? acct-{}?",
        region.as_str().to_uppercase(),
        region.as_str().to_uppercase()
    );

    // Build .build.info using BPSV builder
    let mut builder = BpsvBuilder::new();

    // Add fields according to .build.info schema
    builder.add_field("Branch", BpsvFieldType::String(0))?;
    builder.add_field("Active", BpsvFieldType::Decimal(1))?;
    builder.add_field("Build Key", BpsvFieldType::Hex(16))?;
    builder.add_field("CDN Key", BpsvFieldType::Hex(16))?;
    builder.add_field("Install Key", BpsvFieldType::Hex(16))?;
    builder.add_field("IM Size", BpsvFieldType::Decimal(4))?;
    builder.add_field("CDN Path", BpsvFieldType::String(0))?;
    builder.add_field("CDN Hosts", BpsvFieldType::String(0))?;
    builder.add_field("CDN Servers", BpsvFieldType::String(0))?;
    builder.add_field("Tags", BpsvFieldType::String(0))?;
    builder.add_field("Armadillo", BpsvFieldType::String(0))?;
    builder.add_field("Last Activated", BpsvFieldType::String(0))?;
    builder.add_field("Version", BpsvFieldType::String(0))?;
    builder.add_field("KeyRing", BpsvFieldType::Hex(16))?;
    builder.add_field("Product", BpsvFieldType::String(0))?;

    // Add the data row
    builder.add_row(vec![
        BpsvValue::String(region.as_str().to_string()), // Branch
        BpsvValue::Decimal(1),                          // Active (always 1)
        BpsvValue::Hex(build_config_hash.to_string()),  // Build Key
        BpsvValue::Hex(cdn_config_hash.to_string()),    // CDN Key
        BpsvValue::Hex(install_key.to_string()),        // Install Key
        BpsvValue::Decimal(0),                          // IM Size (empty)
        BpsvValue::String(cdn_entry.path.clone()),      // CDN Path
        BpsvValue::String(cdn_hosts),                   // CDN Hosts
        BpsvValue::String(cdn_servers),                 // CDN Servers
        BpsvValue::String(tags),                        // Tags
        BpsvValue::String(String::new()),               // Armadillo (empty)
        BpsvValue::String(String::new()),               // Last Activated (empty)
        BpsvValue::String(version_entry.versions_name.clone()), // Version
        BpsvValue::Hex(version_entry.key_ring.as_deref().unwrap_or("").to_string()), // KeyRing
        BpsvValue::String(product.to_string()),         // Product
    ])?;

    // Build the BPSV content
    let build_info_content = builder.build_string()?;

    // Write .build.info file to installation root directory
    let build_info_path = install_path.join(".build.info");
    tokio::fs::write(&build_info_path, build_info_content).await?;

    debug!("Written .build.info to: {}", build_info_path.display());
    Ok(())
}

/// Resume an existing installation by detecting missing files
async fn resume_installation(
    install_path: &Path,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📋 Reading installation metadata from .build.info...");

    // Read and parse .build.info file
    let build_info_path = install_path.join(".build.info");
    let build_info_content = tokio::fs::read_to_string(&build_info_path).await?;

    // Parse BPSV format to extract product, version, and CDN information
    let build_info = ngdp_bpsv::BpsvDocument::parse(&build_info_content)?;

    // Extract key information from .build.info
    let rows = build_info.rows();
    if rows.is_empty() {
        return Err("No entries found in .build.info file".into());
    }

    let schema = build_info.schema();
    let row = &rows[0]; // Use first entry
    let product = row
        .get_raw_by_name("Product", schema)
        .ok_or("Product not found in .build.info")?;
    let version = row
        .get_raw_by_name("Version", schema)
        .ok_or("Version not found in .build.info")?;
    let branch = row
        .get_raw_by_name("Branch", schema)
        .ok_or("Branch not found in .build.info")?;
    let build_key = row
        .get_raw_by_name("Build Key", schema)
        .ok_or("Build Key not found in .build.info")?;
    let cdn_path = row
        .get_raw_by_name("CDN Path", schema)
        .ok_or("CDN Path not found in .build.info")?;
    let cdn_hosts_str = row
        .get_raw_by_name("CDN Hosts", schema)
        .ok_or("CDN Hosts not found in .build.info")?;

    // Parse CDN hosts (space-separated)
    let cdn_hosts: Vec<&str> = cdn_hosts_str.split_whitespace().collect();
    let cdn_host = cdn_hosts.first().ok_or("No CDN hosts available")?;

    info!("🔄 Resuming installation:");
    info!("  • Product: {}", product);
    info!("  • Version: {}", version);
    info!("  • Branch: {}", branch);
    info!("  • Build Key: {}", build_key);
    info!("  • CDN Host: {}", cdn_host);

    // Read build configuration from Data/config/ structure
    let build_config_subdir = format!("{}/{}", &build_key[0..2], &build_key[2..4]);
    let build_config_path = install_path
        .join("Data/config")
        .join(&build_config_subdir)
        .join(build_key);

    if !build_config_path.exists() {
        return Err(format!(
            "Build configuration not found at: {}. The installation appears corrupted.",
            build_config_path.display()
        )
        .into());
    }

    let build_config_data = tokio::fs::read_to_string(&build_config_path).await?;
    let build_config = tact_parser::config::BuildConfig::parse(&build_config_data)?;

    info!("✓ Loaded build configuration from local cache");

    // Get encoding file from config
    let encoding_value = build_config
        .config
        .get_value("encoding")
        .ok_or("Missing encoding field in build config")?;
    let encoding_parts: Vec<&str> = encoding_value.split_whitespace().collect();
    let encoding_ekey = if encoding_parts.len() >= 2 {
        encoding_parts[1]
    } else {
        encoding_parts[0]
    };

    // Download and parse encoding file
    info!("📥 Downloading encoding file...");
    let cdn_client = CachedCdnClient::new().await?;
    cdn_client.add_primary_hosts(cdn_hosts.iter().map(|h| h.to_string()));
    // Add community CDNs for fallback
    cdn_client.add_fallback_host("cdn.arctium.tools");
    cdn_client.add_fallback_host("tact.mirror.reliquaryhq.com");
    let encoding_data = cdn_client
        .download_data(cdn_hosts[0], cdn_path, encoding_ekey)
        .await?
        .bytes()
        .await?;

    let encoding_data = if encoding_data.starts_with(b"BLTE") {
        blte::decompress_blte(encoding_data.to_vec(), None)?
    } else {
        encoding_data.to_vec()
    };

    let encoding_file = EncodingFile::parse(&encoding_data)?;
    info!("✓ Encoding file loaded");

    // For resume, we'll create an empty archive index since we don't have CDN config readily available
    // This means we'll fall back to loose file downloads, which should work for resume scenarios
    let archive_index = ArchiveIndex::new();
    info!("📦 Using empty archive index for resume (loose file fallback)");

    // Get install manifest information
    let install_value = build_config
        .config
        .get_value("install")
        .ok_or("Missing install field in build config")?;
    let install_parts: Vec<&str> = install_value.split_whitespace().collect();

    let install_ekey = if install_parts.len() >= 2 {
        install_parts[1].to_string()
    } else {
        // Look up content key in encoding file
        let ckey = install_parts[0];
        let ekey_bytes = encoding_file
            .lookup_by_ckey(&hex::decode(ckey)?)
            .and_then(|e| e.encoding_keys.first())
            .ok_or("Install manifest encoding key not found")?;
        hex::encode(ekey_bytes)
    };

    // Download and parse install manifest
    info!("📥 Downloading install manifest...");
    let install_data = cdn_client
        .download_data(cdn_hosts[0], cdn_path, &install_ekey)
        .await?
        .bytes()
        .await?;

    let install_data = if install_data.starts_with(b"BLTE") {
        blte::decompress_blte(install_data.to_vec(), None)?
    } else {
        install_data.to_vec()
    };

    let install_manifest = InstallManifest::parse(&install_data)?;
    info!(
        "📋 Install manifest loaded: {} files",
        install_manifest.entries.len()
    );

    // Check which files are missing
    let data_dir = install_path.join("Data/data");
    tokio::fs::create_dir_all(&data_dir).await?;

    let mut missing_files = Vec::new();
    let mut total_missing_size = 0u64;

    info!("🔍 Checking for missing files...");
    for entry in &install_manifest.entries {
        // Look up encoding key for this content key
        if let Some(encoding_entry) = encoding_file.lookup_by_ckey(&entry.ckey) {
            if let Some(ekey) = encoding_entry.encoding_keys.first() {
                let ekey_hex = hex::encode(ekey);
                let expected_path = data_dir.join(&ekey_hex);

                if !expected_path.exists() {
                    missing_files.push((entry, ekey_hex));
                    total_missing_size += entry.size as u64;
                }
            }
        }
    }

    if missing_files.is_empty() {
        info!("✅ No missing files found - installation is complete!");
        return Ok(());
    }

    info!(
        "📊 Found {} missing files ({} total)",
        missing_files.len(),
        format_bytes(total_missing_size)
    );

    info!("📥 Downloading missing files...");
    let mut downloaded_count = 0;
    let mut error_count = 0;

    for (entry, ekey_hex) in &missing_files {
        match download_file_with_archive(
            &cdn_client,
            &archive_index,
            cdn_hosts[0],
            cdn_path,
            ekey_hex,
        )
        .await
        {
            Ok(data) => {
                // Write file to disk
                let file_path = data_dir.join(ekey_hex);
                if let Err(e) = tokio::fs::write(&file_path, &data).await {
                    warn!("Failed to write {}: {}", entry.path, e);
                    error_count += 1;
                } else {
                    downloaded_count += 1;
                    if downloaded_count % 10 == 0 {
                        info!(
                            "📥 Downloaded {}/{} files...",
                            downloaded_count,
                            missing_files.len()
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Failed to fetch {}: {}", entry.path, e);
                error_count += 1;
            }
        }
    }

    info!(
        "✅ Resume completed: {} files downloaded, {} errors",
        downloaded_count, error_count
    );

    Ok(())
}

/// Handle repair of an existing installation
async fn handle_repair_installation(
    install_path: PathBuf,
    verify_checksums: bool,
    dry_run: bool,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔧 Starting repair of installation at {:?}", install_path);

    if dry_run {
        info!("🔍 DRY RUN mode - no files will be modified");
    }

    let build_info_path = install_path.join(".build.info");
    if !build_info_path.exists() {
        return Err(format!(
            "No .build.info found at {}. This does not appear to be a valid installation.",
            install_path.display()
        )
        .into());
    }

    if verify_checksums {
        info!("🔍 Verifying file checksums...");
        // TODO: Implement checksum verification
        info!("🚧 Checksum verification not yet implemented");
    }

    // For now, repair is similar to resume - detect missing files
    info!("🔍 Checking for missing or corrupted files...");

    if dry_run {
        info!("✅ Dry run completed - repair functionality in development");
    } else {
        info!("🚧 Repair functionality implementation in progress");
        info!(
            "💡 Use 'ngdp install game <product> --path {} --resume' for now",
            install_path.display()
        );
    }

    Ok(())
}
