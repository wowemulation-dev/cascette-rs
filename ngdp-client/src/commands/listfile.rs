use crate::{ListfileCommands, OutputFormat};
use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tracing::debug;

const LISTFILE_URL: &str =
    "https://github.com/wowdev/wow-listfile/releases/latest/download/community-listfile.csv";

pub async fn handle(
    cmd: ListfileCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ListfileCommands::Download { output, force } => {
            handle_download(output, force, format).await
        }
        ListfileCommands::Info { path } => handle_info(path, format).await,
        ListfileCommands::Search {
            pattern,
            path,
            ignore_case,
            limit,
        } => handle_search(pattern, path, ignore_case, limit, format).await,
    }
}

async fn handle_download(
    output_dir: PathBuf,
    force: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_file = output_dir.join("community-listfile.csv");

    if output_file.exists() && !force {
        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "status": "skipped",
                    "message": "File already exists. Use --force to overwrite.",
                    "path": output_file
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                println!("📁 File already exists: {output_file:?}");
                println!("   Use --force to overwrite");
            }
            OutputFormat::Bpsv => {
                println!("status = skipped");
                println!("path = {output_file:?}");
            }
        }
        return Ok(());
    }

    if let OutputFormat::Text = format {
        println!("📥 Downloading community listfile...");
        println!("   URL: {}", LISTFILE_URL.cyan());
        println!("   Output: {output_dir:?}");
    }

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output_dir)?;

    // Download the file
    let response = reqwest::get(LISTFILE_URL).await?;
    let content = response.text().await?;

    // Write to file
    fs::write(&output_file, &content)?;

    // Parse to get basic stats
    let line_count = content.lines().count();
    let file_size = content.len();

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json = serde_json::json!({
                "status": "success",
                "path": output_file,
                "size": file_size,
                "entries": line_count,
                "url": LISTFILE_URL
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", serde_json::to_string(&json)?);
            }
        }
        OutputFormat::Text => {
            println!("✅ Downloaded successfully!");
            println!("   File: {output_file:?}");
            println!("   Size: {} bytes", file_size.to_string().green());
            println!("   Entries: {}", line_count.to_string().cyan());
        }
        OutputFormat::Bpsv => {
            println!("status = success");
            println!("path = {output_file:?}");
            println!("size = {file_size}");
            println!("entries = {line_count}");
        }
    }

    Ok(())
}

async fn handle_info(
    path: PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        match format {
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let json = serde_json::json!({
                    "error": "File not found",
                    "path": path
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                println!("❌ File not found: {path:?}");
                println!("   Run: ngdp storage listfile download");
            }
            OutputFormat::Bpsv => {
                println!("error = file_not_found");
                println!("path = {path:?}");
            }
        }
        return Ok(());
    }

    let file = fs::File::open(&path)?;
    let reader = BufReader::new(file);

    let mut total_lines = 0;
    let mut sample_entries = Vec::new();
    let mut fdid_count = 0;
    let mut unique_extensions = std::collections::HashSet::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        total_lines += 1;

        if i < 5 {
            sample_entries.push(line.clone());
        }

        if let Some(sep_pos) = line.find(';') {
            if let Ok(_fdid) = line[..sep_pos].parse::<u32>() {
                fdid_count += 1;

                let filename = &line[sep_pos + 1..];
                if let Some(ext_pos) = filename.rfind('.') {
                    let extension = &filename[ext_pos + 1..].to_lowercase();
                    unique_extensions.insert(extension.to_string());
                }
            }
        }
    }

    let file_size = fs::metadata(&path)?.len();
    let mut extensions: Vec<_> = unique_extensions.into_iter().collect();
    extensions.sort();

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json = serde_json::json!({
                "path": path,
                "size": file_size,
                "total_entries": total_lines,
                "valid_entries": fdid_count,
                "extensions": extensions,
                "sample_entries": sample_entries
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", serde_json::to_string(&json)?);
            }
        }
        OutputFormat::Text => {
            println!("📄 Community Listfile Information");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("  File:         {path:?}");
            println!("  Size:         {} bytes", file_size.to_string().green());
            println!("  Total Lines:  {}", total_lines.to_string().cyan());
            println!("  Valid Entries: {}", fdid_count.to_string().cyan());

            if !extensions.is_empty() {
                println!("  File Types:   {} types", extensions.len());

                // Show top 10 extensions
                let display_extensions: Vec<_> = extensions.into_iter().take(10).collect();
                println!("    Extensions: {}", display_extensions.join(", "));
            }

            if !sample_entries.is_empty() {
                println!("\n📋 Sample Entries:");
                for entry in &sample_entries {
                    println!("    {entry}");
                }
            }
        }
        OutputFormat::Bpsv => {
            println!("## Listfile Information");
            println!("path = {path:?}");
            println!("size = {file_size}");
            println!("total_entries = {total_lines}");
            println!("valid_entries = {fdid_count}");
            println!("extensions = {}", extensions.len());
        }
    }

    Ok(())
}

async fn handle_search(
    pattern: String,
    path: PathBuf,
    ignore_case: bool,
    limit: usize,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        match format {
            OutputFormat::Text => {
                println!("❌ Listfile not found: {path:?}");
                println!("   Run: ngdp storage listfile download");
            }
            _ => {
                let json = serde_json::json!({
                    "error": "File not found",
                    "path": path
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
        return Ok(());
    }

    // Create regex pattern
    let regex = if ignore_case {
        Regex::new(&format!("(?i){pattern}"))?
    } else {
        Regex::new(&pattern)?
    };

    let file = fs::File::open(&path)?;
    let reader = BufReader::new(file);

    let mut matches = Vec::new();
    let mut total_checked = 0;

    for line in reader.lines() {
        let line = line?;
        total_checked += 1;

        if regex.is_match(&line) {
            if let Some(sep_pos) = line.find(';') {
                if let Ok(fdid) = line[..sep_pos].parse::<u32>() {
                    let filename = &line[sep_pos + 1..];
                    matches.push((fdid, filename.to_string()));

                    if matches.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let json = serde_json::json!({
                "pattern": pattern,
                "ignore_case": ignore_case,
                "total_checked": total_checked,
                "matches_found": matches.len(),
                "matches": matches.into_iter().map(|(fdid, filename)| {
                    serde_json::json!({
                        "file_data_id": fdid,
                        "filename": filename
                    })
                }).collect::<Vec<_>>()
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", serde_json::to_string(&json)?);
            }
        }
        OutputFormat::Text => {
            println!("🔍 Search Results for: {}", pattern.yellow());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("  Pattern:       {pattern}");
            println!(
                "  Case sensitive: {}",
                if ignore_case { "No" } else { "Yes" }
            );
            println!("  Entries checked: {total_checked}");
            println!("  Matches found: {}", matches.len().to_string().green());

            if !matches.is_empty() {
                println!("\n📋 Results:");
                println!("{:<10} Filename", "FileDataID");
                println!("{}", "─".repeat(80));

                for (fdid, filename) in matches {
                    println!("{:<10} {}", fdid.to_string().cyan(), filename);
                }
            }
        }
        OutputFormat::Bpsv => {
            println!("## Search Results");
            println!("pattern = {pattern}");
            println!("ignore_case = {ignore_case}");
            println!("total_checked = {total_checked}");
            println!("matches_found = {}", matches.len());

            for (fdid, filename) in matches {
                println!("match = {fdid} {filename}");
            }
        }
    }

    Ok(())
}

/// Parse a listfile and return FileDataID -> filename mapping
pub fn parse_listfile(path: &PathBuf) -> Result<HashMap<u32, String>, Box<dyn std::error::Error>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut mapping = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        if let Some(sep_pos) = line.find(';') {
            if let Ok(fdid) = line[..sep_pos].parse::<u32>() {
                let filename = line[sep_pos + 1..].to_string();
                mapping.insert(fdid, filename);
            }
        }
    }

    debug!("Loaded {} filename mappings from listfile", mapping.len());
    Ok(mapping)
}
