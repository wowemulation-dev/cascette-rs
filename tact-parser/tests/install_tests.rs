//! Integration tests for install manifest parsing

use tact_parser::install::{InstallManifest, Platform};

/// Helper to create a test install manifest
fn create_test_install_manifest(
    tags: Vec<(&str, u16, Vec<bool>)>,
    entries: Vec<(&str, Vec<u8>, u32)>,
) -> Vec<u8> {
    let mut data = Vec::new();

    // Magic "IN"
    data.extend_from_slice(&[0x49, 0x4E]);
    // Version 1
    data.push(1);
    // Hash size
    data.push(16);
    // Tag count (big-endian!)
    data.extend_from_slice(&(tags.len() as u16).to_be_bytes());
    // Entry count (big-endian!)
    data.extend_from_slice(&(entries.len() as u32).to_be_bytes());

    // Tags
    let bytes_per_tag = entries.len().div_ceil(8);
    for (name, tag_type, mask) in &tags {
        // Tag name (null-terminated)
        data.extend_from_slice(name.as_bytes());
        data.push(0);
        // Tag type (big-endian!)
        data.extend_from_slice(&(*tag_type).to_be_bytes());
        // Bitmask
        let mut mask_bytes = vec![0u8; bytes_per_tag];
        for (i, &has_tag) in mask.iter().enumerate() {
            if has_tag {
                mask_bytes[i / 8] |= 1 << (i % 8);
            }
        }
        data.extend_from_slice(&mask_bytes);
    }

    // Entries
    for (path, ckey, size) in &entries {
        // Path (null-terminated)
        data.extend_from_slice(path.as_bytes());
        data.push(0);
        // CKey
        data.extend_from_slice(ckey);
        // Size (big-endian!)
        data.extend_from_slice(&size.to_be_bytes());
    }

    data
}

#[test]
fn test_multi_platform_install() {
    // Create a manifest with Windows, Mac, and shared files
    let tags = vec![
        ("Windows", 0x01, vec![true, false, true, true]), // Files 0, 2, 3
        ("OSX", 0x02, vec![false, true, true, false]),    // Files 1, 2
        ("enUS", 0x04, vec![true, true, true, true]),     // All files
    ];

    let entries = vec![
        ("game.exe", vec![1u8; 16], 1000),        // Windows only
        ("game.app", vec![2u8; 16], 2000),        // Mac only
        ("data/shared.dat", vec![3u8; 16], 3000), // Both platforms
        ("win64.dll", vec![4u8; 16], 4000),       // Windows only
    ];

    let data = create_test_install_manifest(tags, entries);
    let manifest = InstallManifest::parse(&data).unwrap();

    // Test platform filtering
    let windows_files = manifest.get_files_for_platform(Platform::Windows);
    assert_eq!(windows_files.len(), 3);
    assert!(windows_files.iter().any(|f| f.path == "game.exe"));
    assert!(windows_files.iter().any(|f| f.path == "data/shared.dat"));
    assert!(windows_files.iter().any(|f| f.path == "win64.dll"));

    let mac_files = manifest.get_files_for_platform(Platform::Mac);
    assert_eq!(mac_files.len(), 2);
    assert!(mac_files.iter().any(|f| f.path == "game.app"));
    assert!(mac_files.iter().any(|f| f.path == "data/shared.dat"));

    // Test size calculations
    assert_eq!(
        manifest.calculate_size_for_platform(Platform::Windows),
        8000
    ); // 1000 + 3000 + 4000
    assert_eq!(manifest.calculate_size_for_platform(Platform::Mac), 5000); // 2000 + 3000
    assert_eq!(manifest.calculate_size_for_platform(Platform::All), 10000); // All files

    // Test tag combinations
    let windows_enus = manifest.get_files_for_tags(&["Windows", "enUS"]);
    assert_eq!(windows_enus.len(), 3); // Windows files that also have enUS

    // Test getting all tags
    let all_tags = manifest.get_all_tags();
    assert!(all_tags.contains(&"Windows"));
    assert!(all_tags.contains(&"OSX"));
    assert!(all_tags.contains(&"enUS"));
}

#[test]
fn test_locale_specific_files() {
    // Create a manifest with locale-specific files
    let tags = vec![
        ("enUS", 0x01, vec![true, false, false, true]),
        ("deDE", 0x02, vec![false, true, false, true]),
        ("frFR", 0x04, vec![false, false, true, true]),
        ("Common", 0x08, vec![false, false, false, true]),
    ];

    let entries = vec![
        ("locale/enUS/strings.db", vec![1u8; 16], 100),
        ("locale/deDE/strings.db", vec![2u8; 16], 110),
        ("locale/frFR/strings.db", vec![3u8; 16], 120),
        ("data/common.dat", vec![4u8; 16], 5000),
    ];

    let data = create_test_install_manifest(tags, entries);
    let manifest = InstallManifest::parse(&data).unwrap();

    // Test locale-specific queries
    let enus_files = manifest.get_files_for_tags(&["enUS"]);
    assert_eq!(enus_files.len(), 2);
    assert!(
        enus_files
            .iter()
            .any(|f| f.path == "locale/enUS/strings.db")
    );
    assert!(enus_files.iter().any(|f| f.path == "data/common.dat"));

    let dede_files = manifest.get_files_for_tags(&["deDE"]);
    assert_eq!(dede_files.len(), 2);
    assert!(
        dede_files
            .iter()
            .any(|f| f.path == "locale/deDE/strings.db")
    );

    // Calculate size for specific locale
    assert_eq!(manifest.calculate_size_for_tags(&["enUS"]), 5100);
    assert_eq!(manifest.calculate_size_for_tags(&["deDE"]), 5110);
    assert_eq!(manifest.calculate_size_for_tags(&["frFR"]), 5120);

    // Common files only
    let common_only = manifest.get_files_for_tags(&["Common"]);
    assert_eq!(common_only.len(), 1);
    assert_eq!(common_only[0].path, "data/common.dat");
}

#[test]
fn test_get_file_by_path() {
    let tags = vec![("All", 0x01, vec![true, true, true])];

    let entries = vec![
        ("file1.dat", vec![1u8; 16], 1000),
        ("dir/file2.dat", vec![2u8; 16], 2000),
        ("dir/subdir/file3.dat", vec![3u8; 16], 3000),
    ];

    let data = create_test_install_manifest(tags, entries);
    let manifest = InstallManifest::parse(&data).unwrap();

    // Test path lookup
    let file1 = manifest.get_file_by_path("file1.dat").unwrap();
    assert_eq!(file1.size, 1000);
    assert_eq!(file1.ckey, vec![1u8; 16]);

    let file2 = manifest.get_file_by_path("dir/file2.dat").unwrap();
    assert_eq!(file2.size, 2000);

    let file3 = manifest.get_file_by_path("dir/subdir/file3.dat").unwrap();
    assert_eq!(file3.size, 3000);

    // Test nonexistent file
    assert!(manifest.get_file_by_path("nonexistent.dat").is_none());
}

#[test]
fn test_complex_tag_combinations() {
    // Test with many overlapping tags
    let tags = vec![
        ("A", 0x01, vec![true, true, false, false]),
        ("B", 0x02, vec![true, false, true, false]),
        ("C", 0x04, vec![true, false, false, true]),
        ("D", 0x08, vec![false, true, true, true]),
    ];

    let entries = vec![
        ("file_abc.dat", vec![1u8; 16], 1000), // Tags: A, B, C
        ("file_ad.dat", vec![2u8; 16], 2000),  // Tags: A, D
        ("file_bd.dat", vec![3u8; 16], 3000),  // Tags: B, D
        ("file_cd.dat", vec![4u8; 16], 4000),  // Tags: C, D
    ];

    let data = create_test_install_manifest(tags, entries);
    let manifest = InstallManifest::parse(&data).unwrap();

    // Test single tag queries
    assert_eq!(manifest.get_files_for_tags(&["A"]).len(), 2);
    assert_eq!(manifest.get_files_for_tags(&["B"]).len(), 2);
    assert_eq!(manifest.get_files_for_tags(&["C"]).len(), 2);
    assert_eq!(manifest.get_files_for_tags(&["D"]).len(), 3);

    // Test multiple tag requirements (AND logic)
    assert_eq!(manifest.get_files_for_tags(&["A", "B"]).len(), 1); // Only file_abc.dat
    assert_eq!(manifest.get_files_for_tags(&["A", "C"]).len(), 1); // Only file_abc.dat
    assert_eq!(manifest.get_files_for_tags(&["B", "D"]).len(), 1); // Only file_bd.dat
    assert_eq!(manifest.get_files_for_tags(&["C", "D"]).len(), 1); // Only file_cd.dat

    // Test impossible combinations
    assert_eq!(manifest.get_files_for_tags(&["A", "B", "D"]).len(), 0);
}

#[test]
fn test_large_manifest() {
    // Test with many files to ensure bitmask calculation works correctly
    let num_files = 100;
    let mut tag_mask = vec![false; num_files];

    // Mark every other file
    for i in (0..num_files).step_by(2) {
        tag_mask[i] = true;
    }

    let tags = vec![("EvenFiles", 0x01, tag_mask.clone())];

    let mut owned_entries = Vec::new();
    for i in 0..num_files {
        let path = format!("file_{i:03}.dat");
        let ckey = vec![i as u8; 16];
        let size = (i * 100) as u32;
        owned_entries.push((path, ckey, size));
    }

    // Create the manifest data
    let mut data = Vec::new();
    data.extend_from_slice(&[0x49, 0x4E]);
    data.push(1);
    data.push(16);
    data.extend_from_slice(&(tags.len() as u16).to_be_bytes());
    data.extend_from_slice(&(num_files as u32).to_be_bytes());

    // Tags
    let bytes_per_tag = num_files.div_ceil(8);
    for (name, tag_type, mask) in &tags {
        data.extend_from_slice(name.as_bytes());
        data.push(0);
        let tag_type_u16: u16 = *tag_type;
        data.extend_from_slice(&tag_type_u16.to_be_bytes());
        let mut mask_bytes = vec![0u8; bytes_per_tag];
        for (i, &has_tag) in mask.iter().enumerate() {
            if has_tag {
                mask_bytes[i / 8] |= 1 << (i % 8);
            }
        }
        data.extend_from_slice(&mask_bytes);
    }

    // Entries
    for (path, ckey, size) in &owned_entries {
        data.extend_from_slice(path.as_bytes());
        data.push(0);
        data.extend_from_slice(ckey);
        data.extend_from_slice(&size.to_be_bytes());
    }

    let manifest = InstallManifest::parse(&data).unwrap();

    // Check that we have the right number of entries
    assert_eq!(manifest.entries.len(), num_files);

    // Check that exactly half have the tag
    let even_files = manifest.get_files_for_tags(&["EvenFiles"]);
    assert_eq!(even_files.len(), num_files / 2);

    // Verify specific files
    for i in 0..num_files {
        let path = format!("file_{i:03}.dat");
        let file = manifest.get_file_by_path(&path).unwrap();
        assert_eq!(file.size, (i * 100) as u32);

        if i % 2 == 0 {
            assert!(file.tags.contains(&"EvenFiles".to_string()));
        } else {
            assert!(!file.tags.contains(&"EvenFiles".to_string()));
        }
    }
}
