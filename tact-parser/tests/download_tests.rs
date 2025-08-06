use tact_parser::download::DownloadManifest;

/// Create a test download manifest with multiple priorities and tags
fn create_test_manifest_data() -> Vec<u8> {
    let mut data = Vec::new();

    // Header (v3)
    data.extend_from_slice(b"DL"); // Magic
    data.push(3); // Version
    data.push(16); // EKey size
    data.push(1); // Has checksum
    data.extend_from_slice(&5u32.to_be_bytes()); // Entry count
    data.extend_from_slice(&2u16.to_be_bytes()); // Tag count
    data.push(2); // Flag size
    data.push(0i8 as u8); // Base priority
    data.extend_from_slice(&[0, 0, 0]); // Unknown (24-bit)

    // Entry 1: Essential file (priority 0)
    data.extend_from_slice(&[1; 16]); // EKey
    data.extend_from_slice(&1000u64.to_le_bytes()[..5]); // Size (40-bit LE)
    data.push(0); // Priority (raw)
    data.extend_from_slice(&0x12345678u32.to_be_bytes()); // Checksum
    data.extend_from_slice(&[0xFF, 0x00]); // Flags

    // Entry 2: High priority file (priority 1)
    data.extend_from_slice(&[2; 16]); // EKey
    data.extend_from_slice(&2000u64.to_le_bytes()[..5]); // Size
    data.push(1); // Priority
    data.extend_from_slice(&0x23456789u32.to_be_bytes()); // Checksum
    data.extend_from_slice(&[0x00, 0xFF]); // Flags

    // Entry 3: Medium priority file (priority 2)
    data.extend_from_slice(&[3; 16]); // EKey
    data.extend_from_slice(&3000u64.to_le_bytes()[..5]); // Size
    data.push(2); // Priority
    data.extend_from_slice(&0x3456789Au32.to_be_bytes()); // Checksum
    data.extend_from_slice(&[0xAA, 0xBB]); // Flags

    // Entry 4: Low priority file (priority 3)
    data.extend_from_slice(&[4; 16]); // EKey
    data.extend_from_slice(&4000u64.to_le_bytes()[..5]); // Size
    data.push(3); // Priority
    data.extend_from_slice(&0x456789ABu32.to_be_bytes()); // Checksum
    data.extend_from_slice(&[0xCC, 0xDD]); // Flags

    // Entry 5: Optional file (priority 10)
    data.extend_from_slice(&[5; 16]); // EKey
    data.extend_from_slice(&5000u64.to_le_bytes()[..5]); // Size
    data.push(10); // Priority
    data.extend_from_slice(&0x56789ABCu32.to_be_bytes()); // Checksum
    data.extend_from_slice(&[0xEE, 0xFF]); // Flags

    // Tags
    // let _bytes_per_tag = (5 + 7) / 8; // 1 byte for 5 entries

    // Tag 1: "Windows"
    data.extend_from_slice(b"Windows\0");
    data.extend_from_slice(&2u16.to_be_bytes()); // Platform type
    data.push(0b11110000); // First 4 entries have this tag

    // Tag 2: "enUS"
    data.extend_from_slice(b"enUS\0");
    data.extend_from_slice(&1u16.to_be_bytes()); // Locale type
    data.push(0b10101000); // Entries 1, 3, 5 have this tag

    data
}

#[test]
fn test_parse_download_manifest() {
    let data = create_test_manifest_data();
    let manifest = DownloadManifest::parse(&data).unwrap();

    // Check header
    assert_eq!(manifest.header.version, 3);
    assert_eq!(manifest.header.entry_count, 5);
    assert_eq!(manifest.header.tag_count, 2);
    assert!(manifest.header.has_checksum);
    assert_eq!(manifest.header.flag_size, 2);

    // Check entries
    assert_eq!(manifest.entries.len(), 5);

    // Check priority order
    assert_eq!(manifest.priority_order.len(), 5);
    assert_eq!(manifest.priority_order[0], vec![1; 16]); // Priority 0
    assert_eq!(manifest.priority_order[1], vec![2; 16]); // Priority 1
    assert_eq!(manifest.priority_order[2], vec![3; 16]); // Priority 2
    assert_eq!(manifest.priority_order[3], vec![4; 16]); // Priority 3
    assert_eq!(manifest.priority_order[4], vec![5; 16]); // Priority 10

    // Check tags
    assert_eq!(manifest.tags.len(), 2);
    assert_eq!(manifest.tags[0].name, "Windows");
    assert_eq!(manifest.tags[0].tag_type, 2);
    assert_eq!(manifest.tags[1].name, "enUS");
    assert_eq!(manifest.tags[1].tag_type, 1);
}

#[test]
fn test_get_priority_files() {
    let data = create_test_manifest_data();
    let manifest = DownloadManifest::parse(&data).unwrap();

    // Get essential files only (priority 0)
    let essential = manifest.get_essential_files();
    assert_eq!(essential.len(), 1);
    assert_eq!(essential[0].priority, 0);

    // Get high priority files (priority <= 1)
    let high_priority = manifest.get_priority_files(1);
    assert_eq!(high_priority.len(), 2);

    // Get all normal priority files (priority <= 3)
    let normal = manifest.get_priority_files(3);
    assert_eq!(normal.len(), 4);

    // Get all files including optional (priority <= 10)
    let all = manifest.get_priority_files(10);
    assert_eq!(all.len(), 5);
}

#[test]
fn test_get_download_size() {
    let data = create_test_manifest_data();
    let manifest = DownloadManifest::parse(&data).unwrap();

    // Essential files only
    assert_eq!(manifest.get_download_size(0), 1000);

    // High priority
    assert_eq!(manifest.get_download_size(1), 3000); // 1000 + 2000

    // Normal priority
    assert_eq!(manifest.get_download_size(3), 10000); // 1000 + 2000 + 3000 + 4000

    // All files
    assert_eq!(manifest.get_download_size(10), 15000); // All 5 files
}

#[test]
fn test_get_files_for_tags() {
    let data = create_test_manifest_data();
    let manifest = DownloadManifest::parse(&data).unwrap();

    // Get Windows files
    let windows_files = manifest.get_files_for_tags(&["Windows"]);
    assert_eq!(windows_files.len(), 4); // First 4 entries

    // Get enUS files
    let enus_files = manifest.get_files_for_tags(&["enUS"]);
    assert_eq!(enus_files.len(), 3); // Entries 1, 3, 5

    // Get files with both tags
    let both_tags = manifest.get_files_for_tags(&["Windows", "enUS"]);
    assert_eq!(both_tags.len(), 5); // Union of both masks
}

#[test]
fn test_version_compatibility() {
    // Test v1 header (minimal)
    let mut data = Vec::new();
    data.extend_from_slice(b"DL");
    data.push(1); // Version 1
    data.push(16); // EKey size
    data.push(0); // No checksum
    data.extend_from_slice(&0u32.to_be_bytes()); // 0 entries
    data.extend_from_slice(&0u16.to_be_bytes()); // 0 tags

    let manifest = DownloadManifest::parse(&data).unwrap();
    assert_eq!(manifest.header.version, 1);
    assert_eq!(manifest.header.flag_size, 0);
    assert_eq!(manifest.header.base_priority, 0);
}

#[test]
fn test_entry_with_large_size() {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(b"DL");
    data.push(1);
    data.push(16);
    data.push(0);
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());

    // Entry with max 40-bit size
    data.extend_from_slice(&[0xAA; 16]);
    data.extend_from_slice(&0xFFFFFFFFFFu64.to_le_bytes()[..5]); // Max 40-bit value
    data.push(0);

    let manifest = DownloadManifest::parse(&data).unwrap();
    let entry = manifest.entries.get(&vec![0xAA; 16]).unwrap();
    assert_eq!(entry.compressed_size, 0xFFFFFFFFFF); // ~1TB
}

#[test]
fn test_negative_base_priority() {
    let mut data = Vec::new();

    // Header v3 with negative base priority
    data.extend_from_slice(b"DL");
    data.push(3);
    data.push(16);
    data.push(0);
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());
    data.push(0); // Flag size
    data.push(254u8); // -2 as i8
    data.extend_from_slice(&[0, 0, 0]);

    // Entry with raw priority 0
    data.extend_from_slice(&[1; 16]);
    data.extend_from_slice(&1000u64.to_le_bytes()[..5]);
    data.push(0); // Raw priority

    let manifest = DownloadManifest::parse(&data).unwrap();
    assert_eq!(manifest.header.base_priority, -2);

    let entry = manifest.entries.get(&vec![1; 16]).unwrap();
    assert_eq!(entry.priority, 0 - (-2)); // 0 - (-2) = 2
    assert_eq!(entry.priority, 2);
}
