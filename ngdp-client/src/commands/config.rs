use crate::{ConfigCommands, OutputFormat};
use std::collections::HashMap;

pub async fn handle(
    cmd: ConfigCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ConfigCommands::Show => show_config(format).await,
        ConfigCommands::Set { key, value } => set_config(key, value, format).await,
        ConfigCommands::Get { key } => get_config(key, format).await,
        ConfigCommands::Reset { yes } => reset_config(yes, format).await,
    }
}

async fn show_config(format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual config loading
    let config = HashMap::from([
        ("default_region", "us"),
        ("cache_dir", "~/.cache/ngdp"),
        ("timeout", "30"),
    ]);

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&config)?
            } else {
                serde_json::to_string(&config)?
            };
            println!("{}", output);
        }
        _ => {
            println!("Current configuration:");
            for (key, value) in &config {
                println!("  {}: {}", key, value);
            }
        }
    }

    Ok(())
}

async fn set_config(
    key: String,
    value: String,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual config saving
    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let result = serde_json::json!({
                "success": true,
                "key": key,
                "value": value,
            });
            println!("{}", serde_json::to_string(&result)?);
        }
        _ => {
            println!("Set {} = {}", key, value);
        }
    }
    Ok(())
}

async fn get_config(key: String, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual config loading
    let value = match key.as_str() {
        "default_region" => Some("us"),
        "cache_dir" => Some("~/.cache/ngdp"),
        "timeout" => Some("30"),
        _ => None,
    };

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let result = serde_json::json!({
                "key": key,
                "value": value,
                "found": value.is_some(),
            });
            println!("{}", serde_json::to_string(&result)?);
        }
        _ => {
            if let Some(value) = value {
                println!("{}", value);
            } else {
                eprintln!("Configuration key '{}' not found", key);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

async fn reset_config(yes: bool, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    if !yes {
        eprintln!("Reset requires confirmation. Use --yes to confirm.");
        std::process::exit(1);
    }

    // TODO: Implement actual config reset
    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let result = serde_json::json!({
                "success": true,
                "message": "Configuration reset to defaults",
            });
            println!("{}", serde_json::to_string(&result)?);
        }
        _ => {
            println!("Configuration reset to defaults");
        }
    }

    Ok(())
}
