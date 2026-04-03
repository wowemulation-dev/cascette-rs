//! Metadata Orchestrator Example
//!
//! Demonstrates initializing community data providers, building the
//! `MetadataOrchestrator`, and performing FileDataID resolution with
//! health/stats reporting.
//!
//! Run with: cargo run --example metadata_orchestrator -p cascette-metadata
//!
//! Requires network access (downloads listfile and TACT keys from GitHub).

use cascette_import::providers::ImportProvider;
use cascette_import::{ListfileProvider, TactKeysProvider};
use cascette_metadata::{HealthStatus, MetadataOrchestrator, OrchestratorConfig};
use std::error::Error;

/// Well-known FileDataIDs for demonstration.
const DEMO_IDS: &[(u32, &str)] = &[
    (1375801, "expected: Interface/Icons related"),
    (53187, "expected: Interface/GLUES related"),
    (2724755, "expected: World/Maps data"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Metadata Orchestrator ===\n");

    // Step 1: Set up cache directory
    let cache_dir = tempfile::tempdir()?;
    println!("Cache directory: {}\n", cache_dir.path().display());

    // Step 2: Initialize providers (async network I/O happens here)
    print!("Initializing ListfileProvider (downloads ~200 MB CSV)... ");
    let mut listfile = ListfileProvider::new(cache_dir.path().join("listfile"))?;
    listfile.initialize().await?;
    println!("done ({} mappings)", listfile.file_mappings().len());

    print!("Initializing TactKeysProvider... ");
    let mut tact_keys = TactKeysProvider::new(cache_dir.path().join("tactkeys"))?;
    tact_keys.initialize().await?;
    println!("done ({} keys)", tact_keys.get_all_tact_keys().len());
    println!();

    // Step 3: Build orchestrator (sync, reads in-memory data only)
    let orch = MetadataOrchestrator::from_providers(
        &listfile,
        &tact_keys,
        OrchestratorConfig {
            include_hardcoded_keys: true,
        },
    );

    // Step 4: Display statistics
    let stats = orch.stats();
    println!("Statistics:");
    println!("  FileDataID mappings: {}", stats.fdid_count);
    println!("  TACT keys:           {}", stats.tact_key_count);
    println!("  FDID ready:          {}", stats.fdid_ready);
    println!("  Keys ready:          {}", stats.keys_ready);
    println!();

    // Step 5: Resolve FileDataIDs
    println!("FileDataID resolution:");
    for &(id, hint) in DEMO_IDS {
        match orch.resolve_id(id) {
            Ok(path) => {
                let cat = orch.content_category(id)?;
                println!("  {id}: {path}  [{cat:?}]");
            }
            Err(_) => println!("  {id}: not found ({hint})"),
        }
    }
    println!();

    // Step 6: Reverse lookup
    println!("Reverse path lookup:");
    if let Ok(path) = orch.resolve_id(DEMO_IDS[0].0) {
        let path = path.to_string();
        match orch.resolve_path(&path) {
            Ok(id) => println!("  \"{path}\" -> FileDataID {id}"),
            Err(e) => println!("  \"{path}\" -> error: {e}"),
        }
    }
    println!();

    // Step 7: Health check
    let health = orch.health();
    match &health {
        HealthStatus::Healthy => println!("Health: healthy"),
        HealthStatus::Degraded { reason } => println!("Health: degraded ({reason})"),
    }

    println!("\nDone.");
    Ok(())
}
