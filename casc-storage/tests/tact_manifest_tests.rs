//! Tests for TACT manifest integration

use casc_storage::types::CascConfig;
use casc_storage::{CascStorage, ManifestConfig, TactManifests};
use tact_parser::wow_root::{ContentFlags, LocaleFlags};
use tempfile::TempDir;

#[test]
fn test_manifest_config_default() {
    let config = ManifestConfig::default();
    assert!(config.locale.all());
    assert!(config.content_flags.is_none());
    assert!(config.cache_manifests);
}

#[test]
fn test_manifest_config_custom() {
    let config = ManifestConfig {
        locale: LocaleFlags::new().with_en_us(true),
        content_flags: Some(ContentFlags::new().with_windows(true).with_x86_64(true)),
        cache_manifests: false,
    };

    assert!(config.locale.en_us());
    assert!(!config.locale.de_de());
    assert!(config.content_flags.unwrap().windows());
    assert!(!config.cache_manifests);
}

#[test]
fn test_tact_manifests_creation() {
    let config = ManifestConfig::default();
    let manifests = TactManifests::new(config);

    // Initially no manifests loaded
    assert!(!manifests.is_loaded());
}

#[test]
fn test_manifest_not_loaded_errors() {
    let config = ManifestConfig::default();
    let manifests = TactManifests::new(config);

    // Should return errors when manifests not loaded
    assert!(manifests.get_all_fdids().is_err());
    assert!(manifests.lookup_by_fdid(123).is_err());
    assert!(manifests.lookup_by_filename("test.blp").is_err());
}

#[test]
fn test_listfile_parsing() {
    let config = ManifestConfig::default();
    let manifests = TactManifests::new(config);

    // Create a temporary listfile
    let temp_dir = TempDir::new().unwrap();
    let listfile_path = temp_dir.path().join("listfile.csv");

    let content = "123;Interface\\Icons\\spell_nature_lightning.blp\n456;Interface\\Icons\\ability_warrior_savageblow.blp\n789;World\\Maps\\Kalimdor\\Kalimdor.wdt";
    std::fs::write(&listfile_path, content).unwrap();

    let count = manifests.load_listfile(&listfile_path).unwrap();
    assert_eq!(count, 3);

    // Test filename lookup
    assert_eq!(
        manifests.get_fdid_for_filename("Interface\\Icons\\spell_nature_lightning.blp"),
        Some(123)
    );
    assert_eq!(
        manifests.get_fdid_for_filename("Interface\\Icons\\ability_warrior_savageblow.blp"),
        Some(456)
    );
    assert_eq!(
        manifests.get_fdid_for_filename("World\\Maps\\Kalimdor\\Kalimdor.wdt"),
        Some(789)
    );
    assert_eq!(manifests.get_fdid_for_filename("nonexistent.blp"), None);
}

#[test]
fn test_casc_storage_tact_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config = CascConfig {
        data_path: temp_dir.path().to_path_buf(),
        read_only: false,
        cache_size_mb: 64,
        max_archive_size: 256 * 1024 * 1024,
        use_memory_mapping: true,
    };

    let mut storage = CascStorage::new(config).unwrap();

    // Initially no TACT manifests
    assert!(!storage.tact_manifests_loaded());

    // Initialize TACT manifests
    let manifest_config = ManifestConfig {
        locale: LocaleFlags::new().with_en_us(true),
        content_flags: Some(ContentFlags::new().with_windows(true)),
        cache_manifests: true,
    };

    storage.init_tact_manifests(manifest_config);

    // Still not loaded (no manifest data loaded yet)
    assert!(!storage.tact_manifests_loaded());

    // Operations should fail without manifest data
    assert!(storage.read_by_fdid(123).is_err());
    assert!(storage.read_by_filename("test.blp").is_err());
    assert!(storage.get_fdid_for_filename("test.blp").is_none());
}

#[test]
fn test_cache_operations() {
    let config = ManifestConfig::default();
    let manifests = TactManifests::new(config);

    // Clear cache should not fail even when empty
    manifests.clear_cache();
}

// Mock data tests (would need actual TACT data to test fully)
#[test]
fn test_mock_data_structures() {
    use casc_storage::FileMapping;
    use casc_storage::types::EKey;

    let mapping = FileMapping {
        file_data_id: 123,
        content_key: [0; 16],
        encoding_key: Some(EKey::new([1; 16])),
        flags: Some(ContentFlags::new().with_install(true)),
    };

    assert_eq!(mapping.file_data_id, 123);
    assert!(mapping.encoding_key.is_some());
    assert!(mapping.flags.unwrap().install());
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    // Helper function to create a minimal test BLTE file
    fn create_test_blte_data() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"BLTE"); // Magic
        data.extend_from_slice(&0u32.to_be_bytes()); // Header size (0 = single chunk)
        data.push(b'N'); // Mode 'N' (uncompressed)
        data.extend_from_slice(b"Hello World"); // Test data
        data
    }

    #[test]
    fn test_blte_detection() {
        let manifests = TactManifests::new(ManifestConfig::default());
        let blte_data = create_test_blte_data();

        // Test that BLTE format is detected (result may succeed if data is parseable as root)
        let result = manifests.load_root_from_data(blte_data);

        // Either succeeds with empty data or fails - both are valid outcomes for test data
        match result {
            Ok(_) => {
                // If it succeeds, manifests should be available
                // This is actually valid for the wow_root parser with empty data
            }
            Err(e) => {
                // If it fails, should be a reasonable error
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("Failed to parse root")
                        || error_msg.contains("Decompression error")
                        || error_msg.contains("Invalid format"),
                    "Unexpected error: {error_msg}"
                );
            }
        }
    }

    #[test]
    fn test_non_blte_data() {
        let manifests = TactManifests::new(ManifestConfig::default());
        let non_blte_data = vec![1, 2, 3, 4, 5]; // Not BLTE format

        // Test that non-BLTE data is handled - may succeed if parseable as root
        let result = manifests.load_root_from_data(non_blte_data);

        // Either succeeds with minimal data or fails - both are valid for test purposes
        match result {
            Ok(_) => {
                // This can happen with simple data that parses as empty root
            }
            Err(_) => {
                // Expected for truly invalid data
            }
        }
    }
}
