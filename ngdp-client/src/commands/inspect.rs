use crate::{
    InspectCommands, OutputFormat,
    cached_client::create_client,
    output::{
        OutputStyle, create_table, format_count_badge, format_header, format_key_value,
        header_cell, numeric_cell, print_section_header, print_subsection_header, regular_cell,
    },
};
use ngdp_bpsv::BpsvDocument;
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cdn::{
    CdnClientBuilderTrait as _, CdnClientWithFallbackBuilder, FallbackCdnClientTrait as _,
};
use ribbit_client::{CdnEntry, Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region};
use std::{collections::BTreeMap, io::Cursor, str::FromStr as _};
use tact_parser::{
    Md5,
    archive::ArchiveIndexParser,
    config::{BuildConfig, CdnConfig, ConfigParsable as _},
};
use thiserror::Error;
use tokio::io::{AsyncBufRead, BufReader};
use tracing::*;

pub async fn handle(
    cmd: InspectCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InspectCommands::Archives { product, region } => {
            inspect_archives(product, region, format).await?
        }
        InspectCommands::Bpsv { input, raw } => inspect_bpsv(input, raw, format).await?,
        InspectCommands::BuildConfig {
            product,
            config,
            region,
        } => inspect_build_config(product, config, region, format).await?,
        InspectCommands::CdnConfig {
            product,
            config,
            region,
        } => inspect_cdn_config(product, config, region, format).await?,
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

/// Error type for fallback client operations
#[derive(Error, Debug)]
enum InspectError {
    #[error("Region config not found on CDN")]
    RegionNotFound,

    #[error("No CDNs found in region")]
    NoCdnsFoundInRegion,

    #[error("BPSV output not supported")]
    BpsvNotSupported,

    #[error("No 'archives' entry in CDN configuration")]
    NoArchivesInCdnConfiguration,
}

async fn inspect_cdn_config(
    product: String,
    config: Option<String>,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region_enum = Region::from_str(&region)?;
    let client = create_client(region_enum).await?;

    let config = match config {
        Some(config) => config,
        None => {
            let endpoint = Endpoint::ProductVersions(product.clone());
            let versions: ProductVersionsResponse = client.request_typed(&endpoint).await?;

            let version = versions
                .get_region(region.as_str())
                .ok_or(InspectError::RegionNotFound)?;
            version.cdn_config.clone()
        }
    };

    // Fetch the CDN host list
    let endpoint = Endpoint::ProductCdns(product.clone());
    let cdns: ProductCdnsResponse = client.request_typed(&endpoint).await?;

    // Filter CDN entries for the specified region
    let filtered_entries: Vec<&CdnEntry> =
        cdns.entries.iter().filter(|e| e.name == region).collect();

    if filtered_entries.is_empty() {
        return Err(Box::new(InspectError::NoCdnsFoundInRegion));
    }

    // Pick CDN to fetch config
    let cdn_client_builder = CdnClientWithFallbackBuilder::<CachedCdnClient>::new();
    // TODO: configure cache directory
    let cdn_entry = filtered_entries.first().unwrap();
    let cdn_client_builder = cdn_client_builder.add_primary_cdns(cdn_entry.hosts.iter());
    let cdn_client = cdn_client_builder.build().await?;

    let cdn_config = cdn_client
        .download_cdn_config(&cdn_entry.path, &config)
        .await?;
    let cdn_config = CdnConfig::parse_config(Cursor::new(cdn_config.bytes().await?))?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = serde_json::json!({
                "config": config,
                "archive_group": cdn_config.archive_group.map(hex::encode),
                "patch_archive_group": cdn_config.patch_archive_group.map(hex::encode),

                "file_index": cdn_config.file_index.map(hex::encode),
                "file_index_size": cdn_config.file_index_size,
                "patch_file_index": cdn_config.patch_file_index.map(hex::encode),
                "patch_file_index_size": cdn_config.patch_file_index_size,

                "archives": cdn_config.archives_with_index_size().map(
                    |o| {
                        o.map(|(hash, size): (&Md5, u32)| {
                            (hex::encode(hash), size)
                        }).collect::<Vec<_>>()
                    }),
                "patch_archives": cdn_config.patch_archives_with_index_size().map(
                    |o| {
                        o.map(|(hash, size): (&Md5, u32)| {
                            (hex::encode(hash), size)
                        }).collect::<Vec<_>>()
                    }),
            });

            if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_writer_pretty(std::io::stdout(), &json_data)?;
            } else {
                serde_json::to_writer(std::io::stdout(), &json_data)?;
            };
        }
        OutputFormat::Bpsv => {
            return Err(Box::new(InspectError::BpsvNotSupported));
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            // TODO: add missing fields
            print_section_header(&format!("CDN configuration {config}"), &style);

            print_subsection_header("Archives", &style);

            if let Some(rows) = cdn_config.archives_with_index_size() {
                let rows_count = rows.size_hint().0;
                let preview_count = rows_count.min(5);
                println!(
                    "\n{}",
                    format_header(&format!("Preview (first {preview_count} rows)"), &style)
                );
                let mut data_table = create_table(&style);
                data_table.set_header([
                    header_cell("#", &style),
                    header_cell("Archive", &style),
                    header_cell("Index size", &style),
                ]);
                for (i, (archive, archive_index_size)) in rows.take(preview_count).enumerate() {
                    data_table.add_row([
                        numeric_cell(&(i + 1).to_string()),
                        regular_cell(&hex::encode(archive)),
                        numeric_cell(&archive_index_size.to_string()),
                    ]);
                }

                println!("{data_table}");

                if rows_count > preview_count {
                    println!(
                        "\n{}",
                        format_header(
                            &format!("... and {} more rows", rows_count - preview_count),
                            &style
                        )
                    );
                }
            }

            print_subsection_header("Patch archives", &style);
            if let Some(rows) = cdn_config.patch_archives_with_index_size() {
                let rows_count = rows.size_hint().0;
                let preview_count = rows_count.min(5);
                println!(
                    "\n{}",
                    format_header(&format!("Preview (first {preview_count} rows)"), &style)
                );
                let mut data_table = create_table(&style);
                data_table.set_header([
                    header_cell("#", &style),
                    header_cell("Patch archive", &style),
                    header_cell("Index size", &style),
                ]);
                for (i, (patch_archive, patch_archive_index_size)) in
                    rows.take(preview_count).enumerate()
                {
                    data_table.add_row([
                        numeric_cell(&(i + 1).to_string()),
                        regular_cell(&hex::encode(patch_archive)),
                        numeric_cell(&patch_archive_index_size.to_string()),
                    ]);
                }

                println!("{data_table}");

                if rows_count > preview_count {
                    println!(
                        "\n{}",
                        format_header(
                            &format!("... and {} more rows", rows_count - preview_count),
                            &style
                        )
                    );
                }
            }
        }
    }

    Ok(())
}

async fn inspect_archives(
    product: String,
    region: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region_enum = Region::from_str(&region)?;
    let client = create_client(region_enum).await?;

    let endpoint = Endpoint::ProductVersions(product.clone());
    let versions: ProductVersionsResponse = client.request_typed(&endpoint).await?;

    let version = versions
        .get_region(region.as_str())
        .ok_or(InspectError::RegionNotFound)?;

    // Fetch the CDN host list
    let endpoint = Endpoint::ProductCdns(product.clone());
    let cdns: ProductCdnsResponse = client.request_typed(&endpoint).await?;

    // Filter CDN entries for the specified region
    let filtered_entries: Vec<&CdnEntry> =
        cdns.entries.iter().filter(|e| e.name == region).collect();

    if filtered_entries.is_empty() {
        return Err(Box::new(InspectError::NoCdnsFoundInRegion));
    }

    // Pick CDN to fetch config
    let cdn_client_builder = CdnClientWithFallbackBuilder::<CachedCdnClient>::new();
    // TODO: configure cache directory
    let cdn_entry = filtered_entries.first().unwrap();
    let cdn_client_builder = cdn_client_builder.add_primary_cdns(cdn_entry.hosts.iter());
    let cdn_client = cdn_client_builder.build().await?;

    let cdn_config = cdn_client
        .download_cdn_config(&cdn_entry.path, &version.cdn_config)
        .await?;
    let cdn_config =
        CdnConfig::aparse_config(BufReader::new(cdn_config.into_inner())).await?;

    // Fetch all the archive indexes
    let Some(archives) = cdn_config.archives_with_index_size() else {
        return Err(Box::new(InspectError::NoArchivesInCdnConfiguration));
    };

    // let mut ekeys = BTreeMap::new();
    info!("Downloading archive indexes...");
    for (archive, size) in archives {
        let hash = hex::encode(archive);
        debug!("Downloading archive index {hash} ({size})...");

        let archive_data = cdn_client
            .download_data_index(&cdn_entry.path, &hash)
            .await?;

        todo!();
        // let mut archive_index =
        //     ArchiveIndexParser::new(BufReader::new(archive_data.to_inner()), archive)?;

        // for block in 0..archive_index.toc().num_blocks {
        //     for entry in archive_index.read_block(block)? {
        //         ekeys.insert(
        //             entry.ekey,
        //             (archive, entry.archive_offset, entry.blte_encoded_size),
        //         );
        //     }
        // }
    }

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json_data = serde_json::json!({
                "config": version.cdn_config,
                // "ekeys": ekeys.into_iter().map(
                //     |(ekey, (archive, off, size))| {
                //         (hex::encode(ekey), (hex::encode(archive), off, size))
                //     }).collect::<BTreeMap<_, _>>(),
            });

            if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_writer_pretty(std::io::stdout(), &json_data)?;
            } else {
                serde_json::to_writer(std::io::stdout(), &json_data)?;
            };
        }
        OutputFormat::Bpsv => {
            return Err(Box::new(InspectError::BpsvNotSupported));
        }
        OutputFormat::Text => {
            let style = OutputStyle::new();

            // TODO: add missing fields
            print_section_header(
                &format!("Archives in configuration {}", version.cdn_config),
                &style,
            );

            print_subsection_header("Archive index", &style);

            // let rows_count = ekeys.len();
            // let preview_count = rows_count.min(5);
            // println!(
            //     "\n{}",
            //     format_header(&format!("Preview (first {preview_count} rows)"), &style)
            // );
            let mut data_table = create_table(&style);
            data_table.set_header([
                header_cell("#", &style),
                header_cell("EKey", &style),
                header_cell("CDN archive", &style),
                header_cell("Offset", &style),
                header_cell("Length", &style),
            ]);
            // for (i, (ekey, (archive, off, size))) in ekeys.iter().take(preview_count).enumerate() {
            //     data_table.add_row([
            //         numeric_cell(&(i + 1).to_string()),
            //         regular_cell(&hex::encode(ekey)),
            //         regular_cell(&hex::encode(archive)),
            //         numeric_cell(&format!("{off:#x}")),
            //         numeric_cell(&format!("{size:#x}")),
            //     ]);
            // }

            println!("{data_table}");

            // if rows_count > preview_count {
            //     println!(
            //         "\n{}",
            //         format_header(
            //             &format!("... and {} more rows", rows_count - preview_count),
            //             &style
            //         )
            //     );
            // }
        }
    }

    Ok(())
}

async fn inspect_build_config(
    product: String,
    config: Option<String>,
    region: String,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let region_enum = Region::from_str(&region)?;
    let client = create_client(region_enum).await?;

    let config = match config {
        Some(config) => config,
        None => {
            let endpoint = Endpoint::ProductVersions(product.clone());
            let versions: ProductVersionsResponse = client.request_typed(&endpoint).await?;

            let version = versions
                .get_region(region.as_str())
                .ok_or(InspectError::RegionNotFound)?;
            version.cdn_config.clone()
        }
    };

    // Fetch the CDN host list
    let endpoint = Endpoint::ProductCdns(product.clone());
    let cdns: ProductCdnsResponse = client.request_typed(&endpoint).await?;

    // Filter CDN entries for the specified region
    let filtered_entries: Vec<&CdnEntry> =
        cdns.entries.iter().filter(|e| e.name == region).collect();

    if filtered_entries.is_empty() {
        return Err(Box::new(InspectError::NoCdnsFoundInRegion));
    }

    // Pick CDN to fetch config
    let cdn_client_builder = CdnClientWithFallbackBuilder::<CachedCdnClient>::new();
    // TODO: configure cache directory
    let cdn_entry = filtered_entries.first().unwrap();
    let cdn_client_builder = cdn_client_builder.add_primary_cdns(cdn_entry.hosts.iter());
    let cdn_client = cdn_client_builder.build().await?;

    let cdn_config = cdn_client
        .download_cdn_config(&cdn_entry.path, &config)
        .await?;
    let build_config =
        BuildConfig::aparse_config(BufReader::new(cdn_config.into_inner())).await?;

    info!("Build config: {build_config:?}");
    todo!()
}
