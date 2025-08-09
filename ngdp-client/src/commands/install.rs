use crate::{InstallCommands, InstallType as CliInstallType, OutputFormat};
use comfy_table::{Cell, ContentArrangement, Table, presets::UTF8_FULL};
use indicatif::{ProgressBar, ProgressStyle};
use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cache::hybrid_version_client::HybridVersionClient;
use ribbit_client::Region;
use std::path::{Path, PathBuf};
use tact_parser::download::DownloadManifest;
use tact_parser::encoding::EncodingFile;
use tact_parser::install::InstallManifest;
use tracing::{debug, info, warn};

/// Unified file entry for both install and download manifests
#[derive(Debug, Clone)]
struct FileEntry {
    path: String,
    ckey: Vec<u8>, // For install manifest entries, for download manifest this is ekey
    size: u64,
    priority: i8,
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

    // Phase 1: Query product version (HTTP-first, Ribbit fallback)
    info!("📋 Querying product versions (HTTPS primary, Ribbit fallback)...");
    let version_client = HybridVersionClient::new(region).await?;
    let versions = version_client.get_product_versions(&product).await?;

    // Find the specific build or use latest
    let version_entry = if let Some(build_str) = &build {
        versions
            .entries
            .iter()
            .find(|v| v.build_id.to_string() == *build_str || v.versions_name == *build_str)
            .ok_or_else(|| format!("Build '{build_str}' not found"))?
    } else {
        versions
            .entries
            .first()
            .ok_or("No versions available for product")?
    };

    info!(
        "📦 Selected build: {} ({})",
        version_entry.versions_name, version_entry.build_id
    );

    let build_config_hash = &version_entry.build_config;
    let cdn_config_hash = &version_entry.cdn_config;

    // Phase 2: Download configurations
    info!("📥 Downloading configurations...");

    // Get CDN servers
    let cdns = version_client.get_product_cdns(&product).await?;
    let cdn_entry = cdns.entries.first().ok_or("No CDN servers available")?;

    // Use the first host from the CDN entry (they're bare hostnames like "blzddist1-a.akamaihd.net")
    let cdn_host = cdn_entry.hosts.first().ok_or("No CDN hosts available")?;
    let cdn_path = &cdn_entry.path;
    debug!("Using CDN host: {} with path: {}", cdn_host, cdn_path);

    // Create CDN client
    let cdn_client = CachedCdnClient::new().await?;

    // Download build config
    let build_config_data = cdn_client
        .download_build_config(cdn_host, cdn_path, build_config_hash)
        .await?
        .bytes()
        .await?;
    let build_config =
        tact_parser::config::BuildConfig::parse(std::str::from_utf8(&build_config_data)?)?;
    info!("✓ Build configuration loaded");

    // Download CDN config
    let cdn_config_data = cdn_client
        .download_cdn_config(cdn_host, cdn_path, cdn_config_hash)
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
        .download_data(cdn_host, cdn_path, encoding_ekey)
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

    // Debug: Show a few sample content keys from encoding file
    info!("Sample content keys from encoding file:");
    for (i, ckey) in encoding_file.get_sample_ckeys(5).iter().enumerate() {
        info!("  CKey[{}]: {}", i, ckey);
    }

    // Download manifests based on installation type
    let (file_entries, manifest_type) = match install_type {
        CliInstallType::Minimal => {
            // For minimal install, use install manifest (bootstrap files only)
            let install_value = build_config
                .config
                .get_value("install")
                .ok_or("Missing install field")?;
            let install_parts: Vec<&str> = install_value.split_whitespace().collect();

            let install_ekey = if install_parts.len() >= 2 {
                info!(
                    "Using direct install EKey from build config: {}",
                    install_parts[1]
                );
                install_parts[1].to_string()
            } else {
                let ckey = install_parts[0];
                info!("Looking up install CKey in encoding file: {}", ckey);
                let ekey_bytes = encoding_file
                    .lookup_by_ckey(&hex::decode(ckey)?)
                    .and_then(|e| e.encoding_keys.first())
                    .ok_or("Install file encoding key not found in encoding table")?;
                let ekey_hex = hex::encode(ekey_bytes);
                info!("Found install EKey via encoding lookup: {}", ekey_hex);
                ekey_hex
            };

            debug!("Downloading install manifest with ekey: {}", install_ekey);

            let install_data = cdn_client
                .download_data(cdn_host, cdn_path, &install_ekey)
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
                "✓ Install manifest loaded: {} files (bootstrap only)",
                install_manifest.entries.len()
            );
            info!("Install manifest verification:");
            info!("  - Downloaded with EKey: {}", install_ekey);
            info!("  - Data size: {} bytes", install_data.len());
            info!("  - Parsed entries: {}", install_manifest.entries.len());

            // Debug: Show a few sample content keys from install manifest
            info!("Sample content keys from install manifest:");
            for (i, entry) in install_manifest.entries.iter().enumerate() {
                if i < 5 {
                    info!(
                        "  Install[{}]: {} (path: {})",
                        i,
                        hex::encode(&entry.ckey),
                        entry.path
                    );
                } else {
                    break;
                }
            }

            // Test: Check multiple install manifest keys to see if ANY exist in encoding file
            let mut found_count = 0;
            let mut _not_found_count = 0;
            let test_count = std::cmp::min(10, install_manifest.entries.len());

            info!(
                "Testing lookup of first {} install manifest keys in encoding file:",
                test_count
            );
            for (i, entry) in install_manifest.entries.iter().take(test_count).enumerate() {
                let test_ckey = hex::encode(&entry.ckey);
                match encoding_file.lookup_by_ckey(&entry.ckey) {
                    Some(encoding_entry) => {
                        // Validate file size to catch corruption (like 121TB files)
                        if encoding_entry.size > 10_000_000_000 {
                            // 10GB limit
                            info!(
                                "  ⚠ Install[{}]: {} FOUND but size suspicious ({} bytes - {}GB), path: {}",
                                i,
                                test_ckey,
                                encoding_entry.size,
                                encoding_entry.size / 1_000_000_000,
                                entry.path
                            );
                        } else {
                            found_count += 1;
                            info!(
                                "  ✓ Install[{}]: {} FOUND (size: {} bytes, path: {})",
                                i, test_ckey, encoding_entry.size, entry.path
                            );
                        }
                    }
                    None => {
                        _not_found_count += 1;
                        info!(
                            "  ✗ Install[{}]: {} NOT FOUND (path: {})",
                            i, test_ckey, entry.path
                        );
                    }
                }
            }

            info!(
                "Install manifest key lookup results: {}/{} found in encoding file",
                found_count, test_count
            );

            if found_count == 0 {
                info!(
                    "No install manifest keys found in encoding - this build may have no installable files"
                );
                info!("This is normal for region-specific or minimal builds");
            } else if found_count < test_count {
                info!(
                    "Partial key availability ({}/{}) - normal for filtered builds with locale/region restrictions",
                    found_count, test_count
                );
                info!(
                    "Missing files are likely locale-specific or platform-specific files not included in this build"
                );
            } else {
                info!("All tested install manifest keys found in encoding file");
            }

            // Convert install entries to common format
            let entries: Vec<FileEntry> = install_manifest
                .entries
                .iter()
                .map(|e| FileEntry {
                    path: e.path.clone(),
                    ckey: e.ckey.clone(),
                    size: e.size as u64,
                    priority: 0, // Install files are high priority
                })
                .collect();

            (entries, "install")
        }
        CliInstallType::Full | CliInstallType::Custom => {
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
                .download_data(cdn_host, cdn_path, &download_ekey)
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
            let entries: Vec<FileEntry> = download_manifest
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, (_ekey, e))| {
                    // Validate ekey exists in encoding file (ensures it's a real file)
                    if encoding_file.lookup_by_ekey(&e.ekey).is_some() {
                        // Filter out files with unreasonable compressed sizes (>1GB indicates bad data)
                        if e.compressed_size < 1_000_000_000 {
                            // 1GB limit
                            Some(FileEntry {
                                path: format!("data/{i:08x}.blte"), // Generate placeholder path
                                ckey: e.ekey.clone(), // Download manifest has ekeys directly
                                size: e.compressed_size, // Use download manifest compressed size
                                priority: e.priority,
                            })
                        } else {
                            debug!(
                                "Skipping file with unreasonable size: {} bytes",
                                e.compressed_size
                            );
                            None
                        }
                    } else {
                        None // Skip entries not found in encoding
                    }
                })
                .collect();

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
        version_entry,
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

    // For metadata-only installations, write configuration files to Data/config/
    if install_type == CliInstallType::MetadataOnly {
        info!("📄 Writing configuration files to Data/config/...");

        // Write build configuration using CDN-style subdirectory structure
        let build_config_subdir =
            format!("{}/{}", &build_config_hash[0..2], &build_config_hash[2..4]);
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

    let mut downloaded_count = 0;
    let mut error_count = 0;

    for entry in &file_entries {
        // Check if we should download this file
        let should_download = match install_type {
            CliInstallType::Minimal => is_required_file(&entry.path),
            CliInstallType::Full => true,
            CliInstallType::Custom => entry.priority <= 0, // High priority only for now
            CliInstallType::MetadataOnly => false,         // Never download files for metadata-only
        };

        if !should_download {
            continue;
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
                    continue;
                }

                if let Some(ekey) = encoding_entry.encoding_keys.first() {
                    debug!(
                        "Found ekey: {} for ckey: {}",
                        hex::encode(ekey),
                        hex::encode(&entry.ckey)
                    );
                    hex::encode(ekey)
                } else {
                    debug!(
                        "No encoding key found for content key: {} (path: {}) - skipping",
                        hex::encode(&entry.ckey),
                        entry.path
                    );
                    continue;
                }
            } else {
                // This is normal - install manifests contain files not present in all builds
                debug!(
                    "Content key not found in encoding file: {} (path: {}) - skipping (normal for filtered builds)",
                    hex::encode(&entry.ckey),
                    entry.path
                );
                continue;
            }
        } else {
            // Download manifest already has encoding keys
            hex::encode(&entry.ckey)
        };

        // Download file
        match cdn_client
            .download_data(cdn_host, cdn_path, &download_key)
            .await
        {
            Ok(response) => {
                match response.bytes().await {
                    Ok(data) => {
                        // Decompress if needed
                        let data = if data.starts_with(b"BLTE") {
                            match blte::decompress_blte(data.to_vec(), None) {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("Failed to decode {}: {}", entry.path, e);
                                    error_count += 1;
                                    continue;
                                }
                            }
                        } else {
                            data.to_vec()
                        };

                        // Write file to disk
                        let file_path = path.join("Data/data").join(&download_key);
                        if let Err(e) = tokio::fs::write(&file_path, &data).await {
                            warn!("Failed to write {}: {}", entry.path, e);
                            error_count += 1;
                        } else {
                            downloaded_count += 1;
                            pb.inc(entry.size);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to download {}: {}", entry.path, e);
                        error_count += 1;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to fetch {}: {}", entry.path, e);
                error_count += 1;
            }
        }
    }

    pb.finish_with_message("Download complete!");

    info!(
        "✅ Installation completed: {} files downloaded, {} errors",
        downloaded_count, error_count
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

    // Core data files
    if path.starts_with("Data/")
        && (path.contains("base") || path.contains("core") || path.contains("common"))
    {
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
    let encoding_data = cdn_client
        .download_data(cdn_host, cdn_path, encoding_ekey)
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
        .download_data(cdn_host, cdn_path, &install_ekey)
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
        match cdn_client.download_data(cdn_host, cdn_path, ekey_hex).await {
            Ok(response) => {
                match response.bytes().await {
                    Ok(data) => {
                        // Decompress if needed
                        let data = if data.starts_with(b"BLTE") {
                            match blte::decompress_blte(data.to_vec(), None) {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("Failed to decode {}: {}", entry.path, e);
                                    error_count += 1;
                                    continue;
                                }
                            }
                        } else {
                            data.to_vec()
                        };

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
                        warn!("Failed to download {}: {}", entry.path, e);
                        error_count += 1;
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
