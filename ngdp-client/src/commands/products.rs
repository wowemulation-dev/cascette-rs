use crate::{OutputFormat, ProductsCommands};
use ribbit_client::{
    Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region, RibbitClient, SummaryResponse,
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
    let client = RibbitClient::new(region);

    let response = client.request(&Endpoint::Summary).await?;
    let summary: SummaryResponse = client.request_typed(&Endpoint::Summary).await?;

    let mut products = summary.products.clone();
    products.sort_by_key(|p| p.product.clone());

    // Apply filter if provided
    if let Some(filter) = &filter {
        products.retain(|p| p.product.contains(filter));
    }

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data: Vec<_> = products
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "code": entry.product,
                        "seqn": entry.seqn,
                    })
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
            println!("Available products ({})", products.len());
            println!("{:<20} Sequence", "Product");
            println!("{}", "-".repeat(30));

            for entry in &products {
                println!("{:<20} {}", entry.product, entry.seqn);
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
    let client = RibbitClient::new(region);

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
            println!("Product: {}", product);
            if let Some(seqn) = versions.sequence_number {
                println!("Sequence: {}", seqn);
            }
            println!();

            if all_regions {
                println!("{:<10} {:<20} {:<10}", "Region", "Version", "Build ID");
                println!("{}", "-".repeat(45));

                for entry in &versions.entries {
                    println!(
                        "{:<10} {:<20} {:<10}",
                        entry.region, entry.versions_name, entry.build_id
                    );
                }
            } else if let Some(entry) = versions.get_region(region.as_str()) {
                println!("Region:       {}", entry.region);
                println!("Version:      {}", entry.versions_name);
                println!("Build ID:     {}", entry.build_id);
                println!("Build Config: {}", entry.build_config);
                println!("CDN Config:   {}", entry.cdn_config);
                println!("Product Config: {}", entry.product_config);
            } else {
                println!(
                    "No version information available for region '{}'",
                    region.as_str()
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
    let client = RibbitClient::new(region);

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
            println!("CDN Configuration for {}", product);
            if let Some(seqn) = cdns.sequence_number {
                println!("Sequence: {}", seqn);
            }
            println!();

            for entry in &cdns.entries {
                println!("Name: {}", entry.name);
                println!("Path: {}", entry.path);
                println!("Config Path: {}", entry.config_path);
                println!("Hosts:");
                for host in &entry.hosts {
                    println!("  - {}", host);
                }
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
    let client = RibbitClient::new(region);

    // Get both versions and CDNs
    let versions_endpoint = Endpoint::ProductVersions(product.clone());
    let cdns_endpoint = Endpoint::ProductCdns(product.clone());

    let ((versions_response, versions), (cdns_response, cdns), summary) = tokio::try_join!(
        async {
            let response = client.request(&versions_endpoint).await?;
            let typed = client
                .request_typed::<ProductVersionsResponse>(&versions_endpoint)
                .await?;
            Ok::<_, ribbit_client::Error>((response, typed))
        },
        async {
            let response = client.request(&cdns_endpoint).await?;
            let typed = client
                .request_typed::<ProductCdnsResponse>(&cdns_endpoint)
                .await?;
            Ok::<_, ribbit_client::Error>((response, typed))
        },
        client.request_typed::<SummaryResponse>(&Endpoint::Summary)
    )
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
            println!("Product Information: {}", product);
            println!("{}", "=".repeat(40));

            if let Some(summary_entry) = summary.get_product(&product) {
                println!("\nSummary:");
                println!("  Sequence: {}", summary_entry.seqn);
            }

            if let Some(version) = versions.get_region(region.as_str()) {
                println!("\nCurrent Version ({}):", region.as_str());
                println!("  Version:      {}", version.versions_name);
                println!("  Build ID:     {}", version.build_id);
                println!("  Build Config: {}", version.build_config);
                println!("  CDN Config:   {}", version.cdn_config);
            }

            println!("\nAvailable Regions:");
            let mut regions: Vec<_> = versions.entries.iter().map(|e| &e.region).collect();
            regions.sort();
            regions.dedup();
            for r in regions {
                println!("  - {}", r);
            }

            println!("\nCDN Hosts:");
            for host in cdns.all_hosts() {
                println!("  - {}", host);
            }
        }
    }

    Ok(())
}
