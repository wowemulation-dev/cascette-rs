//! List files in a CASC storage using discovered WoW installation
//!
//! This example demonstrates how to use the test-utils crate to discover
//! WoW installations and work with real CASC data.

use casc_storage::{CascStorage, types::CascConfig};
use test_utils::{find_any_wow_data, print_setup_instructions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("CASC File Listing Example");
    println!("========================\n");

    // Try to find any WoW installation
    let (version, data_path) = match find_any_wow_data() {
        Some(found) => found,
        None => {
            println!("No WoW installation found.");
            print_setup_instructions();
            return Ok(());
        }
    };

    println!("Using {} data from:", version.display_name());
    println!("  {}\n", data_path.display());

    // Create CASC storage
    let config = CascConfig {
        data_path,
        read_only: true,
        ..Default::default()
    };

    println!("Loading CASC storage...");
    let storage = CascStorage::new(config)?;

    // Load indices
    println!("Loading indices...");
    storage.load_indices()?;

    // Load archives
    println!("Loading archives...");
    storage.load_archives()?;

    println!("✓ CASC storage loaded successfully");

    println!("✓ Example completed - CASC storage is ready for use");

    Ok(())
}
