use crate::{
    InspectCommands, OutputFormat,
    output::{
        OutputStyle, create_table, format_count_badge, format_header, format_key_value,
        header_cell, numeric_cell, print_section_header, print_subsection_header, regular_cell,
    },
};
use ngdp_bpsv::BpsvDocument;

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
            println!("Build config inspection not yet implemented");
            println!("Product: {}", product);
            println!("Build: {}", build);
            println!("Region: {}", region);
        }
        InspectCommands::CdnConfig { product, region } => {
            println!("CDN config inspection not yet implemented");
            println!("Product: {}", product);
            println!("Region: {}", region);
        }
        InspectCommands::Encoding { file, stats } => {
            println!("Encoding inspection not yet implemented");
            println!("File: {:?}", file);
            println!("Stats: {}", stats);
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
        println!("{}", data);
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
            println!("{}", output);
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
            println!("{}", schema_table);

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
                    format_header(&format!("Preview (first {} rows)", preview_count), &style)
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

                println!("{}", data_table);

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
