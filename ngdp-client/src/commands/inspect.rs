use crate::{
    InspectCommands, OutputFormat,
    cached_client::create_client,
    output::{
        OutputStyle, create_table, format_count_badge, format_header, format_key_value,
        format_success, format_warning, header_cell, numeric_cell, print_section_header,
        print_subsection_header, regular_cell,
    },
};
use ngdp_bpsv::BpsvDocument;
use ngdp_cdn::CdnClient;
use ribbit_client::{Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region};
use std::str::FromStr;
use tact_parser::config::BuildConfig;

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
        InspectCommands::Encoding { file, stats } => {
            println!("Encoding inspection not yet implemented");
            println!("File: {file:?}");
            println!("Stats: {stats}");
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
