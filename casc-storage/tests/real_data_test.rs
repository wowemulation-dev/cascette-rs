//! Tests with real WoW installation data

use casc_storage::{CascStorage, types::CascConfig};
use std::path::PathBuf;

#[test]
#[ignore = "requires real WoW installation"]
fn test_load_wow_classic_indices() {
    // Test with WoW Classic 1.13.2 installation
    let config = CascConfig {
        data_path: PathBuf::from(
            "/home/danielsreichenbach/Downloads/wow/1.13.2.31650.windows-win64/Data",
        ),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new(config).unwrap();

    // Try to load indices
    let result = storage.load_indices();
    assert!(result.is_ok(), "Failed to load indices: {result:?}");

    // Try to load archives
    let result = storage.load_archives();
    assert!(result.is_ok(), "Failed to load archives: {result:?}");
}

#[test]
#[ignore = "requires real WoW installation"]
fn test_load_wow_classic_era_indices() {
    // Test with WoW Classic Era 1.14.2 installation
    let config = CascConfig {
        data_path: PathBuf::from(
            "/home/danielsreichenbach/Downloads/wow/1.14.2.42597.windows-win64/Data",
        ),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new(config).unwrap();

    // Try to load indices
    let result = storage.load_indices();
    assert!(result.is_ok(), "Failed to load indices: {result:?}");

    // Try to load archives
    let result = storage.load_archives();
    assert!(result.is_ok(), "Failed to load archives: {result:?}");
}

#[test]
#[ignore = "requires real WoW installation and functional BLTE"]
fn test_read_file_from_wow() {
    // This test will only work once we have BLTE decompression
    let config = CascConfig {
        data_path: PathBuf::from(
            "/home/danielsreichenbach/Downloads/wow/1.13.2.31650.windows-win64/Data",
        ),
        read_only: true,
        ..Default::default()
    };

    let storage = CascStorage::new(config).unwrap();
    storage.load_indices().unwrap();
    storage.load_archives().unwrap();

    // Try to read a known file (would need a real EKey from the installation)
    // This is a placeholder test that will work once BLTE is implemented
}
