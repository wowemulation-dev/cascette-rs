use crate::{
    InspectCommands, OutputFormat,
    cached_client::create_client,
    output::{
        OutputStyle, create_table, format_count_badge, format_header, format_key_value,
        format_success, format_warning, header_cell, numeric_cell, print_section_header,
        print_subsection_header, regular_cell,
    },
};
use blte::decompress_blte;
use ngdp_bpsv::BpsvDocument;
use ngdp_cdn::CdnClient;
use ngdp_crypto::KeyService;
use ribbit_client::{Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region};
use std::str::FromStr;
use tact_parser::{
    config::BuildConfig, download::DownloadManifest, encoding::EncodingFile,
    install::InstallManifest, size::SizeFile,
};

pub async fn handle(
    cmd: InspectCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InspectCommands::Bpsv { input, raw } => inspect_bpsv(input, raw, format).await?,
        InspectCommands::BuildConfig {
            product,
            build,
            region,
        } => {
            inspect_build_config(product, build, region, format).await?;
        }
        InspectCommands::CdnConfig { product, region } => {
            println!("CDN config inspection not yet implemented");
            println!("Product: {product}");
            println!("Region: {region}");
        }
        InspectCommands::Encoding {
            product,
            region,
            stats,
            search,
            limit,
        } => {
            inspect_encoding(product, region, stats, search, limit, format).await?;
        }
        InspectCommands::Install {
            product,
            region,
            tags,
            all,
        } => {
            inspect_install(product, region, tags, all, format).await?;
        }
        InspectCommands::DownloadManifest {
            product,
            region,
            priority_limit,
            tags,
        } => {
            inspect_download_manifest(product, region, priority_limit, tags, format).await?;
        }
        InspectCommands::Size {
            product,
            region,
            largest,
            tags,
        } => {
            inspect_size(product, region, largest, tags, format).await?;
        }
    }
    Ok(())
}

async fn inspect_bpsv(
    input: String,
    raw: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read BPSV data from file or fetch from URL
    let data = if input.starts_with("http://") || input.starts_with("https://") {
        // Fetch from URL
        let response = reqwest::get(&input).await?;
        response.text().await?
    } else {
        // Read from file
        std::fs::read_to_string(&input)?
    };

    if raw {
        println!("{data}");
        return Ok(());
    }

    let doc = BpsvDocument::parse(&data)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = serde_json::json!({
                "schema": doc.schema().field_names(),
                "sequence_number": doc.sequence_number(),
                "row_count": doc.rows().len(),
                "rows": doc.rows().iter().map(|row| {
                    let mut map = serde_json::Map::new();
                    for (name, value) in doc.schema().field_names().iter().zip(row.raw_values()) {
                        map.insert(name.to_string(), serde_json::Value::String(value.to_string()));
                    }
                    map
                }).collect::<Vec<_>>()
            });

            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&json_data)?
            } else {
                serde_json::to_string(&json_data)?
            };
            println!("{output}");
        }
        OutputFormat::Bpsv => {
            println!("{}", doc.to_bpsv_string());
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            print_section_header("BPSV Document Analysis", &style);

            print_subsection_header("Schema", &style);
            let mut schema_table = create_table(&style);
            schema_table.set_header(vec![
                header_cell("Index", &style),
                header_cell("Field Name", &style),
                header_cell("Type", &style),
            ]);

            for (i, field) in doc.schema().fields().iter().enumerate() {
                schema_table.add_row(vec![
                    numeric_cell(&i.to_string()),
                    regular_cell(&field.name),
                    regular_cell(&field.field_type.to_string()),
                ]);
            }
            println!("{schema_table}");

            if let Some(seq) = doc.sequence_number() {
                println!();
                println!(
                    "{}",
                    format_key_value("Sequence Number", &seq.to_string(), &style)
                );
            }

            print_subsection_header(
                &format!(
                    "Data {}",
                    format_count_badge(doc.rows().len(), "row", &style)
                ),
                &style,
            );

            if !doc.rows().is_empty() {
                // Show first few rows in a table
                let preview_count = std::cmp::min(5, doc.rows().len());
                println!(
                    "\n{}",
                    format_header(&format!("Preview (first {preview_count} rows)"), &style)
                );

                let mut data_table = create_table(&style);

                // Set headers from schema
                let mut headers = vec![header_cell("#", &style)];
                headers.extend(
                    doc.schema()
                        .field_names()
                        .iter()
                        .map(|name| header_cell(name, &style)),
                );
                data_table.set_header(headers);

                // Add rows
                for (i, row) in doc.rows().iter().take(preview_count).enumerate() {
                    let mut cells = vec![numeric_cell(&(i + 1).to_string())];
                    cells.extend(row.raw_values().iter().map(|v| regular_cell(v)));
                    data_table.add_row(cells);
                }

                println!("{data_table}");

                if doc.rows().len() > preview_count {
                    println!(
                        "\n{}",
                        format_header(
                            &format!("... and {} more rows", doc.rows().len() - preview_count),
                            &style
                        )
                    );
                }
            }
        }
    }

    Ok(())
}

/// Inspect a build configuration by downloading and parsing it
async fn inspect_build_config(
    product: String,
    build: String,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();
    let region_enum = Region::from_str(&region)?;

    // Step 1: Get product version information
    print_section_header(
        &format!("Build Config Analysis: {product} (Build {build})"),
        &style,
    );

    let client = create_client(region_enum).await?;
    let versions_endpoint = Endpoint::ProductVersions(product.clone());
    let versions: ProductVersionsResponse = client.request_typed(&versions_endpoint).await?;

    // Find the specific build
    let build_entry = versions
        .entries
        .iter()
        .filter(|e| e.region == region)
        .find(|e| e.build_id.to_string() == build || e.versions_name == build);

    let build_entry = match build_entry {
        Some(entry) => entry,
        None => {
            eprintln!(
                "{}",
                format_warning(
                    &format!("Build '{build}' not found for {product} in region {region}"),
                    &style
                )
            );
            return Ok(());
        }
    };

    println!("{}", format_key_value("Product", &product, &style));
    println!("{}", format_key_value("Region", &region, &style));
    println!(
        "{}",
        format_key_value("Build ID", &build_entry.build_id.to_string(), &style)
    );
    println!(
        "{}",
        format_key_value("Version", &build_entry.versions_name, &style)
    );
    println!(
        "{}",
        format_key_value("Build Config Hash", &build_entry.build_config, &style)
    );
    println!();

    // Step 2: Get CDN information
    let cdns_endpoint = Endpoint::ProductCdns(product.clone());
    let cdns: ProductCdnsResponse = client.request_typed(&cdns_endpoint).await?;

    let cdn_entry = cdns.entries.iter().find(|e| e.name == region);
    let cdn_entry = match cdn_entry {
        Some(entry) => entry,
        None => {
            eprintln!(
                "{}",
                format_warning(
                    &format!("No CDN configuration found for region {region}"),
                    &style
                )
            );
            return Ok(());
        }
    };

    // Step 3: Download the build config file
    print_subsection_header("Downloading Build Configuration", &style);

    let cdn_client = CdnClient::new()?;
    let cdn_host = &cdn_entry.hosts[0]; // Use first CDN host
    let cdn_path = &cdn_entry.path;

    println!(
        "Downloading from: {}/{}/config/{}",
        cdn_host, cdn_path, &build_entry.build_config
    );

    let response = cdn_client
        .download_build_config(cdn_host, cdn_path, &build_entry.build_config)
        .await?;
    let config_text = response.text().await?;

    // Step 4: Parse the build config
    let build_config = BuildConfig::parse(&config_text)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            output_build_config_json(&build_config, format)?;
        }
        OutputFormat::Text => {
            output_build_config_tree(&build_config, &style);
        }
        OutputFormat::Bpsv => {
            println!("{config_text}");
        }
    }

    Ok(())
}

/// Output build config as a visual tree
fn output_build_config_tree(config: &BuildConfig, style: &OutputStyle) {
    print_subsection_header("Build Configuration Tree", style);

    // Core Files Section
    println!("📁 {}", format_header("Core Game Files", style));

    if let Some(root_hash) = config.root_hash() {
        println!("├── 🗂️  Root File");
        println!("│   ├── Hash: {root_hash}");
        if let Some(size) = config.config.get_size("root") {
            println!(
                "│   └── Size: {} bytes ({:.2} MB)",
                size,
                size as f64 / (1024.0 * 1024.0)
            );
        }
    }

    if let Some(encoding_hash) = config.encoding_hash() {
        println!("├── 🔗 Encoding File (CKey ↔ EKey mapping)");
        println!("│   ├── Hash: {encoding_hash}");
        if let Some(size) = config.config.get_size("encoding") {
            println!(
                "│   └── Size: {} bytes ({:.2} KB)",
                size,
                size as f64 / 1024.0
            );
        }
    }

    if let Some(install_hash) = config.install_hash() {
        println!("├── 📦 Install Manifest");
        println!("│   ├── Hash: {install_hash}");
        if let Some(size) = config.config.get_size("install") {
            println!(
                "│   └── Size: {} bytes ({:.2} KB)",
                size,
                size as f64 / 1024.0
            );
        }
    }

    if let Some(download_hash) = config.download_hash() {
        println!("├── ⬇️  Download Manifest");
        println!("│   ├── Hash: {download_hash}");
        if let Some(size) = config.config.get_size("download") {
            println!(
                "│   └── Size: {} bytes ({:.2} KB)",
                size,
                size as f64 / 1024.0
            );
        }
    }

    if let Some(size_hash) = config.size_hash() {
        println!("└── 📏 Size File");
        println!("    ├── Hash: {size_hash}");
        if let Some(size) = config.config.get_size("size") {
            println!(
                "    └── Size: {} bytes ({:.2} KB)",
                size,
                size as f64 / 1024.0
            );
        }
    }

    println!();

    // Build Information Section
    println!("📋 {}", format_header("Build Information", style));

    if let Some(build_name) = config.build_name() {
        println!("├── Version: {}", format_success(build_name, style));
    }
    if let Some(build_uid) = config.config.get_value("build-uid") {
        println!("├── Build UID: {build_uid}");
    }
    if let Some(build_product) = config.config.get_value("build-product") {
        println!("├── Product: {build_product}");
    }
    if let Some(installer) = config.config.get_value("build-playbuild-installer") {
        println!("└── Installer: {installer}");
    }

    println!();

    // Patching Section
    println!("🔄 {}", format_header("Patching", style));

    let has_patch = config
        .config
        .get_value("patch")
        .is_some_and(|v| !v.is_empty());
    if has_patch {
        if let Some(patch_hash) = config.config.get_value("patch") {
            println!("├── ✅ Patch Available");
            println!("│   └── Hash: {patch_hash}");
        }
    } else {
        println!("└── ❌ No patch data");
    }

    println!();

    // VFS Section
    println!("🗃️  {}", format_header("Virtual File System (VFS)", style));

    let mut vfs_entries = Vec::new();
    for key in config.config.keys() {
        if key.starts_with("vfs-") {
            if let Some(value) = config.config.get_value(key) {
                vfs_entries.push((key, value));
            }
        }
    }

    if !vfs_entries.is_empty() {
        vfs_entries.sort_by_key(|(k, _)| *k);
        for (i, (key, value)) in vfs_entries.iter().enumerate() {
            let is_last = i == vfs_entries.len() - 1;
            let prefix = if is_last { "└──" } else { "├──" };

            if value.is_empty() {
                println!("{} {}: {}", prefix, key, format_warning("(empty)", style));
            } else {
                println!("{prefix} {key}: {value}");
            }
        }
    } else {
        println!("└── No VFS entries found");
    }

    println!();

    // Raw Configuration Section
    print_subsection_header("Raw Configuration Entries", style);

    let mut table = create_table(style);
    table.set_header(vec![
        header_cell("Key", style),
        header_cell("Value", style),
        header_cell("Type", style),
    ]);

    let mut keys: Vec<_> = config.config.keys().into_iter().collect();
    keys.sort();

    for key in keys {
        if let Some(value) = config.config.get_value(key) {
            let value_type = if config.config.get_hash(key).is_some() {
                "Hash + Size"
            } else if value.is_empty() {
                "Empty"
            } else if value.chars().all(|c| c.is_ascii_digit()) {
                "Number"
            } else {
                "String"
            };

            let display_value = if value.len() > 50 {
                format!("{}...", &value[..47])
            } else {
                value.to_string()
            };

            table.add_row(vec![
                regular_cell(key),
                regular_cell(&display_value),
                regular_cell(value_type),
            ]);
        }
    }

    println!("{table}");
}

/// Output build config as JSON
fn output_build_config_json(
    config: &BuildConfig,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_data = serde_json::Map::new();

    // Core hashes
    if let Some(hash) = config.root_hash() {
        json_data.insert(
            "root_hash".to_string(),
            serde_json::Value::String(hash.to_string()),
        );
    }
    if let Some(hash) = config.encoding_hash() {
        json_data.insert(
            "encoding_hash".to_string(),
            serde_json::Value::String(hash.to_string()),
        );
    }
    if let Some(hash) = config.install_hash() {
        json_data.insert(
            "install_hash".to_string(),
            serde_json::Value::String(hash.to_string()),
        );
    }
    if let Some(hash) = config.download_hash() {
        json_data.insert(
            "download_hash".to_string(),
            serde_json::Value::String(hash.to_string()),
        );
    }
    if let Some(hash) = config.size_hash() {
        json_data.insert(
            "size_hash".to_string(),
            serde_json::Value::String(hash.to_string()),
        );
    }

    // Build info
    if let Some(name) = config.build_name() {
        json_data.insert(
            "build_name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }

    // All raw values
    let mut raw_config = serde_json::Map::new();
    for key in config.config.keys() {
        if let Some(value) = config.config.get_value(key) {
            raw_config.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    json_data.insert(
        "raw_config".to_string(),
        serde_json::Value::Object(raw_config),
    );

    // Hash pairs
    let mut hash_pairs = serde_json::Map::new();
    for key in config.config.keys() {
        if let Some(hash_pair) = config.config.get_hash_pair(key) {
            hash_pairs.insert(
                key.to_string(),
                serde_json::json!({
                    "hash": hash_pair.hash,
                    "size": hash_pair.size
                }),
            );
        }
    }
    if !hash_pairs.is_empty() {
        json_data.insert(
            "hash_pairs".to_string(),
            serde_json::Value::Object(hash_pairs),
        );
    }

    let output = match format {
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&json_data)?,
        _ => serde_json::to_string(&json_data)?,
    };

    println!("{output}");
    Ok(())
}

/// Helper to download and decompress a manifest file from CDN
async fn download_and_decompress_manifest(
    product: &str,
    region: &str,
    manifest_type: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Step 1: Get build config hash
    let region = Region::from_str(region)?;
    let client = create_client(region).await?;

    let versions_endpoint = Endpoint::ProductVersions(product.to_string());
    let versions: ProductVersionsResponse = client
        .request_typed(&versions_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let entry = versions
        .entries
        .iter()
        .find(|e| e.region == region.to_string())
        .ok_or("Region not found")?;

    // Step 2: Get CDN info
    let cdns_endpoint = Endpoint::ProductCdns(product.to_string());
    let cdns: ProductCdnsResponse = client
        .request_typed(&cdns_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let cdn_entry = cdns
        .entries
        .iter()
        .find(|e| e.name == region.to_string())
        .ok_or("CDN not found")?;

    // Step 3: Download build config
    let cdn_client = CdnClient::new()?;
    let cdn_host = &cdn_entry.hosts[0];
    let cdn_path = &cdn_entry.path;

    let config_response = cdn_client
        .download_build_config(cdn_host, cdn_path, &entry.build_config)
        .await?;
    let config_text = config_response.text().await?;
    let build_config = BuildConfig::parse(&config_text)?;

    // Step 4: Get the manifest hash based on type
    // Build configs have two hashes: content key and encoding key (CDN key)
    // We need the second hash (encoding key) for CDN downloads
    let manifest_hash = match manifest_type {
        "encoding" => {
            // Get the raw value and extract the second hash if present
            build_config
                .config
                .get_value("encoding")
                .and_then(|v| v.split_whitespace().nth(1))
                .ok_or("No encoding hash")?
        }
        "install" => build_config
            .config
            .get_value("install")
            .and_then(|v| v.split_whitespace().nth(1))
            .or_else(|| build_config.install_hash())
            .ok_or("No install hash")?,
        "download" => build_config
            .config
            .get_value("download")
            .and_then(|v| v.split_whitespace().nth(1))
            .or_else(|| build_config.download_hash())
            .ok_or("No download hash")?,
        "size" => build_config
            .config
            .get_value("size")
            .and_then(|v| v.split_whitespace().nth(1))
            .or_else(|| build_config.size_hash())
            .ok_or("No size hash")?,
        _ => return Err("Invalid manifest type".into()),
    };

    // Step 5: Download the manifest as data file
    // Encoding, install, download, and size files are stored as data, not config
    let response = cdn_client
        .download_data(cdn_host, cdn_path, manifest_hash)
        .await?;

    let manifest_data = response.bytes().await?.to_vec();

    // Step 6: Decompress with BLTE if needed
    if manifest_data.len() >= 4 && &manifest_data[0..4] == b"BLTE" {
        let key_service = KeyService::new();
        let decompressed = decompress_blte(manifest_data, Some(&key_service))?;
        Ok(decompressed)
    } else {
        Ok(manifest_data)
    }
}

/// Inspect encoding file
async fn inspect_encoding(
    product: String,
    region: String,
    stats: bool,
    search: Option<String>,
    _limit: usize,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();
    print_section_header(&format!("Encoding File Inspector - {product}"), &style);

    // Download and decompress encoding file
    let encoding_data = download_and_decompress_manifest(&product, &region, "encoding").await?;

    // Parse encoding file
    let encoding_file = EncodingFile::parse(&encoding_data)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = serde_json::json!({
                "version": encoding_file.header.version,
                "ckey_count": encoding_file.ckey_count(),
                "ekey_count": encoding_file.ekey_count(),
                "stats": if stats {
                    Some(serde_json::json!({
                        "total_ckeys": encoding_file.ckey_count(),
                        "total_ekeys": encoding_file.ekey_count(),
                    }))
                } else {
                    None
                },
            });

            let output = match format {
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&json_data)?,
                _ => serde_json::to_string(&json_data)?,
            };
            println!("{output}");
        }
        OutputFormat::Text => {
            print_subsection_header("Encoding File Summary", &style);
            println!("Version: {}", encoding_file.header.version);
            println!(
                "CKey entries: {}",
                format_count_badge(encoding_file.ckey_count(), "entry", &style)
            );
            println!(
                "EKey mappings: {}",
                format_count_badge(encoding_file.ekey_count(), "mapping", &style)
            );

            if let Some(search_key) = search {
                print_subsection_header("Search Results", &style);
                let search_bytes = hex::decode(&search_key)?;

                if let Some(entry) = encoding_file.lookup_by_ckey(&search_bytes) {
                    println!("Found CKey: {search_key}");
                    println!("  File size: {} bytes", entry.size);
                    if !entry.encoding_keys.is_empty() {
                        println!("  EKeys:");
                        for ekey in &entry.encoding_keys {
                            println!("    - {}", hex::encode(ekey));
                        }
                    }
                } else if let Some(ckey) = encoding_file.lookup_by_ekey(&search_bytes) {
                    println!("Found EKey: {search_key}");
                    println!("  Maps to CKey: {}", hex::encode(ckey));
                } else {
                    println!("Key not found: {search_key}");
                }
            }

            if stats {
                print_subsection_header("Statistics", &style);
                println!("Total unique content keys: {}", encoding_file.ckey_count());
                println!(
                    "Total encoding key mappings: {}",
                    encoding_file.ekey_count()
                );
            }
        }
        _ => {
            println!("Format not supported for encoding inspection");
        }
    }

    Ok(())
}

/// Inspect install manifest
async fn inspect_install(
    product: String,
    region: String,
    tags: Option<String>,
    all: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();
    print_section_header(&format!("Install Manifest Inspector - {product}"), &style);

    // Download and decompress install manifest
    let install_data = download_and_decompress_manifest(&product, &region, "install").await?;

    // Parse install manifest
    let install_manifest = InstallManifest::parse(&install_data)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = serde_json::json!({
                "version": install_manifest.header.version,
                "entry_count": install_manifest.entries.len(),
                "tag_count": install_manifest.tags.len(),
                "tags": install_manifest.tags.iter().map(|t| &t.name).collect::<Vec<_>>(),
            });

            let output = match format {
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&json_data)?,
                _ => serde_json::to_string(&json_data)?,
            };
            println!("{output}");
        }
        OutputFormat::Text => {
            print_subsection_header("Install Manifest Summary", &style);
            println!("Version: {}", install_manifest.header.version);
            println!(
                "Total files: {}",
                format_count_badge(install_manifest.entries.len(), "file", &style)
            );
            println!(
                "Total tags: {}",
                format_count_badge(install_manifest.tags.len(), "tag", &style)
            );

            if !install_manifest.tags.is_empty() {
                print_subsection_header("Available Tags", &style);
                for tag in &install_manifest.tags {
                    println!("  - {} (type: {})", tag.name, tag.tag_type);
                }
            }

            if let Some(tag_filter) = tags {
                let filter_tags: Vec<&str> = tag_filter.split(',').collect();
                let filtered_files = install_manifest.get_files_for_tags(&filter_tags);

                print_subsection_header(&format!("Files for tags: {tag_filter}"), &style);
                println!(
                    "Found {} files",
                    format_count_badge(filtered_files.len(), "file", &style)
                );

                if all || filtered_files.len() <= 20 {
                    for (i, file) in filtered_files.iter().enumerate() {
                        if i >= 20 && !all {
                            println!("... and {} more", filtered_files.len() - i);
                            break;
                        }
                        println!("  {}", file.path);
                    }
                }
            }
        }
        _ => {
            println!("Format not supported for install manifest inspection");
        }
    }

    Ok(())
}

/// Inspect download manifest
async fn inspect_download_manifest(
    product: String,
    region: String,
    priority_limit: usize,
    tags: Option<String>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();
    print_section_header(&format!("Download Manifest Inspector - {product}"), &style);

    // Download and decompress download manifest
    let download_data = download_and_decompress_manifest(&product, &region, "download").await?;

    // Parse download manifest
    let download_manifest = DownloadManifest::parse(&download_data)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let priority_files =
                download_manifest.get_priority_files(priority_limit.min(127) as i8);
            let json_data = serde_json::json!({
                "version": download_manifest.header.version,
                "entry_count": download_manifest.entries.len(),
                "tag_count": download_manifest.tags.len(),
                "priority_files": priority_files.iter().map(|entry| {
                    serde_json::json!({
                        "ekey": hex::encode(&entry.ekey),
                        "priority": entry.priority,
                    })
                }).collect::<Vec<_>>(),
            });

            let output = match format {
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&json_data)?,
                _ => serde_json::to_string(&json_data)?,
            };
            println!("{output}");
        }
        OutputFormat::Text => {
            print_subsection_header("Download Manifest Summary", &style);
            println!("Version: {}", download_manifest.header.version);
            println!(
                "Total entries: {}",
                format_count_badge(download_manifest.entries.len(), "entry", &style)
            );
            println!(
                "Total tags: {}",
                format_count_badge(download_manifest.tags.len(), "tag", &style)
            );

            print_subsection_header(&format!("Top {priority_limit} Priority Files"), &style);
            let priority_files =
                download_manifest.get_priority_files(priority_limit.min(127) as i8);
            for (i, entry) in priority_files.iter().enumerate() {
                println!(
                    "  {}. Priority {}: {}",
                    i + 1,
                    entry.priority,
                    hex::encode(&entry.ekey)
                );
            }

            if let Some(tag_filter) = tags {
                let filter_tags: Vec<&str> = tag_filter.split(',').collect();
                let filtered_files = download_manifest.get_files_for_tags(&filter_tags);

                print_subsection_header(&format!("Files for tags: {tag_filter}"), &style);
                println!(
                    "Found {} files",
                    format_count_badge(filtered_files.len(), "file", &style)
                );
            }
        }
        _ => {
            println!("Format not supported for download manifest inspection");
        }
    }

    Ok(())
}

/// Inspect size file
async fn inspect_size(
    product: String,
    region: String,
    largest: usize,
    tags: Option<String>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();
    print_section_header(&format!("Size File Inspector - {product}"), &style);

    // Download and decompress size file
    let size_data = download_and_decompress_manifest(&product, &region, "size").await?;

    // Parse size file
    let size_file = SizeFile::parse(&size_data)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let largest_files = size_file.get_largest_files(largest);
            let stats = size_file.get_statistics();
            let json_data = serde_json::json!({
                "version": size_file.header.version,
                "entry_count": size_file.entries.len(),
                "tag_count": size_file.tags.len(),
                "total_size": size_file.get_total_size(),
                "statistics": {
                    "average_size": stats.average_size,
                    "min_size": stats.min_size,
                    "max_size": stats.max_size,
                },
                "largest_files": largest_files.iter().map(|(ekey, size)| {
                    serde_json::json!({
                        "ekey": hex::encode(ekey),
                        "size": size,
                    })
                }).collect::<Vec<_>>(),
            });

            let output = match format {
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&json_data)?,
                _ => serde_json::to_string(&json_data)?,
            };
            println!("{output}");
        }
        OutputFormat::Text => {
            print_subsection_header("Size File Summary", &style);
            println!("Version: {}", size_file.header.version);
            println!(
                "Total entries: {}",
                format_count_badge(size_file.entries.len(), "entry", &style)
            );
            println!(
                "Total tags: {}",
                format_count_badge(size_file.tags.len(), "tag", &style)
            );

            let total_size = size_file.get_total_size();
            println!(
                "Total installation size: {} GB",
                total_size / (1024 * 1024 * 1024)
            );

            let stats = size_file.get_statistics();
            print_subsection_header("File Size Statistics", &style);
            println!(
                "Average file size: {} MB",
                stats.average_size / (1024 * 1024)
            );
            println!("Minimum file size: {} bytes", stats.min_size);
            println!("Maximum file size: {} MB", stats.max_size / (1024 * 1024));

            print_subsection_header(&format!("Top {largest} Largest Files"), &style);
            let largest_files = size_file.get_largest_files(largest);
            for (i, (ekey, size)) in largest_files.iter().enumerate() {
                let size_mb = size / (1024 * 1024);
                println!("  {}. {} MB - {}", i + 1, size_mb, hex::encode(&ekey[0..8]));
            }

            if let Some(tag_filter) = tags {
                let filter_tags: Vec<&str> = tag_filter.split(',').collect();
                let tag_size = size_file.get_size_for_tags(&filter_tags);

                print_subsection_header(&format!("Size for tags: {tag_filter}"), &style);
                println!("Total size: {} GB", tag_size / (1024 * 1024 * 1024));
            }
        }
        _ => {
            println!("Format not supported for size file inspection");
        }
    }

    Ok(())
}
