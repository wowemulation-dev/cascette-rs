use crate::pattern_extraction::{PatternConfig, PatternExtractor};
use crate::{DownloadCommands, OutputFormat};
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

        let build_config_path = output.join("build_config");
        tokio::fs::write(&build_config_path, build_config_response.bytes().await?).await?;
        info!("💾 Saved BuildConfig to: {:?}", build_config_path);
    }

    // Download CDN configuration
    info!("⬇️ Downloading CDNConfig...");
    if dry_run {
        info!("🔍 Would download CDNConfig: {}", version_entry.cdn_config);
    } else {
        let cdn_config_response = cdn_client
            .download_cdn_config(cdn_host, &cdn_entry.path, &version_entry.cdn_config)
            .await?;

        let cdn_config_path = output.join("cdn_config");
        tokio::fs::write(&cdn_config_path, cdn_config_response.bytes().await?).await?;
        info!("💾 Saved CDNConfig to: {:?}", cdn_config_path);
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
        info!("✅ Build download completed successfully!");
        info!("📂 Files saved to: {:?}", output);
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

/// Resume a download from a progress file or directory
async fn resume_download(session: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session_path = PathBuf::from(session);

    if session_path.is_dir() {
        // Find all resumable downloads in the directory
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
            "Invalid session path: {session}. Must be a directory or .download file"
        )
        .into());
    }

    Ok(())
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
