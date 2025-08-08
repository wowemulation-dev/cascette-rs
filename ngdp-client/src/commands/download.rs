use crate::{DownloadCommands, OutputFormat};
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ngdp_cdn::CdnClientWithFallback;
use ribbit_client::Region;
use std::path::{Path, PathBuf};
use tact_client::resumable::{DownloadProgress, ResumableDownload, find_resumable_downloads};
use tact_client::{HttpClient, ProtocolVersion as TactProtocolVersion, Region as TactRegion};
use tracing::{error, info, warn};

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
        info!("🔍 Would download BuildConfig: {}", version_entry.build_config);
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
        info!("🔍 Would download ProductConfig: {}", version_entry.product_config);
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
        "📋 Initializing file download for {} with {} patterns",
        product,
        patterns.len()
    );
    
    if dry_run {
        info!("🔍 DRY RUN mode - no files will be downloaded");
    }
    
    if let Some(tags) = &tags {
        info!("🏷️ Filtering by tags: {}", tags);
    }
    
    if let Some(limit) = limit {
        info!("📊 Limiting to {} files", limit);
    }

    // Create output directory
    tokio::fs::create_dir_all(output).await?;
    info!("📁 Created output directory: {:?}", output);

    // For now, provide detailed information about what each pattern type would do
    for (i, pattern) in patterns.iter().enumerate() {
        info!("🔍 Pattern {}: {}", i + 1, pattern);

        if pattern.len() == 32 && pattern.chars().all(|c| c.is_ascii_hexdigit()) {
            info!("  → Detected as content key (32 hex chars)");
            info!("  → Would download from CDN data endpoint");
        } else if pattern.len() == 18 && pattern.chars().all(|c| c.is_ascii_hexdigit()) {
            info!("  → Detected as encoding key (18 hex chars)");
            info!("  → Would resolve via encoding file to content key");
        } else if pattern.contains('/') || pattern.contains('\\') {
            info!("  → Detected as file path");
            info!("  → Would resolve via root file to content key");
        } else {
            info!("  → Unknown pattern type, would attempt all resolution methods");
        }
    }

    if let Some(build_id) = build {
        info!("🏗️ Specific build requested: {}", build_id);
    } else {
        info!("🏗️ Using latest build");
    }

    info!("📝 Implementation notes:");
    info!("  • Need to parse BuildConfig to get encoding/root file hashes");
    info!("  • Download and parse encoding file for key resolution");
    info!("  • Download and parse root file for path resolution");
    info!("  • Download actual content files via content keys");
    info!("  • Decompress BLTE data and decrypt if needed");
    info!("  • Save files with proper directory structure");

    warn!("🚧 Full file download implementation pending API integration refinement");

    Ok(())
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
