use crate::{
    ConfigCommands, OutputFormat,
    output::{
        OutputStyle, create_table, format_error, format_success, header_cell, print_section_header,
        regular_cell,
    },
};
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
    // Show all configuration options with their default values
    let config = HashMap::from([
        ("default_region", "us"),
        ("cache_dir", "~/.cache/ngdp"),
        ("timeout", "30"),
        ("cache_enabled", "true"),
        ("cache_ttl", "1800"),  // 30 minutes in seconds
        ("max_concurrent_downloads", "4"),
        ("user_agent", "ngdp-client/0.1.2"),
        ("verify_certificates", "true"),
        ("proxy_url", ""),
        ("ribbit_timeout", "30"),
        ("tact_timeout", "30"),
        ("retry_attempts", "3"),
        ("log_file", ""),
        ("color_output", "true"),
        ("fallback_to_tact", "true"),
        ("use_community_cdn_fallbacks", "true"),
    ]);

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&config)?
            } else {
                serde_json::to_string(&config)?
            };
            println!("{output}");
        }
        _ => {
            let style = OutputStyle::new();

            print_section_header("Current Configuration", &style);

            let mut table = create_table(&style);
            table.set_header(vec![
                header_cell("Key", &style),
                header_cell("Value", &style),
            ]);

            let mut sorted_config: Vec<_> = config.iter().collect();
            sorted_config.sort_by(|a, b| a.0.cmp(b.0));

            for (key, value) in sorted_config {
                table.add_row(vec![regular_cell(key), regular_cell(value)]);
            }

            println!("{table}");
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
            let style = OutputStyle::new();
            println!(
                "{}",
                format_success(&format!("✓ Set {key} = {value}"), &style)
            );
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
        "cache_enabled" => Some("true"),
        "cache_ttl" => Some("1800"),
        "max_concurrent_downloads" => Some("4"),
        "user_agent" => Some("ngdp-client/0.1.2"),
        "verify_certificates" => Some("true"),
        "proxy_url" => Some(""),
        "ribbit_timeout" => Some("30"),
        "tact_timeout" => Some("30"),
        "retry_attempts" => Some("3"),
        "log_file" => Some(""),
        "color_output" => Some("true"),
        "fallback_to_tact" => Some("true"),
        "use_community_cdn_fallbacks" => Some("true"),
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
            let style = OutputStyle::new();
            if let Some(value) = value {
                println!("{value}");
            } else {
                eprintln!(
                    "{}",
                    format_error(&format!("Configuration key '{key}' not found"), &style)
                );
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

async fn reset_config(yes: bool, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    let style = OutputStyle::new();

    if !yes {
        eprintln!(
            "{}",
            format_error("Reset requires confirmation. Use --yes to confirm.", &style)
        );
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
            println!(
                "{}",
                format_success("✓ Configuration reset to defaults", &style)
            );
        }
    }

    Ok(())
}
