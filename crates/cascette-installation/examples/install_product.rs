#![allow(clippy::expect_used, clippy::panic)]
//! Installation pipeline demo
//!
//! Demonstrates `InstallConfig` creation, `InstallPipeline` construction,
//! `ProgressEvent` handling, and the expected CASC directory layout.
//! Does not perform a real installation (no CDN access required).
//!
//! ```text
//! cargo run -p cascette-installation --example install_product --features local-install
//! ```

mod common;

use std::path::Path;

use cascette_installation::InstallPipeline;
use cascette_installation::config::InstallConfig;
use cascette_installation::progress::ProgressEvent;
use cascette_protocol::CdnEndpoint;

#[tokio::main]
async fn main() {
    println!("=== InstallConfig Creation ===");
    println!();

    // InstallConfig::new takes product name, install path, and CDN path.
    // All other fields use defaults (region "us", locale "enUS", etc.).
    let install_path = common::default_wow_path();
    let mut config = InstallConfig::new(
        "wow_classic_era".to_string(),
        install_path.clone(),
        "tpr/wow".to_string(),
    );

    println!("Product:     {}", config.product);
    println!("Install path: {}", config.install_path.display());
    println!("CDN path:    {}", config.cdn_path);
    println!("Region:      {}", config.region);
    println!("Locale:      {}", config.locale);
    println!("Platform:    {:?}", config.platform_tags);
    println!();

    println!("=== CDN Endpoints ===");
    println!();

    // CDN endpoints are typically resolved from Ribbit, but can be set manually.
    // Each endpoint has a host, path, and optional query parameters.
    config.endpoints.push(CdnEndpoint {
        host: "level3.blizzard.com".to_string(),
        path: "tpr/wow".to_string(),
        product_path: None,
        scheme: Some("https".to_string()),
        is_fallback: false,
        strict: false,
        max_hosts: Some(4),
    });

    config.endpoints.push(CdnEndpoint {
        host: "cdn.blizzard.com".to_string(),
        path: "tpr/wow".to_string(),
        product_path: None,
        scheme: Some("https".to_string()),
        is_fallback: true,
        strict: false,
        max_hosts: None,
    });

    for (i, ep) in config.endpoints.iter().enumerate() {
        let scheme = ep.scheme.as_deref().unwrap_or("https");
        println!(
            "  Endpoint {}: {}://{}/{} (fallback={})",
            i, scheme, ep.host, ep.path, ep.is_fallback
        );
    }
    println!();

    println!("=== Tuning Parameters ===");
    println!();
    println!(
        "Max connections per host: {}",
        config.max_connections_per_host
    );
    println!(
        "Max connections global:   {}",
        config.max_connections_global
    );
    println!("Index batch size:         {}", config.index_batch_size);
    println!("Checkpoint interval:      {}", config.checkpoint_interval);
    println!("Resume from checkpoint:   {}", config.resume);
    println!();

    // Build and CDN config hashes are normally resolved via Ribbit.
    // Setting them explicitly skips the resolution step.
    config.build_config = Some("abc123def456abc123def456abc123de".to_string());
    config.cdn_config = Some("fedcba987654fedcba987654fedcba98".to_string());

    println!(
        "Build config: {}",
        config.build_config.as_deref().unwrap_or("(auto-resolve)")
    );
    println!(
        "CDN config:   {}",
        config.cdn_config.as_deref().unwrap_or("(auto-resolve)")
    );
    println!();

    println!("=== InstallPipeline ===");
    println!();

    let _pipeline = InstallPipeline::new(config);

    println!("Pipeline created. In a real scenario, call pipeline.run() with:");
    println!("  - An Arc<CdnSource> (e.g., CdnClient)");
    println!("  - A Vec<CdnEndpoint>");
    println!("  - A progress callback: impl Fn(ProgressEvent) + Send + Sync");
    println!();
    println!("The pipeline progresses through these states:");
    println!("  1. FetchingConfigs     -- download build config and CDN config");
    println!("  2. ClassifyingArtifacts -- determine which files to download");
    println!("  3. FetchingArchiveIndices -- download .index files");
    println!("  4. Downloading         -- download archive data");
    println!("  5. WritingLayout       -- write .build.info, .product.db, Data/config/");
    println!("  6. Complete            -- returns InstallReport");
    println!();

    println!("=== ProgressEvent Variants ===");
    println!();

    // Demonstrate what progress events look like during installation.
    let demo_events: Vec<ProgressEvent> = vec![
        ProgressEvent::MetadataResolving {
            product: "wow_classic_era".to_string(),
        },
        ProgressEvent::MetadataResolved {
            artifacts: 42_000,
            total_bytes: 30_000_000_000,
        },
        ProgressEvent::ArchiveIndexDownloading {
            index: 0,
            total: 256,
        },
        ProgressEvent::ArchiveIndexComplete {
            archive_key: "abc123def456".to_string(),
        },
        ProgressEvent::FileDownloading {
            path: "encoding_key_hex".to_string(),
            size: 1_048_576,
        },
        ProgressEvent::FileComplete {
            path: "encoding_key_hex".to_string(),
        },
        ProgressEvent::FileFailed {
            path: "bad_key_hex".to_string(),
            error: "HTTP 404".to_string(),
        },
        ProgressEvent::CheckpointSaved {
            completed: 5000,
            remaining: 37_000,
        },
    ];

    for event in &demo_events {
        println!("  {event:?}");
    }
    println!();

    println!("=== Expected Directory Layout ===");
    println!();

    let base = install_path;
    print_layout_tree(&base);
    println!();

    println!("=== CDN URL Patterns ===");
    println!();
    println!("Config files:");
    println!("  https://{{host}}/{{cdn_path}}/config/{{hash[0..2]}}/{{hash[2..4]}}/{{hash}}");
    println!();
    println!("Data files:");
    println!("  https://{{host}}/{{cdn_path}}/data/{{hash[0..2]}}/{{hash[2..4]}}/{{hash}}");
    println!();
    println!("Archive index files:");
    println!("  https://{{host}}/{{cdn_path}}/data/{{hash[0..2]}}/{{hash[2..4]}}/{{hash}}.index");
    println!();
    println!("Patch files:");
    println!("  https://{{host}}/{{cdn_path}}/patch/{{hash[0..2]}}/{{hash[2..4]}}/{{hash}}");
    println!();

    println!("=== Done ===");
    println!();
    println!("This was a dry-run demonstration. No files were downloaded.");
}

/// Print the expected CASC directory tree.
fn print_layout_tree(base: &Path) {
    let b = base.display();
    println!("  {b}/");
    println!("  +-- .build.info          (BPSV format, identifies installed build)");
    println!("  +-- .product.db          (protobuf-like binary, product metadata)");
    println!("  +-- Data/");
    println!("  |   +-- config/");
    println!("  |   |   +-- ab/cd/abcd...  (build config and CDN config, hash-path layout)");
    println!("  |   +-- data/");
    println!("  |   |   +-- data.000      (archive files containing BLTE-encoded data)");
    println!("  |   |   +-- data.001");
    println!("  |   |   +-- ...");
    println!("  |   +-- indices/");
    println!("  |       +-- <key>.index   (archive index files)");
    println!("  +-- _classic_era_/        (extracted game files, if extract is run)");
}
