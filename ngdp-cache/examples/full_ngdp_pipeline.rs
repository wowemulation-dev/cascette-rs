//! Full NGDP pipeline example using CachedRibbitClient and CachedCdnClient
//!
//! This example demonstrates the complete NGDP content fetching pipeline:
//! 1. Fetching version information from Ribbit
//! 2. Getting CDN server lists
//! 3. Downloading all configuration files:
//!    - BuildConfig (from {path}/config)
//!    - CDNConfig (from {path}/config)
//!    - ProductConfig (from {config_path})
//!    - KeyRing (from {path}/config, if available)
//! 4. Parsing configuration files to show data structure
//! 5. All responses are cached for improved performance

use ngdp_cache::{cached_cdn_client::CachedCdnClient, cached_ribbit_client::CachedRibbitClient};
use ribbit_client::{
    Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region, TypedResponse,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tracing::{Level, debug, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// Products to demonstrate
const PRODUCTS: &[&str] = &["wow", "wow_classic", "wow_classic_era", "agent"];

/// Configuration file types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ConfigType {
    BuildConfig,
    CdnConfig,
    ProductConfig,
    KeyRing,
}

impl ConfigType {
    fn name(&self) -> &'static str {
        match self {
            Self::BuildConfig => "BuildConfig",
            Self::CdnConfig => "CDNConfig",
            Self::ProductConfig => "ProductConfig",
            Self::KeyRing => "KeyRing",
        }
    }
}

/// Download statistics
#[derive(Default)]
struct DownloadStats {
    attempts: HashMap<ConfigType, u32>,
    successes: HashMap<ConfigType, u32>,
    bytes: HashMap<ConfigType, usize>,
    from_cache: HashMap<ConfigType, u32>,
}

impl DownloadStats {
    fn record_attempt(&mut self, config_type: ConfigType) {
        *self.attempts.entry(config_type).or_insert(0) += 1;
    }

    fn record_success(&mut self, config_type: ConfigType, bytes: usize, from_cache: bool) {
        *self.successes.entry(config_type).or_insert(0) += 1;
        *self.bytes.entry(config_type).or_insert(0) += bytes;
        if from_cache {
            *self.from_cache.entry(config_type).or_insert(0) += 1;
        }
    }

    fn print_summary(&self) {
        info!("\n📊 Download Statistics by Type:");
        for config_type in &[
            ConfigType::BuildConfig,
            ConfigType::CdnConfig,
            ConfigType::ProductConfig,
            ConfigType::KeyRing,
        ] {
            let attempts = self.attempts.get(config_type).unwrap_or(&0);
            let successes = self.successes.get(config_type).unwrap_or(&0);
            let bytes = self.bytes.get(config_type).unwrap_or(&0);
            let cached = self.from_cache.get(config_type).unwrap_or(&0);

            if *attempts > 0 {
                info!(
                    "  {}: {}/{} successful, {} bytes total, {} from cache",
                    config_type.name(),
                    successes,
                    attempts,
                    bytes,
                    cached
                );
            }
        }

        let total_attempts: u32 = self.attempts.values().sum();
        let total_successes: u32 = self.successes.values().sum();
        let total_bytes: usize = self.bytes.values().sum();
        let total_cached: u32 = self.from_cache.values().sum();

        info!(
            "\n  Total: {}/{} successful, {} bytes, {} from cache",
            total_successes, total_attempts, total_bytes, total_cached
        );
    }
}

/// Try downloading from multiple CDN hosts with fallback
async fn download_config_with_fallback(
    cdn_client: &CachedCdnClient,
    hosts: &[String],
    cdn_path: &str,
    config_path: &str,
    hash: &str,
    config_type: ConfigType,
) -> Option<(Vec<u8>, bool)> {
    for (i, host) in hosts.iter().enumerate() {
        debug!("    Trying CDN host {} of {}: {}", i + 1, hosts.len(), host);

        let result = match config_type {
            ConfigType::BuildConfig => cdn_client.download_build_config(host, cdn_path, hash).await,
            ConfigType::CdnConfig => cdn_client.download_cdn_config(host, cdn_path, hash).await,
            ConfigType::ProductConfig => {
                cdn_client
                    .download_product_config(host, config_path, hash)
                    .await
            }
            ConfigType::KeyRing => cdn_client.download_key_ring(host, cdn_path, hash).await,
        };

        match result {
            Ok(response) => {
                // TODO: switch to File API
                let is_cached = response.is_from_cache();
                let mut data = response.to_inner();
                let size = data.metadata().await.map(|m| m.len()).unwrap_or_default();
                info!(
                    "    ✓ Downloaded {} ({} bytes, cached: {})",
                    config_type.name(),
                    size,
                    is_cached
                );
                let mut buf = Vec::with_capacity(size as usize);
                data.read_to_end(&mut buf).await.unwrap();
                return Some((buf, is_cached));
            }
            Err(e) => {
                debug!("    ✗ Failed to download from {}: {}", host, e);
            }
        }

        // Small delay before trying next host
        if i < hosts.len() - 1 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    None
}

/// Parse and display config file preview
fn preview_config(data: &[u8], config_type: ConfigType, max_lines: usize) {
    match String::from_utf8(data.to_vec()) {
        Ok(text) => {
            info!("    📄 {} preview:", config_type.name());
            for (i, line) in text.lines().take(max_lines).enumerate() {
                info!("       {}: {}", i + 1, line);
            }
            let total_lines = text.lines().count();
            if total_lines > max_lines {
                info!("       ... ({} more lines)", total_lines - max_lines);
            }
        }
        Err(_) => {
            // Try to show as hex if not valid UTF-8
            info!(
                "    📄 {} (binary data, first 64 bytes):",
                config_type.name()
            );
            let preview = &data[..data.len().min(64)];
            info!("       {:02x?}", preview);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Full NGDP Pipeline Example ===\n");
    info!("This example demonstrates the complete NGDP content fetching pipeline.");
    info!("All configuration files will be downloaded and cached.\n");

    // Create cached clients
    info!("Creating cached clients...");
    let ribbit_client = CachedRibbitClient::new(Region::US).await?;
    let cdn_client = CachedCdnClient::new().await?;

    // Track download statistics
    let mut stats = DownloadStats::default();

    // Process each product
    for product in PRODUCTS {
        info!("\n{}", "=".repeat(60));
        info!("📦 Processing product: {}", product);
        info!("{}", "=".repeat(60));
        let start = Instant::now();

        // Step 1: Get version information
        info!("\n1️⃣ Fetching version information...");
        let versions_endpoint = Endpoint::ProductVersions(product.to_string());

        match ribbit_client.request(&versions_endpoint).await {
            Ok(response) => {
                match ProductVersionsResponse::from_response(&response) {
                    Ok(versions) => {
                        info!("   Found {} version entries", versions.entries.len());

                        // Show available regions
                        let regions: Vec<_> =
                            versions.entries.iter().map(|v| v.region.as_str()).collect();
                        info!("   Available regions: {:?}", regions);

                        // Step 2: Get CDN information
                        info!("\n2️⃣ Fetching CDN information...");
                        let cdns_endpoint = Endpoint::ProductCdns(product.to_string());

                        match ribbit_client.request(&cdns_endpoint).await {
                            Ok(cdn_response) => {
                                match ProductCdnsResponse::from_response(&cdn_response) {
                                    Ok(cdns) => {
                                        info!("   Found {} CDN configurations", cdns.entries.len());

                                        // Use the first available CDN
                                        if let Some(cdn) = cdns.entries.first() {
                                            info!("   Using CDN: {}", cdn.name);
                                            info!("   CDN path: {}", cdn.path);
                                            info!("   CDN config path: {}", cdn.config_path);
                                            info!("   Available hosts: {}", cdn.hosts.len());

                                            // Step 3: Download configuration files for the first region
                                            if let Some(version) = versions.entries.first() {
                                                info!(
                                                    "\n3️⃣ Downloading configuration files for region: {}",
                                                    version.region
                                                );
                                                info!("   Version: {}", version.versions_name);
                                                info!("   Build ID: {}", version.build_id);

                                                // Download BuildConfig
                                                if !version.build_config.is_empty()
                                                    && version.build_config != "0"
                                                {
                                                    info!(
                                                        "\n   📥 BuildConfig: {}",
                                                        version.build_config
                                                    );
                                                    stats.record_attempt(ConfigType::BuildConfig);

                                                    if let Some((data, is_cached)) =
                                                        download_config_with_fallback(
                                                            &cdn_client,
                                                            &cdn.hosts,
                                                            &cdn.path,
                                                            &cdn.config_path,
                                                            &version.build_config,
                                                            ConfigType::BuildConfig,
                                                        )
                                                        .await
                                                    {
                                                        stats.record_success(
                                                            ConfigType::BuildConfig,
                                                            data.len(),
                                                            is_cached,
                                                        );
                                                        preview_config(
                                                            &data,
                                                            ConfigType::BuildConfig,
                                                            5,
                                                        );
                                                    }
                                                }

                                                // Download CDNConfig
                                                if !version.cdn_config.is_empty()
                                                    && version.cdn_config != "0"
                                                {
                                                    info!(
                                                        "\n   📥 CDNConfig: {}",
                                                        version.cdn_config
                                                    );
                                                    stats.record_attempt(ConfigType::CdnConfig);

                                                    if let Some((data, is_cached)) =
                                                        download_config_with_fallback(
                                                            &cdn_client,
                                                            &cdn.hosts,
                                                            &cdn.path,
                                                            &cdn.config_path,
                                                            &version.cdn_config,
                                                            ConfigType::CdnConfig,
                                                        )
                                                        .await
                                                    {
                                                        stats.record_success(
                                                            ConfigType::CdnConfig,
                                                            data.len(),
                                                            is_cached,
                                                        );
                                                        preview_config(
                                                            &data,
                                                            ConfigType::CdnConfig,
                                                            5,
                                                        );
                                                    }
                                                }

                                                // Download ProductConfig
                                                if !version.product_config.is_empty()
                                                    && version.product_config != "0"
                                                {
                                                    info!(
                                                        "\n   📥 ProductConfig: {}",
                                                        version.product_config
                                                    );
                                                    stats.record_attempt(ConfigType::ProductConfig);

                                                    if let Some((data, is_cached)) =
                                                        download_config_with_fallback(
                                                            &cdn_client,
                                                            &cdn.hosts,
                                                            &cdn.path,
                                                            &cdn.config_path,
                                                            &version.product_config,
                                                            ConfigType::ProductConfig,
                                                        )
                                                        .await
                                                    {
                                                        stats.record_success(
                                                            ConfigType::ProductConfig,
                                                            data.len(),
                                                            is_cached,
                                                        );
                                                        preview_config(
                                                            &data,
                                                            ConfigType::ProductConfig,
                                                            5,
                                                        );
                                                    }
                                                }

                                                // Download KeyRing (if available)
                                                if let Some(key_ring) = &version.key_ring {
                                                    if !key_ring.is_empty() && key_ring != "0" {
                                                        info!("\n   📥 KeyRing: {}", key_ring);
                                                        stats.record_attempt(ConfigType::KeyRing);

                                                        if let Some((data, is_cached)) =
                                                            download_config_with_fallback(
                                                                &cdn_client,
                                                                &cdn.hosts,
                                                                &cdn.path,
                                                                &cdn.config_path,
                                                                key_ring,
                                                                ConfigType::KeyRing,
                                                            )
                                                            .await
                                                        {
                                                            stats.record_success(
                                                                ConfigType::KeyRing,
                                                                data.len(),
                                                                is_cached,
                                                            );
                                                            preview_config(
                                                                &data,
                                                                ConfigType::KeyRing,
                                                                5,
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    info!(
                                                        "\n   ℹ️  No KeyRing defined for this version"
                                                    );
                                                }
                                            }
                                        } else {
                                            warn!("   No CDN entries found for {}", product);
                                        }
                                    }
                                    Err(e) => {
                                        error!("   Failed to parse CDN response: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("   Failed to fetch CDN information: {}", e);
                            }
                        }

                        let elapsed = start.elapsed();
                        info!("\n⏱️  Product {} completed in {:.2?}", product, elapsed);
                    }
                    Err(e) => {
                        error!("   Failed to parse version response: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("   Failed to fetch version information: {}", e);
            }
        }
    }

    // Display download statistics
    stats.print_summary();

    // Display cache statistics
    info!("\n📁 Cache Statistics:");
    match cdn_client.cache_stats().await {
        Ok(cache_stats) => {
            info!("  Total cached files: {}", cache_stats.total_files);
            info!("  Total cache size: {}", cache_stats.total_size_human());
            info!(
                "  - Config files: {} ({})",
                cache_stats.config_files,
                cache_stats.config_size_human()
            );
            info!(
                "  - Data files: {} ({})",
                cache_stats.data_files,
                cache_stats.data_size_human()
            );
            info!(
                "  - Patch files: {} ({})",
                cache_stats.patch_files,
                cache_stats.patch_size_human()
            );
        }
        Err(e) => {
            warn!("  Failed to get cache statistics: {}", e);
        }
    }

    info!("\n✅ Full pipeline example completed!");
    info!("   All configuration files have been downloaded and cached.");
    info!("   Run again to see cache performance improvements!");

    Ok(())
}
