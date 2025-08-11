//! Integration tests with real WoW installation data
//!
//! These tests use environment variables to locate WoW installations.
//! See test-utils crate documentation for setup instructions.

use casc_storage::{CascStorage, types::CascConfig};
use test_utils::{WowVersion, require_wow_data};

#[test]
fn test_load_wow_classic_era_indices() {
    let data_path = require_wow_data!(WowVersion::ClassicEra);

    println!(
        "Testing with WoW Classic Era data at: {}",
        data_path.display()
    );

    let config = CascConfig {
        data_path,
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new(config).expect("Failed to create CASC storage");

    // Try to load indices
    let result = storage.load_indices();
    assert!(result.is_ok(), "Failed to load indices: {result:?}");

    // Try to load archives
    let result = storage.load_archives();
    assert!(result.is_ok(), "Failed to load archives: {result:?}");

    println!("✓ Successfully loaded CASC storage for Classic Era");
}

#[test]
fn test_load_any_wow_version() {
    use test_utils::{find_any_wow_data, skip_test_if_no_wow_data};

    skip_test_if_no_wow_data!();

    let (version, data_path) = find_any_wow_data().expect("No WoW data found");

    println!(
        "Testing with {} data at: {}",
        version.display_name(),
        data_path.display()
    );

    let config = CascConfig {
        data_path,
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new(config).expect("Failed to create CASC storage");

    // Verify basic CASC operations work
    let result = storage.load_indices();
    assert!(result.is_ok(), "Failed to load indices: {result:?}");

    println!(
        "✓ Successfully loaded CASC storage for {}",
        version.display_name()
    );
}

#[test]
fn test_skip_behavior_documented() {
    // This test documents the skip behavior without actually testing it
    // (since testing the skip would make the test fail in CI)

    println!("Skip behavior: Tests use require_wow_data!() macro");
    println!("When no data is found, tests will return early with helpful message");

    // Just verify our utility functions work
    let classic_era = WowVersion::ClassicEra;
    assert_eq!(classic_era.env_var(), "WOW_CLASSIC_ERA_DATA");
    assert!(classic_era.display_name().contains("Classic Era"));
}
