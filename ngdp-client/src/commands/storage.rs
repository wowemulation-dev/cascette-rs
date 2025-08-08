use crate::commands::listfile::parse_listfile;
use crate::{OutputFormat, StorageCommands};
use casc_storage::{CascStorage, ConfigDiscovery, ManifestConfig, types::CascConfig};
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tact_parser::wow_root::LocaleFlags;
use tracing::{debug, error, info, warn};

pub async fn handle(
    cmd: StorageCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        StorageCommands::Init { path, product } => handle_init(path, product).await,
        StorageCommands::Info { path } => handle_info(path, format).await,
        StorageCommands::Config { path } => handle_config(path, format).await,
        StorageCommands::Stats { path } => handle_stats(path, format).await,
        StorageCommands::Verify { path, fix } => handle_verify(path, fix, format).await,
        StorageCommands::Read { path, ekey, output } => handle_read(path, ekey, output).await,
        StorageCommands::Write { path, ekey, input } => handle_write(path, ekey, input).await,
        StorageCommands::List {
            path,
            detailed,
            limit,
        } => handle_list(path, detailed, limit, format).await,
        StorageCommands::Rebuild { path, force } => handle_rebuild(path, force).await,
        StorageCommands::Optimize { path } => handle_optimize(path).await,
        StorageCommands::Repair { path, dry_run } => handle_repair(path, dry_run).await,
        StorageCommands::Clean { path, dry_run } => handle_clean(path, dry_run).await,
        StorageCommands::Extract {
            ekey,
            path,
            output,
            listfile,
            resolve_filename,
        } => handle_extract(ekey, path, output, listfile, resolve_filename, format).await,
        StorageCommands::ExtractById {
            fdid,
            path,
            output,
            root_manifest,
            encoding_manifest,
        } => {
            handle_extract_by_id(fdid, path, output, root_manifest, encoding_manifest, format).await
        }
        StorageCommands::ExtractByName {
            filename,
            path,
            output,
            root_manifest,
            encoding_manifest,
            listfile,
        } => {
            handle_extract_by_name(
                filename,
                path,
                output,
                root_manifest,
                encoding_manifest,
                listfile,
                format,
            )
            .await
        }
        StorageCommands::LoadManifests {
            path,
            root_manifest,
            encoding_manifest,
            listfile,
            locale,
            info_only,
        } => {
            handle_load_manifests(
                path,
                root_manifest,
                encoding_manifest,
                listfile,
                locale,
                info_only,
                format,
            )
            .await
        }
    }
}

async fn handle_init(
    path: PathBuf,
    product: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Initializing CASC storage at {path:?}");

    // Check if path exists and is a valid CASC data directory
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    if !data_path.exists() {
        // Create the necessary directory structure
        fs::create_dir_all(&data_path)?;
        fs::create_dir_all(data_path.join("indices"))?;
        fs::create_dir_all(data_path.join("data"))?;

        println!("✅ Created CASC storage structure at {data_path:?}");
    } else {
        println!("ℹ️  Directory already exists at {data_path:?}");
    }

    // Try to open as CASC storage to verify
    match CascStorage::new(CascConfig {
        data_path: data_path.clone(),
        read_only: false,
        ..Default::default()
    }) {
        Ok(storage) => {
            storage.flush()?;
            println!("✅ CASC storage initialized successfully");
            if let Some(product) = product {
                println!("📦 Product: {}", product.cyan());
            }
        }
        Err(e) => {
            error!("Failed to initialize storage: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_info(
    path: PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    debug!("Opening CASC storage at {:?}", data_path);

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Test EKey lookup to debug the issue
    if std::env::var("TEST_EKEY_LOOKUP").is_ok() {
        info!("Running EKey lookup test...");
        let _ = storage.test_ekey_lookup();
    }

    let stats = storage.stats();

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json = serde_json::json!({
                "path": data_path,
                "archives": stats.total_archives,
                "indices": stats.total_indices,
                "total_size": stats.total_size,
                "file_count": stats.file_count,
                "duplicate_count": stats.duplicate_count,
                "compression_ratio": stats.compression_ratio,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("\n📁 CASC Storage Information");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("  Path:         {data_path:?}");
            println!(
                "  Archives:     {}",
                stats.total_archives.to_string().green()
            );
            println!(
                "  Indices:      {}",
                stats.total_indices.to_string().green()
            );
            println!(
                "  Total Size:   {}",
                format_bytes(stats.total_size).yellow()
            );
            println!("  File Count:   {}", stats.file_count.to_string().cyan());
            if stats.duplicate_count > 0 {
                println!(
                    "  Duplicates:   {}",
                    stats.duplicate_count.to_string().magenta()
                );
            }
            if stats.compression_ratio > 0.0 {
                println!("  Compression:  {:.1}%", (stats.compression_ratio * 100.0));
            }
        }
        OutputFormat::Bpsv => {
            // BPSV format for scripting
            println!("path = {data_path:?}");
            println!("archives = {}", stats.total_archives);
            println!("indices = {}", stats.total_indices);
            println!("total_size = {}", stats.total_size);
            println!("file_count = {}", stats.file_count);
        }
    }

    Ok(())
}

async fn handle_config(
    path: PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Discovering NGDP configurations at {:?}", path);

    match ConfigDiscovery::discover_configs(&path) {
        Ok(config_set) => match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "config_dir": config_set.config_dir,
                    "cdn_configs": config_set.cdn_configs.len(),
                    "build_configs": config_set.build_configs.len(),
                    "archive_hashes": config_set.all_archive_hashes(),
                    "file_index_hashes": config_set.file_index_hashes(),
                });

                if matches!(format, OutputFormat::JsonPretty) {
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("{}", serde_json::to_string(&json)?);
                }
            }
            OutputFormat::Text => {
                println!("\n🔧 NGDP Configuration Information");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Config Dir:   {:?}", config_set.config_dir);
                println!(
                    "  CDN Configs:  {}",
                    config_set.cdn_configs.len().to_string().green()
                );
                println!(
                    "  Build Configs: {}",
                    config_set.build_configs.len().to_string().green()
                );

                if let Some(cdn_config) = config_set.latest_cdn_config() {
                    println!("\n📦 Latest CDN Configuration");
                    println!(
                        "  Archives:     {}",
                        cdn_config.archives().len().to_string().cyan()
                    );
                    if let Some(archive_group) = cdn_config.archive_group() {
                        println!("  Archive Group: {archive_group}");
                    }
                    if let Some(file_index) = cdn_config.file_index() {
                        println!("  File Index:   {file_index}");
                    }

                    println!("\n  Archive Hashes (first 5):");
                    for (i, archive) in cdn_config.archives().iter().take(5).enumerate() {
                        println!("    {}: {}", i + 1, archive);
                    }
                    if cdn_config.archives().len() > 5 {
                        println!("    ... and {} more", cdn_config.archives().len() - 5);
                    }
                }

                if let Some(build_config) = config_set.latest_build_config() {
                    println!("\n🏗️  Latest Build Configuration");
                    if let Some(build_name) = build_config.build_name() {
                        println!("  Build Name:   {}", build_name.yellow());
                    }
                    if let Some(root_hash) = build_config.root_hash() {
                        println!("  Root Hash:    {root_hash}");
                    }
                    if let Some(encoding_hash) = build_config.encoding_hash() {
                        println!("  Encoding Hash: {encoding_hash}");
                    }
                    if let Some(install_hash) = build_config.install_hash() {
                        println!("  Install Hash: {install_hash}");
                    }
                }
            }
            OutputFormat::Bpsv => {
                println!("## NGDP Configuration");
                println!("config_dir = {:?}", config_set.config_dir);
                println!("cdn_configs = {}", config_set.cdn_configs.len());
                println!("build_configs = {}", config_set.build_configs.len());

                if let Some(cdn_config) = config_set.latest_cdn_config() {
                    println!("archives_count = {}", cdn_config.archives().len());
                    for (i, archive) in cdn_config.archives().iter().enumerate() {
                        println!("archive_{i} = {archive}");
                    }
                }
            }
        },
        Err(e) => match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "error": format!("Failed to discover configs: {}", e),
                    "path": path,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                println!("❌ Failed to discover NGDP configurations: {e}");
                println!("   Path: {path:?}");
                println!("   Hint: Make sure the path points to a WoW installation directory");
            }
            OutputFormat::Bpsv => {
                println!("error = {e}");
                println!("path = {path:?}");
            }
        },
    }

    Ok(())
}

async fn handle_stats(
    path: PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    let stats = storage.stats();

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json = serde_json::json!({
                "total_archives": stats.total_archives,
                "total_indices": stats.total_indices,
                "total_size": stats.total_size,
                "file_count": stats.file_count,
                "duplicate_count": stats.duplicate_count,
                "compression_ratio": stats.compression_ratio,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic);

            table.set_header(vec![
                Cell::new("Metric").add_attribute(Attribute::Bold),
                Cell::new("Value").add_attribute(Attribute::Bold),
            ]);

            table.add_row(vec!["Total Archives", &stats.total_archives.to_string()]);
            table.add_row(vec!["Total Indices", &stats.total_indices.to_string()]);
            table.add_row(vec!["Total Size", &format_bytes(stats.total_size)]);
            table.add_row(vec!["File Count", &stats.file_count.to_string()]);
            table.add_row(vec!["Duplicate Count", &stats.duplicate_count.to_string()]);
            table.add_row(vec![
                "Compression Ratio",
                &format!("{:.2}%", stats.compression_ratio * 100.0),
            ]);

            println!("\n📊 CASC Storage Statistics");
            println!("{table}");
        }
        OutputFormat::Bpsv => {
            println!("## Storage Statistics");
            println!("total_archives = {}", stats.total_archives);
            println!("total_indices = {}", stats.total_indices);
            println!("total_size = {}", stats.total_size);
            println!("file_count = {}", stats.file_count);
            println!("duplicate_count = {}", stats.duplicate_count);
            println!("compression_ratio = {}", stats.compression_ratio);
        }
    }

    Ok(())
}

async fn handle_verify(
    path: PathBuf,
    fix: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    println!("🔍 Verifying CASC storage at {data_path:?}");
    if fix {
        println!("🔧 Fix mode enabled - will attempt repairs");
    }

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: !fix,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    let errors = storage.verify()?;

    if errors.is_empty() {
        println!("✅ Storage verification complete: all files OK");
    } else {
        println!("❌ Storage verification found {} errors", errors.len());

        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "errors": errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
                    "count": errors.len(),
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                if errors.len() <= 10 {
                    for ekey in &errors {
                        println!("  ❌ Failed: {ekey}");
                    }
                } else {
                    for ekey in errors.iter().take(10) {
                        println!("  ❌ Failed: {ekey}");
                    }
                    println!("  ... and {} more", errors.len() - 10);
                }
            }
            OutputFormat::Bpsv => {
                for ekey in &errors {
                    println!("error = {ekey}");
                }
            }
        }

        if fix {
            warn!("Repair functionality not yet implemented");
        }
    }

    Ok(())
}

async fn handle_read(
    path: PathBuf,
    ekey: String,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let ekey_bytes = hex::decode(&ekey)?;
    if ekey_bytes.len() != 16 && ekey_bytes.len() != 9 {
        return Err("EKey must be 16 or 9 bytes (32 or 18 hex characters)".into());
    }

    let config = CascConfig {
        data_path,
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Convert to EKey type
    let ekey = if ekey_bytes.len() == 9 {
        // Expand truncated key
        let mut full_key = [0u8; 16];
        full_key[0..9].copy_from_slice(&ekey_bytes);
        casc_storage::types::EKey::new(full_key)
    } else {
        casc_storage::types::EKey::from_slice(&ekey_bytes).ok_or("Invalid EKey format")?
    };

    debug!("Reading file with EKey: {}", ekey);
    let data = storage.read(&ekey)?;

    if let Some(output_path) = output {
        fs::write(&output_path, &data)?;
        println!("✅ Wrote {} bytes to {:?}", data.len(), output_path);
    } else {
        io::stdout().write_all(&data)?;
    }

    Ok(())
}

async fn handle_write(
    path: PathBuf,
    ekey: String,
    input: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let ekey_bytes = hex::decode(&ekey)?;
    if ekey_bytes.len() != 16 && ekey_bytes.len() != 9 {
        return Err("EKey must be 16 or 9 bytes (32 or 18 hex characters)".into());
    }

    let config = CascConfig {
        data_path,
        read_only: false,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Convert to EKey type
    let ekey = if ekey_bytes.len() == 9 {
        let mut full_key = [0u8; 16];
        full_key[0..9].copy_from_slice(&ekey_bytes);
        casc_storage::types::EKey::new(full_key)
    } else {
        casc_storage::types::EKey::from_slice(&ekey_bytes).ok_or("Invalid EKey format")?
    };

    let data = if let Some(input_path) = input {
        fs::read(&input_path)?
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };

    debug!("Writing {} bytes with EKey: {}", data.len(), ekey);
    storage.write(&ekey, &data)?;
    storage.flush()?;

    println!("✅ Wrote {} bytes to storage", data.len());
    Ok(())
}

async fn handle_list(
    path: PathBuf,
    detailed: bool,
    limit: Option<usize>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    println!("📋 Listing files in CASC storage");

    let limit = limit.unwrap_or(if detailed { 100 } else { 1000 });

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let files: Vec<serde_json::Value> = storage
                .enumerate_files()
                .take(limit)
                .map(|(ekey, location)| {
                    serde_json::json!({
                        "ekey": ekey.to_string(),
                        "archive_id": location.archive_id,
                        "offset": format!("0x{:x}", location.offset),
                        "size": location.size
                    })
                })
                .collect();

            let json = serde_json::json!({
                "total_files": storage.stats().file_count,
                "shown": files.len(),
                "files": files
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", serde_json::to_string(&json)?);
            }
        }
        OutputFormat::Text => {
            println!("Total files: {}", storage.stats().file_count);
            println!("Showing first {limit} files:\n");

            if detailed {
                println!(
                    "{:<34} {:<8} {:<12} {:<8}",
                    "EKey", "Archive", "Offset", "Size"
                );
                println!("{}", "─".repeat(70));

                for (i, (ekey, location)) in storage.enumerate_files().take(limit).enumerate() {
                    println!(
                        "{:<34} {:<8} 0x{:<10x} {:<8}",
                        ekey.to_string(),
                        location.archive_id,
                        location.offset,
                        location.size
                    );

                    if i > 0 && (i + 1) % 10 == 0 {
                        println!(); // Add spacing every 10 rows
                    }
                }
            } else {
                // Simple format - just EKeys
                for (i, (ekey, _)) in storage.enumerate_files().take(limit).enumerate() {
                    print!("{ekey} ");
                    if (i + 1) % 4 == 0 {
                        println!(); // 4 EKeys per line
                    }
                }
                println!();
            }

            let total = storage.stats().file_count;
            if (limit as u64) < total {
                println!("\n... and {} more files", total - limit as u64);
            }

            // Show files per archive breakdown
            if detailed {
                println!("\n📊 Files per archive:");
                let mut archive_counts: Vec<_> = storage.files_per_archive().into_iter().collect();
                archive_counts.sort_by_key(|(id, _)| *id);

                for (archive_id, count) in archive_counts {
                    println!("  Archive {archive_id}: {count} files");
                }
            }
        }
        OutputFormat::Bpsv => {
            println!("## CASC File List");
            println!("total_files = {}", storage.stats().file_count);
            println!("shown = {}", limit.min(storage.stats().file_count as usize));

            for (ekey, location) in storage.enumerate_files().take(limit) {
                println!(
                    "file = {} {} 0x{:x} {}",
                    ekey, location.archive_id, location.offset, location.size
                );
            }
        }
    }

    Ok(())
}

async fn handle_rebuild(path: PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    println!("🔨 Rebuilding indices for CASC storage at {data_path:?}");
    if force {
        println!("⚠️  Force mode enabled - rebuilding all indices");
    }

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: false,
        ..Default::default()
    };

    let storage = CascStorage::new(config)?;

    storage.rebuild_indices()?;

    println!("✅ Indices rebuilt successfully");
    Ok(())
}

async fn handle_optimize(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    println!("⚡ Optimizing CASC storage at {data_path:?}");

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: false,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Clear cache to free memory
    storage.clear_cache();

    // Flush any pending writes
    storage.flush()?;

    println!("✅ Storage optimized successfully");
    Ok(())
}

async fn handle_repair(path: PathBuf, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    println!("🔧 Repairing CASC storage at {data_path:?}");
    if dry_run {
        println!("🔍 Dry run mode - no changes will be made");
    }

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: dry_run,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    let errors = storage.verify()?;

    if errors.is_empty() {
        println!("✅ No errors found - storage is healthy");
    } else {
        println!("❌ Found {} errors", errors.len());

        if !dry_run {
            // Attempt to rebuild indices which might fix some issues
            storage.rebuild_indices()?;
            println!("✅ Rebuilt indices");

            // Verify again
            let remaining_errors = storage.verify()?;
            if remaining_errors.len() < errors.len() {
                println!("✅ Fixed {} errors", errors.len() - remaining_errors.len());
            }
            if !remaining_errors.is_empty() {
                println!("⚠️  {} errors remain unfixed", remaining_errors.len());
            }
        }
    }

    Ok(())
}

async fn handle_clean(path: PathBuf, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    println!("🧹 Cleaning CASC storage at {data_path:?}");
    if dry_run {
        println!("🔍 Dry run mode - no files will be deleted");
    }

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: dry_run,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Clear the cache
    storage.clear_cache();
    println!("✅ Cleared cache");

    // Note: Additional cleanup operations would require more API from casc-storage
    // such as removing orphaned files, compacting archives, etc.

    Ok(())
}

async fn handle_extract(
    ekey: String,
    path: PathBuf,
    output: Option<PathBuf>,
    listfile: Option<PathBuf>,
    resolve_filename: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let ekey_bytes = hex::decode(&ekey)?;
    debug!(
        "Parsed EKey bytes: {:?} (length: {})",
        ekey_bytes,
        ekey_bytes.len()
    );
    if ekey_bytes.len() != 16 && ekey_bytes.len() != 9 {
        return Err("EKey must be 16 or 9 bytes (32 or 18 hex characters)".into());
    }

    let config = CascConfig {
        data_path,
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new_async(config).await?;

    // Convert to EKey type
    let ekey_obj = if ekey_bytes.len() == 9 {
        // Expand truncated key
        let mut full_key = [0u8; 16];
        full_key[0..9].copy_from_slice(&ekey_bytes);
        casc_storage::types::EKey::new(full_key)
    } else {
        casc_storage::types::EKey::from_slice(&ekey_bytes).ok_or("Invalid EKey format")?
    };

    debug!("Extracting file with EKey: {}", ekey);
    let bucket = ekey_obj.bucket_index();
    debug!("EKey {} maps to bucket {:02x}", ekey, bucket);
    let data = storage.read(&ekey_obj)?;

    // Try to resolve filename if requested
    let resolved_filename: Option<String> = None;
    if resolve_filename {
        if let Some(listfile_path) = &listfile {
            if listfile_path.exists() {
                match parse_listfile(listfile_path) {
                    Ok(mapping) => {
                        // For now, we can't map EKey to FileDataID without TACT manifests
                        // This is a placeholder for future enhancement
                        info!(
                            "Listfile loaded with {} entries, but EKey->FileDataID mapping not yet implemented",
                            mapping.len()
                        );
                        warn!("Filename resolution requires TACT manifest integration");
                    }
                    Err(e) => {
                        warn!("Failed to parse listfile: {}", e);
                    }
                }
            } else {
                warn!("Listfile not found at {:?}", listfile_path);
            }
        } else {
            // Try default listfile path
            let default_listfile = PathBuf::from("community-listfile.csv");
            if default_listfile.exists() {
                match parse_listfile(&default_listfile) {
                    Ok(mapping) => {
                        info!("Loaded default listfile with {} entries", mapping.len());
                        warn!("Filename resolution requires TACT manifest integration");
                    }
                    Err(e) => {
                        warn!("Failed to parse default listfile: {}", e);
                    }
                }
            }
        }
    }

    // Determine output path
    let output_path = if let Some(path) = output {
        path
    } else if let Some(ref filename) = resolved_filename {
        PathBuf::from(filename)
    } else {
        // Use EKey as filename
        PathBuf::from(format!("{ekey}.bin"))
    };

    // Write the file
    if output_path.to_string_lossy() == "-" {
        // Output to stdout
        io::stdout().write_all(&data)?;
    } else {
        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&output_path, &data)?;

        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "status": "success",
                    "ekey": ekey,
                    "output_path": output_path,
                    "size": data.len(),
                    "filename_resolved": resolved_filename.is_some()
                });

                if matches!(format, OutputFormat::JsonPretty) {
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("{}", serde_json::to_string(&json)?);
                }
            }
            OutputFormat::Text => {
                println!("✅ Extracted file successfully!");
                println!("   EKey:   {}", ekey.cyan());
                println!("   Size:   {} bytes", data.len().to_string().green());
                println!("   Output: {:?}", output_path.bright_blue());

                if resolved_filename.is_some() {
                    println!("   📝 Filename resolved from listfile");
                } else {
                    println!("   📝 Used EKey as filename (no resolution available)");
                }
            }
            OutputFormat::Bpsv => {
                println!("status = success");
                println!("ekey = {ekey}");
                println!("output_path = {output_path:?}");
                println!("size = {}", data.len());
                println!("filename_resolved = {}", resolved_filename.is_some());
            }
        }
    }

    Ok(())
}

async fn handle_extract_by_id(
    fdid: u32,
    path: PathBuf,
    output: Option<PathBuf>,
    root_manifest: Option<PathBuf>,
    encoding_manifest: Option<PathBuf>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        cache_size_mb: 256,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
    };

    let mut storage = CascStorage::new(config)?;
    storage.load_indices()?;
    storage.load_archives()?;

    // Initialize TACT manifests
    let manifest_config = ManifestConfig {
        locale: LocaleFlags::any_locale(),
        content_flags: None,
        cache_manifests: true,
        lazy_loading: true,       // Enable lazy loading by default
        lazy_cache_limit: 50_000, // Higher limit for CLI usage
    };
    storage.init_tact_manifests(manifest_config);

    // Load manifests
    if let Some(root_path) = root_manifest {
        storage.load_root_manifest_from_file(&root_path)?;
        info!("Loaded root manifest from {:?}", root_path);
    }

    if let Some(encoding_path) = encoding_manifest {
        storage.load_encoding_manifest_from_file(&encoding_path)?;
        info!("Loaded encoding manifest from {:?}", encoding_path);
    }

    if !storage.tact_manifests_loaded() {
        return Err(
            "TACT manifests not loaded. Use --root-manifest and --encoding-manifest".into(),
        );
    }

    // Extract file by FileDataID
    debug!("Extracting FileDataID: {}", fdid);
    let data = storage.read_by_fdid(fdid)?;

    // Determine output path
    let output_path = output.unwrap_or_else(|| PathBuf::from(format!("fdid_{fdid}.bin")));

    // Write the file
    if output_path.to_string_lossy() == "-" {
        io::stdout().write_all(&data)?;
    } else {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, &data)?;

        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "status": "success",
                    "fdid": fdid,
                    "output_path": output_path,
                    "size": data.len()
                });

                if matches!(format, OutputFormat::JsonPretty) {
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("{}", serde_json::to_string(&json)?);
                }
            }
            OutputFormat::Text => {
                println!("✅ Extracted file successfully!");
                println!("   FileDataID: {}", fdid.to_string().cyan());
                println!("   Size:       {} bytes", data.len().to_string().green());
                println!("   Output:     {:?}", output_path.bright_blue());
            }
            OutputFormat::Bpsv => {
                println!("status = success");
                println!("fdid = {fdid}");
                println!("output_path = {output_path:?}");
                println!("size = {}", data.len());
            }
        }
    }

    Ok(())
}

async fn handle_extract_by_name(
    filename: String,
    path: PathBuf,
    output: Option<PathBuf>,
    root_manifest: Option<PathBuf>,
    encoding_manifest: Option<PathBuf>,
    listfile: Option<PathBuf>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        cache_size_mb: 256,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
    };

    let mut storage = CascStorage::new(config)?;
    storage.load_indices()?;
    storage.load_archives()?;

    // Initialize TACT manifests
    let manifest_config = ManifestConfig {
        locale: LocaleFlags::any_locale(),
        content_flags: None,
        cache_manifests: true,
        lazy_loading: true,       // Enable lazy loading by default
        lazy_cache_limit: 50_000, // Higher limit for CLI usage
    };
    storage.init_tact_manifests(manifest_config);

    // Load manifests
    if let Some(root_path) = root_manifest {
        storage.load_root_manifest_from_file(&root_path)?;
        info!("Loaded root manifest from {:?}", root_path);
    }

    if let Some(encoding_path) = encoding_manifest {
        storage.load_encoding_manifest_from_file(&encoding_path)?;
        info!("Loaded encoding manifest from {:?}", encoding_path);
    }

    // Load listfile if provided
    if let Some(listfile_path) = listfile {
        let count = storage.load_listfile(&listfile_path)?;
        info!("Loaded {} filename mappings", count);
    }

    if !storage.tact_manifests_loaded() {
        return Err(
            "TACT manifests not loaded. Use --root-manifest and --encoding-manifest".into(),
        );
    }

    // Extract file by filename
    debug!("Extracting filename: {}", filename);
    let data = storage.read_by_filename(&filename)?;

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        // Use original filename or sanitize it
        let safe_filename = filename.replace(['\\', '/', ':'], "_");
        PathBuf::from(safe_filename)
    });

    // Write the file
    if output_path.to_string_lossy() == "-" {
        io::stdout().write_all(&data)?;
    } else {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, &data)?;

        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "status": "success",
                    "filename": filename,
                    "output_path": output_path,
                    "size": data.len()
                });

                if matches!(format, OutputFormat::JsonPretty) {
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("{}", serde_json::to_string(&json)?);
                }
            }
            OutputFormat::Text => {
                println!("✅ Extracted file successfully!");
                println!("   Filename: {}", filename.cyan());
                println!("   Size:     {} bytes", data.len().to_string().green());
                println!("   Output:   {:?}", output_path.bright_blue());
            }
            OutputFormat::Bpsv => {
                println!("status = success");
                println!("filename = {filename}");
                println!("output_path = {output_path:?}");
                println!("size = {}", data.len());
            }
        }
    }

    Ok(())
}

async fn handle_load_manifests(
    path: PathBuf,
    root_manifest: Option<PathBuf>,
    encoding_manifest: Option<PathBuf>,
    listfile: Option<PathBuf>,
    locale: String,
    info_only: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = if path.ends_with("Data") {
        path.clone()
    } else {
        path.join("Data")
    };

    // Parse locale
    let locale_flags = match locale.to_lowercase().as_str() {
        "all" => LocaleFlags::any_locale(),
        "en_us" => LocaleFlags::new().with_en_us(true),
        "de_de" => LocaleFlags::new().with_de_de(true),
        "fr_fr" => LocaleFlags::new().with_fr_fr(true),
        "es_es" => LocaleFlags::new().with_es_es(true),
        "zh_cn" => LocaleFlags::new().with_zh_cn(true),
        "zh_tw" => LocaleFlags::new().with_zh_tw(true),
        "ko_kr" => LocaleFlags::new().with_ko_kr(true),
        "ru_ru" => LocaleFlags::new().with_ru_ru(true),
        _ => {
            warn!("Unknown locale '{}', using 'all'", locale);
            LocaleFlags::any_locale()
        }
    };

    let config = CascConfig {
        data_path: data_path.clone(),
        read_only: true,
        cache_size_mb: 256,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
    };

    let mut storage = CascStorage::new(config)?;
    storage.load_indices()?;
    storage.load_archives()?;

    // Initialize TACT manifests
    let manifest_config = ManifestConfig {
        locale: locale_flags,
        content_flags: None,
        cache_manifests: true,
        lazy_loading: true,       // Enable lazy loading by default
        lazy_cache_limit: 50_000, // Higher limit for CLI usage
    };
    storage.init_tact_manifests(manifest_config);

    let mut stats = serde_json::json!({
        "manifests_loaded": {},
        "errors": []
    });

    // Load root manifest
    if let Some(root_path) = root_manifest {
        match storage.load_root_manifest_from_file(&root_path) {
            Ok(_) => {
                info!("Successfully loaded root manifest from {:?}", root_path);
                stats["manifests_loaded"]["root"] = serde_json::json!({
                    "path": root_path,
                    "status": "success"
                });
            }
            Err(e) => {
                error!("Failed to load root manifest: {}", e);
                stats["errors"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "manifest": "root",
                        "path": root_path,
                        "error": e.to_string()
                    }));
            }
        }
    }

    // Load encoding manifest
    if let Some(encoding_path) = encoding_manifest {
        match storage.load_encoding_manifest_from_file(&encoding_path) {
            Ok(_) => {
                info!(
                    "Successfully loaded encoding manifest from {:?}",
                    encoding_path
                );
                stats["manifests_loaded"]["encoding"] = serde_json::json!({
                    "path": encoding_path,
                    "status": "success"
                });
            }
            Err(e) => {
                error!("Failed to load encoding manifest: {}", e);
                stats["errors"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "manifest": "encoding",
                        "path": encoding_path,
                        "error": e.to_string()
                    }));
            }
        }
    }

    // Load listfile
    if let Some(listfile_path) = listfile {
        match storage.load_listfile(&listfile_path) {
            Ok(count) => {
                info!(
                    "Successfully loaded {} filename mappings from listfile",
                    count
                );
                stats["manifests_loaded"]["listfile"] = serde_json::json!({
                    "path": listfile_path,
                    "status": "success",
                    "entries": count
                });
            }
            Err(e) => {
                error!("Failed to load listfile: {}", e);
                stats["errors"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "manifest": "listfile",
                        "path": listfile_path,
                        "error": e.to_string()
                    }));
            }
        }
    }

    // Get additional stats if manifests loaded
    if storage.tact_manifests_loaded() {
        if let Ok(fdids) = storage.get_all_fdids() {
            stats["file_count"] = fdids.len().into();
        }
    }

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("{}", serde_json::to_string(&stats)?);
            }
        }
        OutputFormat::Text => {
            println!("📋 TACT Manifest Loading Results");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

            if storage.tact_manifests_loaded() {
                println!("✅ TACT manifests loaded successfully");

                if let Ok(fdids) = storage.get_all_fdids() {
                    println!(
                        "   FileDataIDs available: {}",
                        fdids.len().to_string().green()
                    );
                }

                println!("   Locale filter: {}", locale.yellow());

                if info_only {
                    println!("   ℹ️  Info-only mode (not persisted)");
                }
            } else {
                println!("❌ TACT manifests not fully loaded");
            }

            if !stats["errors"].as_array().unwrap().is_empty() {
                println!("\n⚠️  Errors:");
                for error in stats["errors"].as_array().unwrap() {
                    println!(
                        "   • {}: {}",
                        error["manifest"].as_str().unwrap(),
                        error["error"].as_str().unwrap()
                    );
                }
            }
        }
        OutputFormat::Bpsv => {
            println!("## TACT Manifests");
            println!("loaded = {}", storage.tact_manifests_loaded());
            if let Ok(fdids) = storage.get_all_fdids() {
                println!("file_count = {}", fdids.len());
            }
            println!("locale = {locale}");
            println!("errors = {}", stats["errors"].as_array().unwrap().len());
        }
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.2} {}", size, UNITS[unit])
    }
}
