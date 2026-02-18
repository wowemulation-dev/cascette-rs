#![allow(clippy::expect_used, clippy::panic)]

//! Dump `.build.info` from a local WoW installation.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example dump_build_info \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::BuildInfoFile;

#[tokio::main]
async fn main() {
    let wow = common::wow_path();
    let info_path = wow.join(".build.info");

    println!("Reading: {}", info_path.display());
    let info = BuildInfoFile::from_path(&info_path)
        .await
        .expect("failed to parse .build.info");

    println!("Entries: {}\n", info.entry_count());

    // Active entry details
    if let Some(active) = info.active_entry() {
        println!("=== Active Entry ===");
        println!("  Branch:     {}", active.branch().unwrap_or("-"));
        println!("  Product:    {}", active.product().unwrap_or("-"));
        println!("  Version:    {}", active.version().unwrap_or("-"));
        println!("  Build Key:  {}", active.build_key().unwrap_or("-"));
        println!("  CDN Key:    {}", active.cdn_key().unwrap_or("-"));
        println!("  Install Key:{}", active.install_key().unwrap_or("-"));
        if let Some(size) = active.install_size() {
            println!("  IM Size:    {size}");
        }
        println!("  CDN Path:   {}", active.cdn_path().unwrap_or("-"));
        let hosts = active.cdn_hosts();
        if !hosts.is_empty() {
            println!("  CDN Hosts:");
            for h in &hosts {
                println!("    - {h}");
            }
        }
        let servers = active.cdn_servers();
        if !servers.is_empty() {
            println!("  CDN Servers:");
            for s in &servers {
                println!("    - {s}");
            }
        }
        println!("  Tags:       {}", active.tags().unwrap_or("-"));
        println!("  Armadillo:  {}", active.armadillo().unwrap_or("-"));
        println!(
            "  Activated:  {}",
            active.last_activated().unwrap_or("-")
        );
        println!();
    } else {
        println!("No active entry found.\n");
    }

    // All entries table
    println!("=== All Entries ===");
    for (i, entry) in info.entries().iter().enumerate() {
        let active_marker = if entry.is_active() { "*" } else { " " };
        println!(
            "  [{i}]{active_marker} branch={:<6} product={:<12} version={:<16} build_key={}",
            entry.branch().unwrap_or("-"),
            entry.product().unwrap_or("-"),
            entry.version().unwrap_or("-"),
            entry.build_key().unwrap_or("-"),
        );
    }
}
