use crate::OutputFormat;
use clap::Subcommand;
use serde_json;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Subcommand)]
pub enum KeysCommands {
    /// Update the encryption key database from GitHub
    Update {
        /// Custom output path for the key file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Force update even if local file is recent
        #[arg(short, long)]
        force: bool,
    },

    /// Show current key database status
    Status,
}

pub async fn handle_keys_command(
    command: KeysCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        KeysCommands::Update { output, force } => update_keys(output, force, format).await,
        KeysCommands::Status => show_key_status(format),
    }
}

async fn update_keys(
    output: Option<PathBuf>,
    force: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // Default path: ~/.config/cascette/WoW.txt
    let output_path = output.unwrap_or_else(|| {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cascette");
        config_dir.join("WoW.txt")
    });

    // Check if we should update
    if !force && output_path.exists() {
        let metadata = fs::metadata(&output_path)?;
        if let Ok(modified) = metadata.modified() {
            let age = std::time::SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default();

            // Skip if file is less than 24 hours old
            if age.as_secs() < 86400 {
                info!(
                    "Key file is recent ({}h old), skipping update. Use --force to override.",
                    age.as_secs() / 3600
                );
                return show_key_status(format);
            }
        }
    }

    info!("Downloading latest TACTKeys from GitHub...");

    // Download from GitHub
    let url = "https://raw.githubusercontent.com/wowdev/TACTKeys/master/WoW.txt";
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download keys: HTTP {}", response.status()).into());
    }

    let content = response.text().await?;

    info!("Processing key file...");

    // Count valid keys
    let mut key_count = 0;
    let mut new_keys = 0;
    let existing_keys = if output_path.exists() {
        fs::read_to_string(&output_path).ok()
    } else {
        None
    };

    // Parse and validate keys
    let mut valid_lines = Vec::new();
    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            valid_lines.push(line.to_string());
            continue;
        }

        // Parse key line (format: keyname keyhex [description])
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            // Validate key name (16 hex chars)
            if parts[0].len() == 16 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
                // Validate key value (32 hex chars)
                if parts[1].len() == 32 && parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
                    key_count += 1;

                    // Check if this is a new key
                    if let Some(ref existing) = existing_keys {
                        if !existing.contains(parts[0]) {
                            new_keys += 1;
                        }
                    } else {
                        new_keys += 1;
                    }

                    valid_lines.push(line.to_string());
                } else {
                    warn!("Invalid key value for {}: {}", parts[0], parts[1]);
                }
            } else {
                warn!("Invalid key name: {}", parts[0]);
            }
        }
    }

    info!("Writing key file...");

    // Ensure directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write to file
    fs::write(&output_path, valid_lines.join("\n"))?;

    // Report results
    if new_keys > 0 {
        info!(
            "✅ Updated key database: {} total keys ({} new)",
            key_count, new_keys
        );
    } else {
        info!("✅ Key database is up to date: {} total keys", key_count);
    }
    info!("📁 Key file saved to: {}", output_path.display());

    Ok(())
}

fn show_key_status(format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cascette");
    let key_file = config_dir.join("WoW.txt");

    if !key_file.exists() {
        warn!("No key file found at {}", key_file.display());
        info!("Run 'ngdp keys update' to download the latest keys");
        return Ok(());
    }

    let content = fs::read_to_string(&key_file)?;

    let mut key_count = 0;
    let mut key_names = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2
            && parts[0].len() == 16
            && parts[0].chars().all(|c| c.is_ascii_hexdigit())
            && parts[1].len() == 32
            && parts[1].chars().all(|c| c.is_ascii_hexdigit())
        {
            key_count += 1;
            if key_count <= 5 {
                key_names.push(parts[0].to_string());
            }
        }
    }

    let metadata = fs::metadata(&key_file)?;
    let file_size = metadata.len();
    let modified = metadata
        .modified()
        .ok()
        .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())
        .map(|d| format!("{}h ago", d.as_secs() / 3600))
        .unwrap_or_else(|| "unknown".to_string());

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let status = serde_json::json!({
                "location": key_file.display().to_string(),
                "total_keys": key_count,
                "file_size_bytes": file_size,
                "file_size_kb": file_size / 1024,
                "last_updated": modified,
                "sample_keys": key_names,
                "status": "loaded"
            });
            let output = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&status)?
            } else {
                serde_json::to_string(&status)?
            };
            println!("{output}");
        }
        _ => {
            info!("📊 Key Database Status");
            info!("  Location: {}", key_file.display());
            info!("  Total keys: {}", key_count);
            info!("  File size: {} KB", file_size / 1024);
            info!("  Last updated: {}", modified);

            if !key_names.is_empty() {
                info!("  Sample keys:");
                for name in key_names {
                    info!("    - {}", name);
                }
                if key_count > 5 {
                    info!("    ... and {} more", key_count - 5);
                }
            }
        }
    }

    Ok(())
}
