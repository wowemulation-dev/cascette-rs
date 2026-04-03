#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Maintenance verification against a local WoW installation.
//!
//! Runs a dry-run maintenance pass on a local CASC installation to verify
//! preservation, garbage collection, and repair analysis.
//!
//! Environment variables:
//!   CASCETTE_WOW_PATH  Path to a local WoW installation root (required)
//!
//! Usage:
//!   CASCETTE_WOW_PATH=~/Downloads/wow_classic/1.13.2.31650.windows-win64 \
//!     cargo run -p cascette-maintenance --example maintenance_verification \
//!       --features local-install

mod common;

use cascette_maintenance::ExecutionMode;

#[tokio::main]
async fn main() {
    let wow = common::wow_path();
    let data_path = wow.join("Data");

    // E1: Verify local installation (dry-run)
    println!("=== E1: Maintenance dry-run ===");
    let install =
        cascette_client_storage::Installation::open(data_path).expect("installation open");
    install.initialize().await.expect("initialization");

    let report = cascette_maintenance::run_maintenance(&install, ExecutionMode::DryRun)
        .await
        .expect("maintenance dry-run should succeed");

    if let Some(ref pres) = report.preservation {
        println!(
            "  preservation: {} keys in preservation set",
            pres.key_count
        );
    }

    if let Some(ref gc) = report.gc {
        println!(
            "  gc dry-run: {} orphaned segments, {} bytes freeable",
            gc.segments_orphaned, gc.bytes_freed,
        );
    }

    if let Some(ref repair) = report.repair {
        println!(
            "  repair dry-run: {} entries verified, {} corrupted",
            repair.entries_verified, repair.entries_corrupted,
        );
    }

    println!("  completed in {:?}", report.total_duration);

    println!("\nAll maintenance checks passed.");
}
