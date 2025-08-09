use crate::pattern_extraction::{PatternConfig, PatternExtractor};
use crate::{DownloadCommands, OutputFormat};
use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ngdp_cdn::CdnClientWithFallback;
use ribbit_client::Region;
use std::path::{Path, PathBuf};
use tact_client::resumable::{DownloadProgress, ResumableDownload, find_resumable_downloads};
use tact_client::{HttpClient, ProtocolVersion as TactProtocolVersion, Region as TactRegion};
use tracing::{debug, error, info, warn};

pub async fn handle(
    cmd: DownloadCommands,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DownloadCommands::Build {
            product,
            build,
            output,
            region,
            dry_run,
            tags,
        } => {
            info!(
                "Build download requested: product={}, build={}, region={}",
                product, build, region
            );
            info!("Output directory: {:?}", output);

            // Parse region or use US as default
            let region = region.parse::<Region>().unwrap_or(Region::US);

            match download_build(&product, &build, &output, region, dry_run, tags).await {
                Ok(_) => info!("✅ Build download completed successfully!"),
                Err(e) => {
                    error!("❌ Build download failed: {}", e);
                    return Err(e);
                }
            }
        }
        DownloadCommands::Files {
            product,
            patterns,
            output,
            build,
            dry_run,
            tags,
            limit,
        } => {
            info!(
                "File download requested: product={}, patterns={:?}",
                product, patterns
            );
            info!("Output directory: {:?}", output);

            match download_files(&product, &patterns, &output, build, dry_run, tags, limit).await {
                Ok(_) => info!("✅ File download completed successfully!"),
                Err(e) => {
                    error!("❌ File download failed: {}", e);
                    return Err(e);
                }
            }
        }
        DownloadCommands::Resume { session } => {
            info!("Resuming download: session={}", session);

            match resume_download(&session).await {
                Ok(_) => info!("✅ Resume download completed successfully!"),
                Err(e) => {
                    error!("❌ Resume download failed: {}", e);
                    return Err(e);
                }
            }
        }
        DownloadCommands::TestResume {
            hash,
            host,
            output,
            resumable,
        } => {
            info!(
                "Testing resumable download: hash={}, host={}, output={:?}, resumable={}",
                hash, host, output, resumable
            );

            match test_resumable_download(&hash, &host, &output, resumable).await {
                Ok(_) => info!("✅ Test download completed successfully!"),
                Err(e) => {
                    error!("❌ Test download failed: {}", e);
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// Download build files (encoding, root, install manifests)
async fn download_build(
    product: &str,
    build: &str,
    output: &Path,
    region: Region,
    dry_run: bool,
    tags: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "📋 Initializing build download for {} build {}",
        product, build
    );

    if dry_run {
        info!("🔍 DRY RUN mode - no files will be downloaded");
    }

    if let Some(tags) = &tags {
        info!("🏷️ Filtering by tags: {}", tags);
    }

    // Create output directory
    tokio::fs::create_dir_all(output).await?;
    info!("📁 Created output directory: {:?}", output);

    // Initialize clients
    let ribbit_client = CachedRibbitClient::new(region).await?;
    let cdn_client = CachedCdnClient::new().await?;

    info!("🌐 Getting product versions from Ribbit...");
    let versions = ribbit_client.get_product_versions(product).await?;

    // Find the specific build or use latest
    let version_entry = if build.is_empty() || build == "latest" {
        versions
            .entries
            .first()
            .ok_or("No versions available for product")?
    } else {
        versions
            .entries
            .iter()
            .find(|v| v.build_id.to_string() == build || v.versions_name == build)
            .ok_or_else(|| format!("Build '{build}' not found for product '{product}'"))?
    };

    info!(
        "📦 Found build: {} ({})",
        version_entry.versions_name, version_entry.build_id
    );

    // Get CDN configuration
    info!("🌐 Getting CDN configuration...");
    let cdns = ribbit_client.get_product_cdns(product).await?;
    let cdn_entry = cdns.entries.first().ok_or("No CDN servers available")?;

    let cdn_host = cdn_entry.hosts.first().ok_or("No CDN hosts available")?;

    info!("🔗 Using CDN host: {}", cdn_host);

    // Download build configuration
    info!("⬇️ Downloading BuildConfig...");
    if dry_run {
        info!(
            "🔍 Would download BuildConfig: {}",
            version_entry.build_config
        );
    } else {
        let build_config_response = cdn_client
            .download_build_config(cdn_host, &cdn_entry.path, &version_entry.build_config)
            .await?;

        let build_config_data = build_config_response.bytes().await?;

        // Save build config using .build.info compatible structure
        let config_dir = output.join("Data/config");
        tokio::fs::create_dir_all(&config_dir).await?;

        // Save with CDN-style subdirectory structure
        let build_config_hash = &version_entry.build_config;
        let build_config_subdir =
            format!("{}/{}", &build_config_hash[0..2], &build_config_hash[2..4]);
        let build_config_subdir_path = config_dir.join(&build_config_subdir);
        tokio::fs::create_dir_all(&build_config_subdir_path).await?;
        let build_config_path = build_config_subdir_path.join(build_config_hash);
        tokio::fs::write(&build_config_path, &build_config_data).await?;
        info!(
            "💾 Saved BuildConfig to: {}/{}",
            build_config_subdir, build_config_hash
        );

        // Also save legacy flat file for backwards compatibility
        let legacy_path = output.join("build_config");
        tokio::fs::write(&legacy_path, &build_config_data).await?;
        info!("💾 Saved BuildConfig (legacy) to: {:?}", legacy_path);
    }

    // Download CDN configuration
    info!("⬇️ Downloading CDNConfig...");
    if dry_run {
        info!("🔍 Would download CDNConfig: {}", version_entry.cdn_config);
    } else {
        let cdn_config_response = cdn_client
            .download_cdn_config(cdn_host, &cdn_entry.path, &version_entry.cdn_config)
            .await?;

        let cdn_config_data = cdn_config_response.bytes().await?;

        // Save CDN config using .build.info compatible structure
        let config_dir = output.join("Data/config");
        tokio::fs::create_dir_all(&config_dir).await?;

        // Save with CDN-style subdirectory structure
        let cdn_config_hash = &version_entry.cdn_config;
        let cdn_config_subdir = format!("{}/{}", &cdn_config_hash[0..2], &cdn_config_hash[2..4]);
        let cdn_config_subdir_path = config_dir.join(&cdn_config_subdir);
        tokio::fs::create_dir_all(&cdn_config_subdir_path).await?;
        let cdn_config_path = cdn_config_subdir_path.join(cdn_config_hash);
        tokio::fs::write(&cdn_config_path, &cdn_config_data).await?;
        info!(
            "💾 Saved CDNConfig to: {}/{}",
            cdn_config_subdir, cdn_config_hash
        );

        // Also save legacy flat file for backwards compatibility
        let legacy_path = output.join("cdn_config");
        tokio::fs::write(&legacy_path, &cdn_config_data).await?;
        info!("💾 Saved CDNConfig (legacy) to: {:?}", legacy_path);
    }

    // Download product configuration
    info!("⬇️ Downloading ProductConfig...");
    if dry_run {
        info!(
            "🔍 Would download ProductConfig: {}",
            version_entry.product_config
        );
    } else {
        let product_config_response = cdn_client
            .download_product_config(
                cdn_host,
                &cdn_entry.config_path,
                &version_entry.product_config,
            )
            .await?;

        let product_config_path = output.join("product_config");
        tokio::fs::write(&product_config_path, product_config_response.bytes().await?).await?;
        info!("💾 Saved ProductConfig to: {:?}", product_config_path);
    }

    // Download keyring if available
    if let Some(keyring_hash) = &version_entry.key_ring {
        info!("⬇️ Downloading KeyRing...");
        if dry_run {
            info!("🔍 Would download KeyRing: {}", keyring_hash);
        } else {
            let keyring_response = cdn_client
                .download_key_ring(cdn_host, &cdn_entry.path, keyring_hash)
                .await?;

            let keyring_path = output.join("keyring");
            tokio::fs::write(&keyring_path, keyring_response.bytes().await?).await?;
            info!("💾 Saved KeyRing to: {:?}", keyring_path);
        }
    }

    if dry_run {
        info!("✅ Dry run completed - showed what would be downloaded");
    } else {
        // Generate .build.info file for compatibility with install commands
        info!("📄 Writing .build.info file...");

        let region_enum = match region {
            Region::US => ribbit_client::Region::US,
            Region::EU => ribbit_client::Region::EU,
            Region::KR => ribbit_client::Region::KR,
            Region::TW => ribbit_client::Region::TW,
            _ => ribbit_client::Region::US,
        };

        write_build_info_for_download(
            output,
            product,
            version_entry,
            &version_entry.build_config,
            &version_entry.cdn_config,
            cdn_entry,
            region_enum,
        )
        .await?;

        info!("✓ .build.info file written");
        info!("✅ Build download completed successfully!");
        info!("📂 Files saved to: {:?}", output);
        info!(
            "💡 Use 'ngdp download resume {}' to continue incomplete installations",
            output.display()
        );
    }

    Ok(())
}

/// Download specific files by patterns (content keys, encoding keys, or paths)
async fn download_files(
    product: &str,
    patterns: &[String],
    output: &Path,
    build: Option<String>,
    dry_run: bool,
    tags: Option<String>,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "📋 Initializing pattern-based file download for {} with {} patterns",
        product,
        patterns.len()
    );

    if dry_run {
        info!("🔍 DRY RUN mode - analyzing patterns and showing matches");
    }

    if let Some(tags) = &tags {
        info!("🏷️ Filtering by tags: {}", tags);
    }

    if let Some(limit) = limit {
        info!("📊 Limiting to {} files per pattern", limit);
    }

    // Create output directory
    tokio::fs::create_dir_all(output).await?;
    info!("📁 Created output directory: {:?}", output);

    // Initialize pattern extractor with configuration
    let pattern_config = PatternConfig {
        max_matches_per_pattern: limit,
        ..Default::default()
    };

    let mut extractor = PatternExtractor::with_config(pattern_config);

    // Add all patterns to the extractor
    for pattern in patterns {
        match extractor.add_pattern(pattern) {
            Ok(()) => info!("✅ Added pattern: {}", pattern),
            Err(e) => {
                error!("❌ Invalid pattern '{}': {}", pattern, e);
                return Err(format!("Invalid pattern '{pattern}': {e}").into());
            }
        }
    }

    // Show pattern statistics
    let stats = extractor.get_stats();
    info!("📊 Pattern Analysis:");
    info!("  • Total patterns: {}", stats.total_patterns);
    info!("  • Glob patterns: {}", stats.glob_patterns);
    info!("  • Regex patterns: {}", stats.regex_patterns);
    info!("  • Content keys: {}", stats.content_keys);
    info!("  • Encoding keys: {}", stats.encoding_keys);
    info!("  • File paths: {}", stats.file_paths);

    if dry_run {
        // For dry run, demonstrate pattern matching with sample data
        info!("🔍 DRY RUN: Demonstrating pattern matching with sample file list");

        let sample_files = get_sample_file_list();
        let matches = extractor.match_files(&sample_files);

        if matches.is_empty() {
            info!("📝 No matches found in sample data");
            info!("💡 Sample files available for testing:");
            for (i, file) in sample_files.iter().take(10).enumerate() {
                info!("  {}: {}", i + 1, file);
            }
        } else {
            info!("🎯 Found {} pattern matches in sample data:", matches.len());

            for (i, pattern_match) in matches.iter().take(20).enumerate() {
                info!(
                    "  {}: {} (pattern: {}, priority: {})",
                    i + 1,
                    pattern_match.file_path,
                    pattern_match.pattern,
                    pattern_match.metadata.priority_score
                );
            }

            if matches.len() > 20 {
                info!("  ... and {} more matches", matches.len() - 20);
            }
        }

        info!("✅ Dry run completed - patterns would be applied to real manifest data");
        return Ok(());
    }

    // Initialize clients for actual download
    let region = Region::US; // Default region, could be parameterized
    let ribbit_client = CachedRibbitClient::new(region).await?;
    let cdn_client = CachedCdnClient::new().await?;

    info!("🌐 Getting product versions from Ribbit...");
    let versions = ribbit_client.get_product_versions(product).await?;

    // Find the specific build or use latest
    let version_entry = if let Some(build_id) = build {
        versions
            .entries
            .iter()
            .find(|v| v.build_id.to_string() == build_id || v.versions_name == build_id)
            .ok_or_else(|| format!("Build '{build_id}' not found for product '{product}'"))?
    } else {
        versions
            .entries
            .first()
            .ok_or("No versions available for product")?
    };

    info!(
        "📦 Found build: {} ({})",
        version_entry.versions_name, version_entry.build_id
    );

    // Get CDN configuration
    info!("🌐 Getting CDN configuration...");
    let cdns = ribbit_client.get_product_cdns(product).await?;
    let cdn_entry = cdns.entries.first().ok_or("No CDN servers available")?;
    let cdn_host = cdn_entry.hosts.first().ok_or("No CDN hosts available")?;

    info!("🔗 Using CDN host: {}", cdn_host);

    // Download and parse build configuration to get manifest hashes
    info!("⬇️ Downloading BuildConfig...");
    let build_config_response = cdn_client
        .download_build_config(cdn_host, &cdn_entry.path, &version_entry.build_config)
        .await?;

    let build_config_data = build_config_response.bytes().await?;

    // Parse build configuration to extract manifest file hashes
    let build_config_text = String::from_utf8_lossy(&build_config_data);

    info!("📋 Parsing BuildConfig to extract manifest hashes...");
    let (encoding_hash, root_hash, install_hash) = parse_build_config_hashes(&build_config_text)?;

    info!("🔑 Found manifest hashes:");
    info!("  • Encoding: {}", encoding_hash);
    info!("  • Root: {}", root_hash.as_deref().unwrap_or("None"));
    info!("  • Install: {}", install_hash.as_deref().unwrap_or("None"));

    // For now, demonstrate what would happen with real manifest integration
    info!("🚧 Next steps for full implementation:");
    info!("  1. Download and decompress BLTE-encoded encoding file");
    info!("  2. Parse encoding file to build CKey → EKey mapping");
    info!("  3. Download and decompress root file if available");
    info!("  4. Parse root file to build path → CKey mapping");
    info!("  5. Apply patterns to real file list from manifest");
    info!("  6. Download matched files from CDN data endpoint");
    info!("  7. Decompress BLTE data and save with directory structure");

    // Apply patterns to mock data for demonstration
    let mock_file_list = get_comprehensive_file_list();
    let matches = extractor.match_files(&mock_file_list);

    if matches.is_empty() {
        warn!("📝 No pattern matches found");
        return Ok(());
    }

    info!(
        "🎯 Pattern matching results: {} files matched",
        matches.len()
    );

    // Show what files would be downloaded
    for (i, pattern_match) in matches.iter().take(limit.unwrap_or(10)).enumerate() {
        info!(
            "  {}: {} (pattern: '{}', priority: {})",
            i + 1,
            pattern_match.file_path,
            pattern_match.pattern,
            pattern_match.metadata.priority_score
        );

        // Show file type if detected
        if let Some(file_type) = &pattern_match.metadata.file_type {
            debug!("    File type: {}", file_type);
        }
    }

    info!("✅ Pattern-based file extraction analysis completed!");
    info!("💡 Use --dry-run to see pattern matching without attempting downloads");

    warn!(
        "🚧 Full manifest integration and download implementation pending TACT parser integration"
    );

    Ok(())
}

type BuildConfigResult =
    Result<(String, Option<String>, Option<String>), Box<dyn std::error::Error>>;

/// Parse build configuration to extract manifest file hashes
fn parse_build_config_hashes(build_config: &str) -> BuildConfigResult {
    let mut encoding_hash = None;
    let mut root_hash = None;
    let mut install_hash = None;

    for line in build_config.lines() {
        let line = line.trim();
        if line.starts_with("encoding = ") {
            encoding_hash = Some(
                line.split_whitespace()
                    .nth(2)
                    .unwrap_or_default()
                    .to_string(),
            );
        } else if line.starts_with("root = ") {
            root_hash = Some(
                line.split_whitespace()
                    .nth(2)
                    .unwrap_or_default()
                    .to_string(),
            );
        } else if line.starts_with("install = ") {
            install_hash = Some(
                line.split_whitespace()
                    .nth(2)
                    .unwrap_or_default()
                    .to_string(),
            );
        }
    }

    let encoding = encoding_hash.ok_or("No encoding hash found in build config")?;

    Ok((encoding, root_hash, install_hash))
}

/// Get sample file list for pattern testing
fn get_sample_file_list() -> Vec<String> {
    vec![
        "achievement.dbc".to_string(),
        "spell.dbc".to_string(),
        "item.db2".to_string(),
        "world/maps/azeroth/azeroth.wdt".to_string(),
        "interface/framexml/uiparent.lua".to_string(),
        "interface/addons/blizzard_auctionui/blizzard_auctionui.lua".to_string(),
        "sound/music/zonemusic/stormwind.ogg".to_string(),
        "sound/spells/frostbolt.ogg".to_string(),
        "textures/interface/buttons/ui-button.blp".to_string(),
        "creature/human/male/humanmale.m2".to_string(),
        "world/wmo/stormwind/stormwind_keep.wmo".to_string(),
    ]
}

/// Get comprehensive file list for pattern testing
fn get_comprehensive_file_list() -> Vec<String> {
    vec![
        // Database files
        "achievement.dbc".to_string(),
        "spell.dbc".to_string(),
        "item.db2".to_string(),
        "creature.dbc".to_string(),
        "gameobject.dbc".to_string(),
        // Interface files
        "interface/framexml/uiparent.lua".to_string(),
        "interface/framexml/worldframe.lua".to_string(),
        "interface/framexml/chatframe.lua".to_string(),
        "interface/addons/blizzard_auctionui/blizzard_auctionui.lua".to_string(),
        "interface/addons/blizzard_raidui/blizzard_raidui.lua".to_string(),
        "interface/framexml/uiparent.xml".to_string(),
        // Sound files
        "sound/music/zonemusic/stormwind.ogg".to_string(),
        "sound/music/zonemusic/ironforge.ogg".to_string(),
        "sound/spells/frostbolt.ogg".to_string(),
        "sound/spells/fireball.ogg".to_string(),
        "sound/creature/human/humanvoicemale01.ogg".to_string(),
        // Texture files
        "textures/interface/buttons/ui-button.blp".to_string(),
        "textures/interface/icons/spell_frost_frostbolt.blp".to_string(),
        "textures/world/azeroth/stormwind/stormwind_cobblestone.blp".to_string(),
        "textures/character/human/male/humanmale_face00_00.blp".to_string(),
        // 3D Models
        "creature/human/male/humanmale.m2".to_string(),
        "creature/orc/male/orcmale.m2".to_string(),
        "item/weapon/sword/2h_sword_01.m2".to_string(),
        // World files
        "world/maps/azeroth/azeroth.wdt".to_string(),
        "world/maps/azeroth/azeroth_31_49.adt".to_string(),
        "world/wmo/stormwind/stormwind_keep.wmo".to_string(),
        "world/wmo/ironforge/ironforge_main.wmo".to_string(),
        // Misc files
        "fonts/frizqt__.ttf".to_string(),
        "tileset/generic/dirt.blp".to_string(),
        "character/human/male/humanmale.skin".to_string(),
        "character/bloodelf/female/bloodelffemale.skin".to_string(),
    ]
}

/// Resume a download from a progress file, directory, or installation with .build.info
async fn resume_download(session: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session_path = PathBuf::from(session);

    if session_path.is_dir() {
        // Check if this is an installation directory with .build.info
        let build_info_path = session_path.join(".build.info");
        if build_info_path.exists() {
            info!(
                "🏗️ Detected installation directory with .build.info: {:?}",
                session_path
            );
            return resume_from_installation(&session_path).await;
        }

        // Find all resumable downloads in the directory (existing behavior)
        info!(
            "🔍 Searching for resumable downloads in: {:?}",
            session_path
        );
        let downloads = find_resumable_downloads(&session_path).await?;

        if downloads.is_empty() {
            warn!("No resumable downloads found in directory");
            return Ok(());
        }

        info!("Found {} resumable download(s):", downloads.len());
        for (i, progress) in downloads.iter().enumerate() {
            info!(
                "  {}: {} - {}",
                i + 1,
                progress.file_hash,
                progress.progress_string()
            );
        }

        // Resume the first one (in a real CLI, you'd prompt for choice)
        let progress = &downloads[0];
        info!("Resuming first download: {}", progress.file_hash);

        let client = create_tact_client().await?;
        let mut resumable_download = ResumableDownload::new(client, progress.clone());
        resumable_download.start_or_resume().await?;
        resumable_download.cleanup_completed().await?;
    } else if session_path.extension().and_then(|s| s.to_str()) == Some("download") {
        // Resume specific progress file
        info!("📂 Loading progress from: {:?}", session_path);
        let progress = DownloadProgress::load_from_file(&session_path).await?;

        info!(
            "Resuming: {} - {}",
            progress.file_hash,
            progress.progress_string()
        );

        let client = create_tact_client().await?;
        let mut resumable_download = ResumableDownload::new(client, progress);
        resumable_download.start_or_resume().await?;
        resumable_download.cleanup_completed().await?;
    } else {
        return Err(format!(
            "Invalid session path: {session}. Must be a directory, .download file, or installation with .build.info"
        )
        .into());
    }

    Ok(())
}

/// Resume download from an existing installation with .build.info and Data/config structure
async fn resume_from_installation(install_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

    info!("📦 Installation details:");
    info!("  • Product: {}", product);
    info!("  • Version: {}", version);
    info!("  • Branch: {}", branch);
    info!("  • Build Key: {}", build_key);
    info!("  • CDN Host: {}", cdn_host);
    info!("  • CDN Path: {}", cdn_path);

    // Read build configuration from Data/config/ structure
    let build_config_subdir = format!("{}/{}", &build_key[0..2], &build_key[2..4]);
    let build_config_path = install_path
        .join("Data/config")
        .join(&build_config_subdir)
        .join(build_key);

    if !build_config_path.exists() {
        return Err(format!(
            "Build configuration not found at: {}. Run metadata-only installation first.",
            build_config_path.display()
        )
        .into());
    }

    let build_config_data = tokio::fs::read_to_string(&build_config_path).await?;
    let build_config = tact_parser::config::BuildConfig::parse(&build_config_data)?;

    info!("✓ Loaded build configuration from local cache");

    // Initialize CDN client for downloading
    let cdn_client = CachedCdnClient::new().await?;

    // Get install manifest information
    let install_value = build_config
        .config
        .get_value("install")
        .ok_or("Missing install field in build config")?;
    let install_parts: Vec<&str> = install_value.split_whitespace().collect();

    // Use encoding key if available, otherwise use content key
    let install_ekey = if install_parts.len() >= 2 {
        install_parts[1].to_string()
    } else {
        // Need to look up content key in encoding file first
        return Err("Install manifest content key lookup not yet implemented for resume. Use direct encoding key.".into());
    };

    info!(
        "📥 Resuming installation using install manifest: {}",
        install_ekey
    );

    // Download and parse install manifest
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

    let install_manifest = tact_parser::install::InstallManifest::parse(&install_data)?;

    info!(
        "📋 Install manifest loaded: {} files",
        install_manifest.entries.len()
    );

    // Check which files are missing from Data/data/
    let data_dir = install_path.join("Data/data");
    tokio::fs::create_dir_all(&data_dir).await?;

    let mut missing_files = Vec::new();
    let mut total_missing_size = 0u64;

    info!("🔍 Checking for missing files...");
    for entry in &install_manifest.entries {
        // For install manifest, we need encoding key to download
        // For now, assume the path contains the encoding key (simplified)
        let ckey_hex = hex::encode(&entry.ckey);
        let expected_path = data_dir.join(&ckey_hex);

        if !expected_path.exists() {
            missing_files.push(entry);
            total_missing_size += entry.size as u64;
        }
    }

    if missing_files.is_empty() {
        info!("✅ No missing files found - installation appears complete!");
        return Ok(());
    }

    info!(
        "📊 Found {} missing files ({} bytes total)",
        missing_files.len(),
        format_bytes(total_missing_size)
    );

    // For now, just report what would be downloaded
    info!("🚧 Resume functionality implementation in progress");
    info!("📋 Missing files that would be downloaded:");
    for (i, entry) in missing_files.iter().take(10).enumerate() {
        info!("  {}: {} ({} bytes)", i + 1, entry.path, entry.size);
    }

    if missing_files.len() > 10 {
        info!("  ... and {} more files", missing_files.len() - 10);
    }

    info!(
        "💡 Use 'ngdp install game {} --path {} --resume' for full resume functionality",
        product,
        install_path.display()
    );

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

/// Test resumable download functionality
async fn test_resumable_download(
    hash: &str,
    _host: &str,
    output: &Path,
    resumable: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate hash format
    if hash.len() != 32 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hash format. Expected 32 hex characters.".into());
    }

    info!("🚀 Starting test download");
    info!("📋 Hash: {}", hash);
    info!("📁 Output: {:?}", output);
    info!("🔄 Resumable: {}", resumable);

    if resumable {
        // Use resumable download
        info!("📥 Starting resumable download...");

        let progress = DownloadProgress::new(
            hash.to_string(),
            "blzddist1-a.akamaihd.net".to_string(),
            "/tpr/wow/data".to_string(),
            output.to_path_buf(),
        );

        let client = create_tact_client().await?;
        let mut resumable_download = ResumableDownload::new(client, progress);

        resumable_download.start_or_resume().await?;
        resumable_download.cleanup_completed().await?;
    } else {
        // Use CDN client with fallback for regular download
        info!("📥 Starting regular CDN download with fallback...");

        let cdn_client = CdnClientWithFallback::new()?;
        let response = cdn_client.download_data("/tpr/wow", hash).await?;
        let bytes = response.bytes().await?;

        tokio::fs::write(output, bytes).await?;
        info!("💾 Saved to: {:?}", output);
    }

    // Show file info
    if let Ok(metadata) = tokio::fs::metadata(output).await {
        info!("📊 Downloaded {} bytes", metadata.len());
    }

    Ok(())
}

/// Create a TACT HTTP client configured for downloads
async fn create_tact_client() -> Result<HttpClient, Box<dyn std::error::Error>> {
    let client = HttpClient::new(TactRegion::US, TactProtocolVersion::V2)?
        .with_max_retries(3)
        .with_initial_backoff_ms(1000)
        .with_user_agent("ngdp-client/0.3.1");

    Ok(client)
}

/// Write .build.info file for downloaded build configurations
async fn write_build_info_for_download(
    output_path: &Path,
    product: &str,
    version_entry: &ribbit_client::VersionEntry,
    build_config_hash: &str,
    cdn_config_hash: &str,
    cdn_entry: &ribbit_client::CdnEntry,
    region: ribbit_client::Region,
) -> Result<(), Box<dyn std::error::Error>> {
    // For download command, we may not have parsed the build config yet
    // Use placeholder values and let the user know
    let install_key = ""; // Empty - not available without parsing build config

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
        BpsvValue::Hex(install_key.to_string()),        // Install Key (empty)
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

    // Write .build.info file to output directory
    let build_info_path = output_path.join(".build.info");
    tokio::fs::write(&build_info_path, build_info_content).await?;

    debug!("Written .build.info to: {}", build_info_path.display());
    Ok(())
}
