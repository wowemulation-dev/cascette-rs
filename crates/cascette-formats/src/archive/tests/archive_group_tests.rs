//! Tests for archive-group parsing and building
//!
//! Archive-groups use 6-byte offsets: 2 bytes for archive index + 4 bytes for offset

use crate::archive::{ArchiveIndexBuilder, IndexEntry};
use std::io::Cursor;

#[test]
fn test_archive_group_detection() {
    let mut builder = ArchiveIndexBuilder::new();

    // Create an entry with archive index
    let key = vec![
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10,
    ];
    builder.add_entry(key.clone(), 1000, 0x1234_5678);

    let mut output = Vec::new();
    let index = builder
        .build(&mut Cursor::new(&mut output))
        .expect("Failed to build index");

    // Regular index should not be detected as archive-group
    assert!(!index.is_archive_group());
    assert_eq!(index.footer.offset_bytes, 4);
}

#[test]
fn test_archive_group_6byte_offset_parsing() {
    // Test parsing a 6-byte offset field
    let data = vec![
        // Key (16 bytes)
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, // Size (4 bytes, big-endian)
        0x00, 0x00, 0x10, 0x00, // Archive index (2 bytes) + Offset (4 bytes)
        0x00, 0x42, // Archive index: 66
        0x12, 0x34, 0x56, 0x78, // Offset: 0x1234_5678
    ];

    let entry = IndexEntry::parse(&data, 16, 4, 6).expect("Failed to parse entry");

    assert_eq!(entry.encoding_key.len(), 16);
    assert_eq!(entry.size, 0x1000);
    assert_eq!(entry.archive_index, Some(0x42));
    assert_eq!(entry.offset, 0x1234_5678);
}

#[test]
fn test_archive_group_entry_creation() {
    let key = vec![0xAA; 16];
    let entry = IndexEntry::new_archive_group(
        key.clone(),
        5000,        // size
        42,          // archive index
        0xDEAD_BEEF, // offset
    );

    assert_eq!(entry.encoding_key, key);
    assert_eq!(entry.size, 5000);
    assert_eq!(entry.archive_index, Some(42));
    assert_eq!(entry.offset, 0xDEAD_BEEF);
}

#[test]
fn test_archive_group_entry_to_bytes() {
    let key = vec![0xFF; 16];
    let entry = IndexEntry::new_archive_group(
        key,
        0x1234,      // size
        0x0100,      // archive index 256
        0xABCD_EF00, // offset
    );

    let bytes = entry.to_bytes(4, 6).expect("Failed to convert to bytes");

    // Check the serialized format
    assert_eq!(bytes.len(), 26); // 16 key + 4 size + 6 offset

    // Check archive index (bytes 20-21)
    assert_eq!(bytes[20], 0x01);
    assert_eq!(bytes[21], 0x00);

    // Check offset (bytes 22-25)
    assert_eq!(bytes[22], 0xAB);
    assert_eq!(bytes[23], 0xCD);
    assert_eq!(bytes[24], 0xEF);
    assert_eq!(bytes[25], 0x00);
}

#[test]
fn test_archive_group_round_trip() {
    let key = vec![
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88,
    ];

    let original = IndexEntry::new_archive_group(
        key,
        0x1234_5678, // size
        0x4242,      // archive index
        0x8765_4321, // offset
    );

    // Serialize
    let bytes = original.to_bytes(4, 6).expect("Failed to serialize");

    // Parse back
    let parsed = IndexEntry::parse(&bytes, 16, 4, 6).expect("Failed to parse");

    // Verify round-trip
    assert_eq!(original.encoding_key, parsed.encoding_key);
    assert_eq!(original.size, parsed.size);
    assert_eq!(original.archive_index, parsed.archive_index);
    assert_eq!(original.offset, parsed.offset);
}

#[test]
fn test_archive_group_max_values() {
    // Test with maximum archive index and offset
    let entry = IndexEntry::new_archive_group(
        vec![0xEE; 16],
        u32::MAX, // max size
        u16::MAX, // max archive index (65535)
        u32::MAX, // max offset
    );

    let bytes = entry.to_bytes(4, 6).expect("Failed to convert to bytes");
    let parsed = IndexEntry::parse(&bytes, 16, 4, 6).expect("Failed to parse");

    assert_eq!(parsed.archive_index, Some(u16::MAX));
    assert_eq!(parsed.offset, u32::MAX as u64);
    assert_eq!(parsed.size, u32::MAX);
}

#[test]
fn test_archive_group_hash_distribution() {
    // Simulate the hash-based archive index assignment used by Battle.net
    use std::collections::HashMap;

    let mut distribution: HashMap<u16, usize> = HashMap::new();

    // Generate many encoding keys and compute their archive indices
    for i in 0u32..10000 {
        let mut key = vec![0u8; 16];
        key[0..4].copy_from_slice(&i.to_be_bytes());

        // Simulate hash-based assignment (simplified)
        let hash = cascette_crypto::md5::ContentKey::from_data(&key);
        let archive_idx = u16::from_be_bytes([hash.as_bytes()[0], hash.as_bytes()[1]]);

        *distribution.entry(archive_idx).or_insert(0) += 1;
    }

    // Check that we're using a wide range of indices (not just 0-605)
    let unique_indices = distribution.len();
    assert!(
        unique_indices > 1000,
        "Should use many different archive indices, got {}",
        unique_indices
    );

    // Check archive 0 gets roughly expected percentage (around 1/256 ≈ 0.39%)
    let archive_0_count = distribution.get(&0).copied().unwrap_or(0);
    let archive_0_pct = (archive_0_count as f64 / 10000.0) * 100.0;
    assert!(
        archive_0_pct < 2.0,
        "Archive 0 should get < 2% of entries, got {}%",
        archive_0_pct
    );
}

#[test]
fn test_differentiate_regular_vs_archive_group() {
    // Test that we can differentiate between regular indices and archive-groups

    // Regular index entry (4-byte offset)
    let regular_data = vec![
        // Key (16 bytes)
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, // Size (4 bytes)
        0x00, 0x00, 0x10, 0x00, // Offset (4 bytes)
        0x12, 0x34, 0x56, 0x78,
    ];

    let regular =
        IndexEntry::parse(&regular_data, 16, 4, 4).expect("Failed to parse regular entry");
    assert_eq!(regular.archive_index, None);
    assert_eq!(regular.offset, 0x1234_5678);

    // Archive-group entry (6-byte offset)
    let archive_group_data = vec![
        // Key (16 bytes)
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, // Size (4 bytes)
        0x00, 0x00, 0x10, 0x00, // Archive index (2 bytes) + Offset (4 bytes)
        0x00, 0x2A, // Archive index: 42
        0x12, 0x34, 0x56, 0x78, // Offset
    ];

    let archive_group = IndexEntry::parse(&archive_group_data, 16, 4, 6)
        .expect("Failed to parse archive-group entry");
    assert_eq!(archive_group.archive_index, Some(42));
    assert_eq!(archive_group.offset, 0x1234_5678);
}
