use crate::{
    ConfigCommands, OutputFormat,
    config_manager::{ConfigManager, ConfigError},
    output::{
        OutputStyle, create_table, format_error, format_success, header_cell, print_section_header,
        regular_cell,
    },
};

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
    let config_manager = ConfigManager::new()?;
    let config = config_manager.get_all();

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
    let mut config_manager = ConfigManager::new()?;
    config_manager.set(key.clone(), value.clone())?;
    
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
    let config_manager = ConfigManager::new()?;
    let value = config_manager.get(&key);

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let result = match &value {
                Ok(val) => serde_json::json!({
                    "key": key,
                    "value": val,
                    "found": true,
                }),
                Err(_) => serde_json::json!({
                    "key": key,
                    "value": null,
                    "found": false,
                }),
            };
            println!("{}", serde_json::to_string(&result)?);
        }
        _ => {
            let style = OutputStyle::new();
            match value {
                Ok(val) => println!("{val}"),
                Err(ConfigError::KeyNotFound { key }) => {
                    eprintln!(
                        "{}",
                        format_error(&format!("Configuration key '{key}' not found"), &style)
                    );
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        format_error(&format!("Configuration error: {e}"), &style)
                    );
                    std::process::exit(1);
                }
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

    let mut config_manager = ConfigManager::new()?;
    config_manager.reset()?;
    
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
