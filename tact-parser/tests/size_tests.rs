use tact_parser::size::SizeFile;

/// Create test size file data
fn create_test_size_data() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(b"DS"); // Magic
    data.push(1); // Version
    data.push(9); // EKey size (partial MD5)
    data.extend_from_slice(&5u32.to_be_bytes()); // Entry count
    data.extend_from_slice(&2u16.to_be_bytes()); // Tag count

    // Total size (15000 as 40-bit LE)
    let total_size = 15000u64;
    data.extend_from_slice(&total_size.to_le_bytes()[..5]);

    // Tags (come before entries in size file)
    // let _bytes_per_tag = (5 + 7) / 8; // 1 byte for 5 entries

    // Tag 1: "Windows"
    data.extend_from_slice(b"Windows\0");
    data.extend_from_slice(&2u16.to_be_bytes()); // Platform type
    data.push(0b11110000); // First 4 entries

    // Tag 2: "enUS"
    data.extend_from_slice(b"enUS\0");
    data.extend_from_slice(&1u16.to_be_bytes()); // Locale type
    data.push(0b10101000); // Entries 1, 3, 5

    // Entries (5 total)
    // Entry 1: 5000 bytes
    data.extend_from_slice(&[0xA1; 9]); // Partial EKey
    data.extend_from_slice(&5000u32.to_be_bytes()); // Size

    // Entry 2: 4000 bytes
    data.extend_from_slice(&[0xA2; 9]);
    data.extend_from_slice(&4000u32.to_be_bytes());

    // Entry 3: 3000 bytes
    data.extend_from_slice(&[0xA3; 9]);
    data.extend_from_slice(&3000u32.to_be_bytes());

    // Entry 4: 2000 bytes
    data.extend_from_slice(&[0xA4; 9]);
    data.extend_from_slice(&2000u32.to_be_bytes());

    // Entry 5: 1000 bytes
    data.extend_from_slice(&[0xA5; 9]);
    data.extend_from_slice(&1000u32.to_be_bytes());

    data
}

#[test]
fn test_parse_size_file() {
    let data = create_test_size_data();
    let size_file = SizeFile::parse(&data).unwrap();

    // Check header
    assert_eq!(size_file.header.version, 1);
    assert_eq!(size_file.header.ekey_size, 9);
    assert_eq!(size_file.header.entry_count, 5);
    assert_eq!(size_file.header.tag_count, 2);
    assert_eq!(size_file.header.total_size, 15000);

    // Check entries
    assert_eq!(size_file.entries.len(), 5);

    // Check total size matches
    assert_eq!(size_file.get_total_size(), 15000);

    // Check size ordering (largest first)
    assert_eq!(size_file.size_order[0], vec![0xA1; 9]); // 5000 bytes
    assert_eq!(size_file.size_order[1], vec![0xA2; 9]); // 4000 bytes
    assert_eq!(size_file.size_order[2], vec![0xA3; 9]); // 3000 bytes
    assert_eq!(size_file.size_order[3], vec![0xA4; 9]); // 2000 bytes
    assert_eq!(size_file.size_order[4], vec![0xA5; 9]); // 1000 bytes
}

#[test]
fn test_get_file_size() {
    let data = create_test_size_data();
    let size_file = SizeFile::parse(&data).unwrap();

    // Check individual file sizes
    assert_eq!(size_file.get_file_size(&[0xA1; 9]), Some(5000));
    assert_eq!(size_file.get_file_size(&[0xA2; 9]), Some(4000));
    assert_eq!(size_file.get_file_size(&[0xA3; 9]), Some(3000));
    assert_eq!(size_file.get_file_size(&[0xA4; 9]), Some(2000));
    assert_eq!(size_file.get_file_size(&[0xA5; 9]), Some(1000));

    // Test with full MD5 (16 bytes) - should truncate to 9
    let full_md5 = vec![0xA1; 16];
    assert_eq!(size_file.get_file_size(&full_md5), Some(5000));

    // Test non-existent key
    assert_eq!(size_file.get_file_size(&[0xFF; 9]), None);
}

#[test]
fn test_get_largest_files() {
    let data = create_test_size_data();
    let size_file = SizeFile::parse(&data).unwrap();

    // Get top 3 largest files
    let largest = size_file.get_largest_files(3);
    assert_eq!(largest.len(), 3);
    assert_eq!(largest[0].1, 5000);
    assert_eq!(largest[1].1, 4000);
    assert_eq!(largest[2].1, 3000);

    // Get all files
    let all = size_file.get_largest_files(10);
    assert_eq!(all.len(), 5);
}

#[test]
fn test_get_statistics() {
    let data = create_test_size_data();
    let size_file = SizeFile::parse(&data).unwrap();

    let stats = size_file.get_statistics();

    assert_eq!(stats.total_size, 15000);
    assert_eq!(stats.file_count, 5);
    assert_eq!(stats.average_size, 3000); // 15000 / 5
    assert_eq!(stats.min_size, 1000);
    assert_eq!(stats.max_size, 5000);
}

#[test]
fn test_get_size_for_tags() {
    let data = create_test_size_data();
    let size_file = SizeFile::parse(&data).unwrap();

    // Windows tag: first 4 entries (5000 + 4000 + 3000 + 2000)
    let windows_size = size_file.get_size_for_tags(&["Windows"]);
    assert_eq!(windows_size, 14000);

    // enUS tag: entries 1, 3, 5 (5000 + 3000 + 1000)
    let enus_size = size_file.get_size_for_tags(&["enUS"]);
    assert_eq!(enus_size, 9000);

    // Both tags (union): all 5 entries
    let both_size = size_file.get_size_for_tags(&["Windows", "enUS"]);
    assert_eq!(both_size, 15000);
}

#[test]
fn test_empty_size_file() {
    let mut data = Vec::new();

    // Minimal header
    data.extend_from_slice(b"DS");
    data.push(1);
    data.push(9);
    data.extend_from_slice(&0u32.to_be_bytes()); // 0 entries
    data.extend_from_slice(&0u16.to_be_bytes()); // 0 tags
    data.extend_from_slice(&0u64.to_le_bytes()[..5]); // 0 total size

    let size_file = SizeFile::parse(&data).unwrap();

    assert_eq!(size_file.header.entry_count, 0);
    assert_eq!(size_file.entries.len(), 0);
    assert_eq!(size_file.get_total_size(), 0);

    let stats = size_file.get_statistics();
    assert_eq!(stats.file_count, 0);
    assert_eq!(stats.total_size, 0);
}

#[test]
fn test_large_file_sizes() {
    let mut data = Vec::new();

    // Header with 1 entry
    data.extend_from_slice(b"DS");
    data.push(1);
    data.push(9);
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());

    // Total size: max 40-bit value
    let max_size = 0xFFFFFFFFFFu64;
    data.extend_from_slice(&max_size.to_le_bytes()[..5]);

    // Single entry with max 32-bit size
    data.extend_from_slice(&[0xBB; 9]);
    data.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());

    let size_file = SizeFile::parse(&data).unwrap();

    assert_eq!(size_file.get_total_size(), 0xFFFFFFFFFF);
    assert_eq!(size_file.get_file_size(&[0xBB; 9]), Some(0xFFFFFFFF));
}
