use crate::{DownloadCommands, OutputFormat};
use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ribbit_client::Region;
use std::path::Path;
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
        } => {
            info!(
                "Build download requested: product={}, build={}, region={}",
                product, build, region
            );
            info!("Output directory: {:?}", output);
            
            // Parse region or use US as default
            let region = region.parse::<Region>().unwrap_or(Region::US);
            
            match download_build(&product, &build, &output, region).await {
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
        } => {
            info!(
                "File download requested: product={}, patterns={:?}",
                product, patterns
            );
            info!("Output directory: {:?}", output);
            
            match download_files(&product, &patterns, &output, build).await {
                Ok(_) => info!("✅ File download completed successfully!"),
                Err(e) => {
                    error!("❌ File download failed: {}", e);
                    return Err(e);
                }
            }
        }
        DownloadCommands::Resume { session } => {
            warn!("Resume download not yet implemented");
            info!("Session: {}", session);
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
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📋 Initializing build download for {} build {}", product, build);
    
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
        versions.entries.first()
            .ok_or("No versions available for product")?
    } else {
        versions.entries.iter()
            .find(|v| v.build_id.to_string() == build || v.versions_name == build)
            .ok_or_else(|| format!("Build '{}' not found for product '{}'", build, product))?
    };
    
    info!("📦 Found build: {} ({})", version_entry.versions_name, version_entry.build_id);
    
    // Get CDN configuration
    info!("🌐 Getting CDN configuration...");
    let cdns = ribbit_client.get_product_cdns(product).await?;
    let cdn_entry = cdns.entries.first()
        .ok_or("No CDN servers available")?;
    
    let cdn_host = cdn_entry.hosts.first()
        .ok_or("No CDN hosts available")?;
    
    info!("🔗 Using CDN host: {}", cdn_host);
    
    // Download build configuration
    info!("⬇️ Downloading BuildConfig...");
    let build_config_response = cdn_client.download_build_config(
        cdn_host,
        &cdn_entry.path,
        &version_entry.build_config
    ).await?;
    
    let build_config_path = output.join("build_config");
    tokio::fs::write(&build_config_path, build_config_response.bytes().await?).await?;
    info!("💾 Saved BuildConfig to: {:?}", build_config_path);
    
    // Download CDN configuration  
    info!("⬇️ Downloading CDNConfig...");
    let cdn_config_response = cdn_client.download_cdn_config(
        cdn_host,
        &cdn_entry.path,
        &version_entry.cdn_config
    ).await?;
    
    let cdn_config_path = output.join("cdn_config");
    tokio::fs::write(&cdn_config_path, cdn_config_response.bytes().await?).await?;
    info!("💾 Saved CDNConfig to: {:?}", cdn_config_path);
    
    // Download product configuration
    info!("⬇️ Downloading ProductConfig...");
    let product_config_response = cdn_client.download_product_config(
        cdn_host,
        &cdn_entry.config_path,
        &version_entry.product_config
    ).await?;
    
    let product_config_path = output.join("product_config");
    tokio::fs::write(&product_config_path, product_config_response.bytes().await?).await?;
    info!("💾 Saved ProductConfig to: {:?}", product_config_path);
    
    // Download keyring if available
    if let Some(keyring_hash) = &version_entry.key_ring {
        info!("⬇️ Downloading KeyRing...");
        let keyring_response = cdn_client.download_key_ring(
            cdn_host,
            &cdn_entry.path,
            keyring_hash
        ).await?;
        
        let keyring_path = output.join("keyring");
        tokio::fs::write(&keyring_path, keyring_response.bytes().await?).await?;
        info!("💾 Saved KeyRing to: {:?}", keyring_path);
    }
    
    info!("✅ Build download completed successfully!");
    info!("📂 Files saved to: {:?}", output);
    
    Ok(())
}

/// Download specific files by patterns (content keys, encoding keys, or paths)
async fn download_files(
    product: &str,
    patterns: &[String],
    output: &Path,
    build: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📋 Initializing file download for {} with {} patterns", product, patterns.len());
    
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
