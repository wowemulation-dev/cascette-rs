use crate::{
    OutputFormat, ProductsCommands,
    cached_client::create_client,
    output::{
        OutputStyle, create_list_table, create_table, format_count_badge, format_hash,
        format_header, format_key_value, format_success, format_url, format_warning, hash_cell,
        header_cell, numeric_cell, print_section_header, print_subsection_header, regular_cell,
    },
};
use ribbit_client::{
    Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region, SummaryResponse,
};
use std::str::FromStr;

pub async fn handle(
    cmd: ProductsCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProductsCommands::List { filter, region } => list_products(filter, region, format).await,
        ProductsCommands::Versions {
            product,
            region,
            all_regions,
        } => show_versions(product, region, all_regions, format).await,
        ProductsCommands::Cdns { product, region } => show_cdns(product, region, format).await,
        ProductsCommands::Info { product, region } => show_info(product, region, format).await,
    }
}

async fn list_products(
    filter: Option<String>,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region = Region::from_str(&region)?;
    let client = create_client(region).await?;

    let response = client.request(&Endpoint::Summary).await?;
    let summary: SummaryResponse = client.request_typed(&Endpoint::Summary).await?;

    let mut products = summary.products.clone();

    // Apply filter if provided
    if let Some(filter) = &filter {
        products.retain(|p| p.product.contains(filter));
    }

    // Sort by product name first, then by sequence number
    products.sort_by(|a, b| a.product.cmp(&b.product).then_with(|| a.seqn.cmp(&b.seqn)));

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data: Vec<_> = products
                .iter()
                .map(|entry| {
                    let mut obj = serde_json::json!({
                        "code": entry.product,
                        "seqn": entry.seqn,
                    });
                    if let Some(flags) = &entry.flags {
                        obj["flags"] = serde_json::json!(flags);
                    }
                    obj
                })
                .collect();

            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&json_data)?
            } else {
                serde_json::to_string(&json_data)?
            };
            println!("{}", output);
        }
        OutputFormat::Bpsv => {
            if let Some(data) = response.as_text() {
                println!("{}", data);
            }
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            // Print header with count
            println!(
                "{} {}",
                format_header("Available products", &style),
                format_count_badge(products.len(), "product", &style)
            );

            // Create table
            let mut table = create_table(&style);
            table.set_header(vec![
                header_cell("Product", &style),
                header_cell("Sequence", &style),
                header_cell("Flags", &style),
            ]);

            // Add rows
            for entry in &products {
                table.add_row(vec![
                    regular_cell(&entry.product),
                    numeric_cell(&entry.seqn.to_string()),
                    regular_cell(entry.flags.as_deref().unwrap_or("")),
                ]);
            }

            println!("{}", table);

            // Add a note about flags if any products have flags
            if products.iter().any(|p| p.flags.is_some()) {
                println!();
                println!("{}", format_header("Flag meanings:", &style));
                println!(
                    "  {} - Product has CDN configuration",
                    format_success("cdn", &style)
                );
                println!(
                    "  {} - Product has background download configuration",
                    format_success("bgdl", &style)
                );
            }
        }
    }

    Ok(())
}

async fn show_versions(
    product: String,
    region: String,
    all_regions: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region = Region::from_str(&region)?;
    let client = create_client(region).await?;

    let endpoint = Endpoint::ProductVersions(product.clone());
    let response = client.request(&endpoint).await?;
    let versions: ProductVersionsResponse = client.request_typed(&endpoint).await?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = if all_regions {
                let entries: Vec<_> = versions
                    .entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "region": e.region,
                            "versions_name": e.versions_name,
                            "build_id": e.build_id,
                            "build_config": e.build_config,
                            "cdn_config": e.cdn_config,
                            "product_config": e.product_config,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "sequence_number": versions.sequence_number,
                    "entries": entries
                })
            } else {
                // Filter to just the requested region
                let filtered: Vec<_> = versions
                    .entries
                    .iter()
                    .filter(|e| e.region == region.as_str())
                    .map(|e| {
                        serde_json::json!({
                            "region": e.region,
                            "versions_name": e.versions_name,
                            "build_id": e.build_id,
                            "build_config": e.build_config,
                            "cdn_config": e.cdn_config,
                            "product_config": e.product_config,
                        })
                    })
                    .collect();
                serde_json::Value::Array(filtered)
            };

            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&json_data)?
            } else {
                serde_json::to_string(&json_data)?
            };
            println!("{}", output);
        }
        OutputFormat::Bpsv => {
            if let Some(data) = response.as_text() {
                println!("{}", data);
            }
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            print_section_header(&format!("Product: {}", product), &style);
            if let Some(seqn) = versions.sequence_number {
                println!(
                    "{}",
                    format_key_value("Sequence", &seqn.to_string(), &style)
                );
            }

            if all_regions {
                println!();

                // Create table for all regions with multi-row format
                let mut table = create_table(&style);
                table.set_header(vec![
                    header_cell("Region", &style),
                    header_cell("Version", &style),
                    header_cell("Build", &style),
                    header_cell("Configuration Hash", &style),
                ]);

                for entry in &versions.entries {
                    // First row with build config
                    table.add_row(vec![
                        regular_cell(&entry.region),
                        regular_cell(&entry.versions_name),
                        numeric_cell(&entry.build_id.to_string()),
                        hash_cell(&format!("Build Config:   {}", &entry.build_config), &style),
                    ]);

                    // Second row with CDN config
                    table.add_row(vec![
                        regular_cell(""),
                        regular_cell(""),
                        regular_cell(""),
                        hash_cell(&format!("CDN Config:     {}", &entry.cdn_config), &style),
                    ]);

                    // Third row with Product config
                    table.add_row(vec![
                        regular_cell(""),
                        regular_cell(""),
                        regular_cell(""),
                        hash_cell(
                            &format!("Product Config: {}", &entry.product_config),
                            &style,
                        ),
                    ]);

                    // Fourth row with Key Ring (if present)
                    if let Some(key_ring) = &entry.key_ring {
                        table.add_row(vec![
                            regular_cell(""),
                            regular_cell(""),
                            regular_cell(""),
                            hash_cell(&format!("Key Ring:       {}", key_ring), &style),
                        ]);
                    }
                }

                println!("{}", table);
            } else if let Some(entry) = versions.get_region(region.as_str()) {
                println!();

                // Create a detailed table for single region
                let mut table = create_table(&style);
                table.set_header(vec![
                    header_cell("Property", &style),
                    header_cell("Value", &style),
                ]);

                table.add_row(vec![regular_cell("Region"), regular_cell(&entry.region)]);

                table.add_row(vec![
                    regular_cell("Version"),
                    regular_cell(&entry.versions_name),
                ]);

                table.add_row(vec![
                    regular_cell("Build ID"),
                    regular_cell(&entry.build_id.to_string()),
                ]);

                table.add_row(vec![
                    regular_cell("Build Config"),
                    hash_cell(&entry.build_config, &style),
                ]);

                table.add_row(vec![
                    regular_cell("CDN Config"),
                    hash_cell(&entry.cdn_config, &style),
                ]);

                table.add_row(vec![
                    regular_cell("Product Config"),
                    hash_cell(&entry.product_config, &style),
                ]);

                if let Some(key_ring) = &entry.key_ring {
                    table.add_row(vec![regular_cell("Key Ring"), hash_cell(key_ring, &style)]);
                }

                println!("{}", table);
            } else {
                println!();
                println!(
                    "{}",
                    format_warning(
                        &format!(
                            "No version information available for region '{}'",
                            region.as_str()
                        ),
                        &style
                    )
                );
            }
        }
    }

    Ok(())
}

async fn show_cdns(
    product: String,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region = Region::from_str(&region)?;
    let client = create_client(region).await?;

    let endpoint = Endpoint::ProductCdns(product.clone());
    let response = client.request(&endpoint).await?;
    let cdns: ProductCdnsResponse = client.request_typed(&endpoint).await?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = cdns
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "name": e.name,
                        "path": e.path,
                        "config_path": e.config_path,
                        "hosts": e.hosts,
                    })
                })
                .collect::<Vec<_>>();

            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&json_data)?
            } else {
                serde_json::to_string(&json_data)?
            };
            println!("{}", output);
        }
        OutputFormat::Bpsv => {
            if let Some(data) = response.as_text() {
                println!("{}", data);
            }
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            print_section_header(&format!("CDN Configuration for {}", product), &style);
            if let Some(seqn) = cdns.sequence_number {
                println!(
                    "{}",
                    format_key_value("Sequence", &seqn.to_string(), &style)
                );
            }

            println!();

            // Create a table for each region
            for entry in &cdns.entries {
                print_subsection_header(&format!("Region: {}", &entry.name), &style);

                let mut table = create_table(&style);
                table.set_header(vec![
                    header_cell("Property", &style),
                    header_cell("Value", &style),
                ]);

                table.add_row(vec![regular_cell("Path"), regular_cell(&entry.path)]);

                table.add_row(vec![
                    regular_cell("Config Path"),
                    regular_cell(&entry.config_path),
                ]);

                // Add hosts to the table (before servers)
                if !entry.hosts.is_empty() {
                    // First host
                    table.add_row(vec![
                        regular_cell("CDN Hosts"),
                        regular_cell(&entry.hosts[0]),
                    ]);

                    // Additional hosts on separate lines
                    for host in &entry.hosts[1..] {
                        table.add_row(vec![regular_cell(""), regular_cell(host)]);
                    }
                }

                if !entry.servers.is_empty() {
                    // First server
                    table.add_row(vec![
                        regular_cell("Servers"),
                        regular_cell(&entry.servers[0]),
                    ]);

                    // Additional servers on separate lines
                    for server in &entry.servers[1..] {
                        table.add_row(vec![regular_cell(""), regular_cell(server)]);
                    }
                }

                println!("{}", table);
                println!();
            }
        }
    }

    Ok(())
}

async fn show_info(
    product: String,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region = Region::from_str(&region)?;
    let client = create_client(region).await?;

    // Get both versions and CDNs
    let versions_endpoint = Endpoint::ProductVersions(product.clone());
    let cdns_endpoint = Endpoint::ProductCdns(product.clone());

    // Fetch all data concurrently
    let versions_response = client
        .request(&versions_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let versions = client
        .request_typed::<ProductVersionsResponse>(&versions_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let cdns_response = client
        .request(&cdns_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let cdns = client
        .request_typed::<ProductCdnsResponse>(&cdns_endpoint)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let summary = client
        .request_typed::<SummaryResponse>(&Endpoint::Summary)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let versions_data = versions
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "region": e.region,
                        "versions_name": e.versions_name,
                        "build_id": e.build_id,
                    })
                })
                .collect::<Vec<_>>();

            let cdns_data = cdns
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "name": e.name,
                        "hosts": e.hosts,
                    })
                })
                .collect::<Vec<_>>();

            let info = serde_json::json!({
                "product": product,
                "summary": summary.get_product(&product).map(|s| serde_json::json!({
                    "product": s.product,
                    "seqn": s.seqn,
                    "flags": s.flags,
                })),
                "versions": versions_data,
                "cdns": cdns_data,
            });

            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&info)?
            } else {
                serde_json::to_string(&info)?
            };
            println!("{}", output);
        }
        OutputFormat::Bpsv => {
            println!("# Versions");
            if let Some(data) = versions_response.as_text() {
                println!("{}", data);
            }
            println!("\n# CDNs");
            if let Some(data) = cdns_response.as_text() {
                println!("{}", data);
            }
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            print_section_header(&format!("Product Information: {}", product), &style);

            if let Some(summary_entry) = summary.get_product(&product) {
                print_subsection_header("Summary", &style);
                println!(
                    "{}",
                    format_key_value("Sequence", &summary_entry.seqn.to_string(), &style)
                );
                if let Some(flags) = &summary_entry.flags {
                    println!("{}", format_key_value("Flags", flags, &style));
                }
            }

            if let Some(version) = versions.get_region(region.as_str()) {
                print_subsection_header(&format!("Current Version ({})", region.as_str()), &style);
                println!(
                    "{}",
                    format_key_value("Version", &version.versions_name, &style)
                );
                println!(
                    "{}",
                    format_key_value("Build ID", &version.build_id.to_string(), &style)
                );
                println!(
                    "{}",
                    format_key_value(
                        "Build Config",
                        &format_hash(&version.build_config, &style),
                        &style
                    )
                );
                println!(
                    "{}",
                    format_key_value(
                        "CDN Config",
                        &format_hash(&version.cdn_config, &style),
                        &style
                    )
                );
                println!(
                    "{}",
                    format_key_value(
                        "Product Config",
                        &format_hash(&version.product_config, &style),
                        &style
                    )
                );
            }

            // Available regions
            let mut regions: Vec<_> = versions.entries.iter().map(|e| &e.region).collect();
            regions.sort();
            regions.dedup();

            print_subsection_header("Available Regions", &style);
            let mut table = create_list_table(&style);
            table.set_header(vec![
                header_cell("Region", &style),
                header_cell("Version", &style),
            ]);

            for region_name in &regions {
                if let Some(entry) = versions.entries.iter().find(|e| &e.region == *region_name) {
                    table.add_row(vec![
                        regular_cell(&entry.region),
                        regular_cell(&entry.versions_name),
                    ]);
                }
            }
            println!("{}", table);

            // CDN hosts
            let hosts = cdns.all_hosts();
            print_subsection_header(
                &format!(
                    "CDN Hosts {}",
                    format_count_badge(hosts.len(), "host", &style)
                ),
                &style,
            );
            for host in hosts {
                println!(
                    "  {} {}",
                    format_success("•", &style),
                    format_url(&host, &style)
                );
            }
        }
    }

    Ok(())
}
