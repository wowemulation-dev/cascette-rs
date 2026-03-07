//! Maintenance analysis for a local WoW Classic installation.
//!
//! Runs all four maintenance operations in dry-run mode and prints
//! a report of what would happen. Shows both index-only and manifest-aware
//! report fields when available.
//!
//! Usage:
//! ```sh
//! CASCETTE_WOW_PATH=/path/to/wow cargo run --example maintenance_analysis \
//!     -p cascette-maintenance --features local-install
//! ```

mod common;

use cascette_client_storage::Installation;
use cascette_maintenance::{ExecutionMode, run_maintenance};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wow_path = common::wow_path();
    let data_root = wow_path.join("Data");

    println!("=== CASC Maintenance Analysis ===");
    println!("Installation: {}\n", data_root.display());

    let installation = Installation::open(data_root)?;
    installation.initialize().await?;

    let stats = installation.stats().await;
    println!("Index entries: {}", stats.index_entries);
    println!("Archive files: {}", stats.archive_files);
    println!("Archive size:  {} bytes\n", stats.archive_size);

    let report = run_maintenance(&installation, ExecutionMode::DryRun).await?;

    if let Some(pres) = &report.preservation {
        println!("--- Preservation Set ---");
        println!("  Keys preserved:   {}", pres.key_count);
        println!("  Source entries:    {}", pres.source_index_entries);
        println!("  Manifest keys:    {}", pres.manifest_keys);
        println!("  Duration:         {:?}\n", pres.duration);
    }

    if let Some(gc) = &report.gc {
        println!("--- Garbage Collection (dry-run) ---");
        println!("  Entries scanned:     {}", gc.entries_scanned);
        println!("  Would remove:        {}", gc.entries_removed);
        println!("  Orphaned segments:   {}", gc.segments_orphaned);
        println!("  Bytes freeable:      {}", gc.bytes_freed);
        println!("  Config files stale:  {}", gc.config_files_removed);
        println!("  CDN indices stale:   {}\n", gc.cdn_indices_removed);
    }

    if let Some(comp) = &report.compaction {
        println!("--- Compaction (dry-run) ---");
        println!("  Segments analyzed:   {}", comp.segments_analyzed);
        println!("  Would compact:       {}", comp.segments_compacted);
        println!("  Defrag segments:     {}", comp.defrag_segments);
        println!("  Fillholes segments:  {}", comp.fillholes_segments);
        println!("  Merged segments:     {}", comp.merged_segments);
        println!("  Moves planned:       {}", comp.moves_planned);
        println!("  Bytes reclaimable:   {}\n", comp.bytes_reclaimed);
    }

    if let Some(rep) = &report.repair {
        println!("--- Build Repair (dry-run) ---");
        println!("  Entries verified:    {}", rep.entries_verified);
        println!("  Valid:               {}", rep.entries_valid);
        println!("  Corrupted:           {}", rep.entries_corrupted);
        println!("  Re-downloaded:       {}", rep.entries_redownloaded);
        println!("  Redownload failed:   {}", rep.redownload_failed);
        println!("  Loose files checked: {}", rep.loose_files_checked);
        println!("  Loose files repaired:{}", rep.loose_files_repaired);
        println!("  Markers written:     {}", rep.markers_written);
        println!("  Duration:            {:?}\n", rep.duration);
    }

    println!("Total duration: {:?}", report.total_duration);
    Ok(())
}
