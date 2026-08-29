#![allow(clippy::expect_used, clippy::panic)]
//! Verification pipeline demo
//!
//! Demonstrates `VerifyConfig` with the three `VerifyMode` levels,
//! `VerifyPipeline` construction, and `ProgressEvent::VerifyResult` handling.
//! If the `CASCETTE_WOW_PATH` environment variable is set, opens and verifies
//! a real CASC installation. Otherwise, demonstrates configuration only.
//!
//! ```text
//! cargo run -p cascette-installation --example verify_installation --features local-install
//! ```

mod common;

use std::path::PathBuf;

use cascette_installation::VerifyPipeline;
use cascette_installation::config::{VerifyConfig, VerifyMode};
use cascette_installation::progress::ProgressEvent;

#[tokio::main]
async fn main() {
    println!("=== VerifyMode Levels ===");
    println!();

    println!("VerifyMode::Existence");
    println!("  Checks that index entries exist in the archive files.");
    println!("  Fastest mode. Does not read file contents.");
    println!();

    println!("VerifyMode::Size");
    println!("  Checks existence and validates that the archive entry");
    println!("  can be read at the expected offset and size.");
    println!("  Catches truncated or corrupted archive files.");
    println!();

    println!("VerifyMode::Full");
    println!("  Reads and decompresses each BLTE entry from the archive.");
    println!("  Validates full data integrity. Slowest mode.");
    println!();

    println!("=== VerifyConfig Creation ===");
    println!();

    // Check if a real installation path was provided.
    let wow_path = std::env::var("CASCETTE_WOW_PATH").ok().map(PathBuf::from);

    if let Some(ref path) = wow_path {
        println!("CASCETTE_WOW_PATH is set: {}", path.display());
    } else {
        println!("CASCETTE_WOW_PATH is not set. Using demo path.");
        println!("Set CASCETTE_WOW_PATH to a real installation to run verification.");
    }
    println!();

    let install_path = wow_path.clone().unwrap_or_else(common::default_wow_path);

    // VerifyConfig::new defaults to VerifyMode::Existence.
    let config = VerifyConfig::new(install_path.clone());
    println!("Default mode: {:?}", config.mode);
    println!("Install path: {}", config.install_path.display());
    println!();

    // Override the mode to demonstrate all three levels.
    for mode in [VerifyMode::Existence, VerifyMode::Size, VerifyMode::Full] {
        println!("  VerifyConfig {{ mode: {mode:?}, .. }}");
    }
    println!();

    println!("=== VerifyPipeline ===");
    println!();

    println!("VerifyPipeline::new(config) creates the pipeline.");
    println!("pipeline.run(progress_callback) executes verification.");
    println!("Returns VerifyReport with: total, valid, invalid, missing, invalid_keys.");
    println!();

    println!("=== ProgressEvent::VerifyResult ===");
    println!();

    // Show what the progress callback receives.
    let demo_events = vec![
        ProgressEvent::VerifyResult {
            path: "abc123def456abc123def456abc123de".to_string(),
            valid: true,
        },
        ProgressEvent::VerifyResult {
            path: "deadbeefdeadbeefdeadbeefdeadbeef".to_string(),
            valid: false,
        },
    ];

    for event in &demo_events {
        println!("  {event:?}");
    }
    println!();

    println!("The callback receives one VerifyResult per index entry.");
    println!("The `path` field is the encoding key in hex.");
    println!("The `valid` field indicates whether the entry passed the check.");
    println!();

    // If a real installation path was provided, run verification.
    if wow_path.is_some() {
        run_verification(install_path).await;
    } else {
        println!("=== Dry Run Complete ===");
        println!();
        println!("No real installation available. Set CASCETTE_WOW_PATH to verify one.");
    }
}

async fn run_verification(install_path: PathBuf) {
    println!("=== Running Verification ===");
    println!();
    println!("Install path: {}", install_path.display());
    println!("Mode: Existence (fastest)");
    println!();

    let config = VerifyConfig {
        install_path,
        mode: VerifyMode::Existence,
    };

    let pipeline = VerifyPipeline::new(config);

    let report = pipeline
        .run(|event| {
            if let ProgressEvent::VerifyResult { ref path, valid } = event {
                let status = if valid { "OK" } else { "FAIL" };
                // Truncate key display for readability.
                let short_key = if path.len() > 12 { &path[..12] } else { path };
                println!("  [{status}] {short_key}...");
            }
        })
        .await;

    match report {
        Ok(report) => {
            println!("=== Verification Results ===");
            println!();
            println!("Total entries: {}", report.total);
            println!("Valid:         {}", report.valid);
            println!("Invalid:       {}", report.invalid);
            println!("Missing:       {}", report.missing);
            if !report.invalid_keys.is_empty() {
                println!();
                println!("Invalid keys:");
                for key in &report.invalid_keys {
                    println!("  {key}");
                }
            }
        }
        Err(e) => {
            println!("Verification failed: {e}");
            println!();
            println!("Make sure the path contains a valid CASC installation");
            println!("with Data/data/ directory and index files.");
        }
    }
}
