//! Demonstrates WoW data discovery functionality
//!
//! This example shows how the test utilities locate WoW installation data
//! using environment variables and fallback paths.

use test_utils::{
    find_wow_data, find_any_wow_data, is_valid_wow_data, print_setup_instructions, WowVersion,
};

fn main() {
    println!("WoW Data Discovery Demo");
    println!("======================\n");

    // Try to find specific versions
    for &version in &[WowVersion::ClassicEra, WowVersion::Classic, WowVersion::Retail] {
        print!("Looking for {}... ", version.display_name());
        
        if let Some(path) = find_wow_data(version) {
            println!("✓ Found at: {}", path.display());
            
            // Validate the path
            if is_valid_wow_data(&path) {
                println!("  ✓ Data directory structure is valid");
            } else {
                println!("  ⚠ Data directory structure is invalid");
            }
        } else {
            println!("✗ Not found");
            println!("  Environment variable: {}", version.env_var());
        }
        println!();
    }

    // Try to find any available version
    println!("Looking for any WoW version...");
    match find_any_wow_data() {
        Some((version, path)) => {
            println!("✓ Found {} at: {}", version.display_name(), path.display());
        }
        None => {
            println!("✗ No WoW data found anywhere");
            println!("\nSetup instructions:");
            print_setup_instructions();
        }
    }
}