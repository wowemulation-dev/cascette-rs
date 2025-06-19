use crate::{InspectCommands, OutputFormat};
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
            println!("BPSV Document Analysis");
            println!("{}", "=".repeat(40));

            println!("\nSchema:");
            for (i, field) in doc.schema().fields().iter().enumerate() {
                println!("  [{}] {} ({})", i, field.name, field.field_type);
            }

            if let Some(seq) = doc.sequence_number() {
                println!("\nSequence Number: {}", seq);
            }

            println!("\nData:");
            println!("  Rows: {}", doc.rows().len());

            if !doc.rows().is_empty() {
                println!("\nFirst 5 rows:");
                for (i, row) in doc.rows().iter().take(5).enumerate() {
                    println!("\n  Row {}:", i + 1);
                    for (field, value) in doc.schema().field_names().iter().zip(row.raw_values()) {
                        println!("    {}: {}", field, value);
                    }
                }

                if doc.rows().len() > 5 {
                    println!("\n  ... and {} more rows", doc.rows().len() - 5);
                }
            }
        }
    }

    Ok(())
}
