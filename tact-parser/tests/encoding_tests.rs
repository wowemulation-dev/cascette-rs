//! Integration tests for encoding file parsing

use tact_parser::encoding::EncodingFile;
use tact_parser::utils::write_uint40;

/// Helper to create a test encoding file with given entries
fn create_test_encoding_file(entries: Vec<(Vec<u8>, Vec<Vec<u8>>, u64)>) -> Vec<u8> {
    let mut data = Vec::new();
    
    // Magic "EN"
    data.extend_from_slice(&[0x45, 0x4E]);
    // Version 1
    data.push(1);
    // Hash sizes
    data.push(16); // CKey hash size
    data.push(16); // EKey hash size
    
    // Calculate page size needed (at least big enough for our entries)
    let entries_size = entries.iter()
        .map(|(ckey, ekeys, _size)| 1 + 5 + ckey.len() + ekeys.len() * ekeys[0].len())
        .sum::<usize>();
    let page_size_kb = ((entries_size + 1023) / 1024).max(1) as u16;
    
    // Page sizes (big-endian!)
    data.extend_from_slice(&page_size_kb.to_be_bytes()); // CKey page size
    data.extend_from_slice(&page_size_kb.to_be_bytes()); // EKey page size
    
    // Page counts (big-endian!)
    let page_count: u32 = if entries.is_empty() { 0 } else { 1 };
    data.extend_from_slice(&page_count.to_be_bytes()); // CKey page count
    data.extend_from_slice(&0u32.to_be_bytes()); // EKey page count (simplified)
    
    // Unknown field
    data.push(0);
    // ESpec block size (big-endian!)
    data.extend_from_slice(&0u32.to_be_bytes());
    
    if !entries.is_empty() {
        // CKey page table entry
        // First hash (just use zeros)
        data.extend_from_slice(&[0u8; 16]);
        // Checksum (simplified - just zeros)
        data.extend_from_slice(&[0u8; 16]);
        
        // Build CKey page
        let mut page_data = Vec::new();
        for (ckey, ekeys, size) in &entries {
            // Key count
            page_data.push(ekeys.len() as u8);
            // File size (40-bit)
            page_data.extend_from_slice(&write_uint40(*size));
            // CKey
            page_data.extend_from_slice(ckey);
            // EKeys
            for ekey in ekeys {
                page_data.extend_from_slice(ekey);
            }
        }
        
        // Pad to page size
        let page_size = page_size_kb as usize * 1024;
        page_data.resize(page_size, 0);
        
        data.extend_from_slice(&page_data);
    }
    
    data
}

#[test]
fn test_encoding_file_with_entries() {
    // Create test CKeys and EKeys
    let ckey1 = vec![1u8; 16];
    let ekey1 = vec![2u8; 16];
    let ekey2 = vec![3u8; 16];
    
    let ckey2 = vec![4u8; 16];
    let ekey3 = vec![5u8; 16];
    
    let entries = vec![
        (ckey1.clone(), vec![ekey1.clone(), ekey2.clone()], 1000),
        (ckey2.clone(), vec![ekey3.clone()], 2000),
    ];
    
    let data = create_test_encoding_file(entries);
    let encoding = EncodingFile::parse(&data).unwrap();
    
    // Test CKey → EKey lookup
    let entry1 = encoding.lookup_by_ckey(&ckey1).unwrap();
    assert_eq!(entry1.encoding_keys.len(), 2);
    assert_eq!(entry1.encoding_keys[0], ekey1);
    assert_eq!(entry1.encoding_keys[1], ekey2);
    assert_eq!(entry1.size, 1000);
    
    let entry2 = encoding.lookup_by_ckey(&ckey2).unwrap();
    assert_eq!(entry2.encoding_keys.len(), 1);
    assert_eq!(entry2.encoding_keys[0], ekey3);
    assert_eq!(entry2.size, 2000);
    
    // Test EKey → CKey reverse lookup
    assert_eq!(encoding.lookup_by_ekey(&ekey1), Some(&ckey1));
    assert_eq!(encoding.lookup_by_ekey(&ekey2), Some(&ckey1));
    assert_eq!(encoding.lookup_by_ekey(&ekey3), Some(&ckey2));
    
    // Test helper methods
    assert_eq!(encoding.get_ekey_for_ckey(&ckey1), Some(&ekey1));
    assert_eq!(encoding.get_file_size(&ckey1), Some(1000));
    assert_eq!(encoding.get_file_size(&ckey2), Some(2000));
    
    // Test counts
    assert_eq!(encoding.ckey_count(), 2);
    assert_eq!(encoding.ekey_count(), 3);
}

#[test]
fn test_encoding_file_large_sizes() {
    // Test 40-bit integer support with large file sizes
    let ckey = vec![0xAAu8; 16];
    let ekey = vec![0xBBu8; 16];
    let large_size = 0xFF_FFFF_FFFF; // Max 40-bit value
    
    let entries = vec![(ckey.clone(), vec![ekey.clone()], large_size)];
    let data = create_test_encoding_file(entries);
    let encoding = EncodingFile::parse(&data).unwrap();
    
    assert_eq!(encoding.get_file_size(&ckey), Some(large_size));
}

#[test]
fn test_encoding_file_empty_lookup() {
    let data = create_test_encoding_file(vec![]);
    let encoding = EncodingFile::parse(&data).unwrap();
    
    let nonexistent_key = vec![0xFFu8; 16];
    assert!(encoding.lookup_by_ckey(&nonexistent_key).is_none());
    assert!(encoding.lookup_by_ekey(&nonexistent_key).is_none());
    assert_eq!(encoding.get_ekey_for_ckey(&nonexistent_key), None);
    assert_eq!(encoding.get_file_size(&nonexistent_key), None);
}

#[test]
fn test_encoding_header_endianness() {
    // This test verifies that we're correctly handling big-endian values
    let mut data = Vec::new();
    
    // Magic "EN"
    data.extend_from_slice(&[0x45, 0x4E]);
    // Version 1
    data.push(1);
    // Hash sizes
    data.push(16);
    data.push(16);
    
    // These values should be read as big-endian
    // 0x1234 in big-endian = [0x12, 0x34]
    data.extend_from_slice(&[0x12, 0x34]); // CKey page size = 4660 KB
    data.extend_from_slice(&[0x56, 0x78]); // EKey page size = 22136 KB
    
    // Page counts set to 0 for header-only test
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // CKey page count = 0  
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // EKey page count = 0
    
    data.push(0); // Unknown
    data.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]); // ESpec block size
    
    let encoding = EncodingFile::parse(&data).unwrap();
    
    assert_eq!(encoding.header.ckey_page_size_kb, 0x1234);
    assert_eq!(encoding.header.ekey_page_size_kb, 0x5678);
    assert_eq!(encoding.header.ckey_page_count, 0);
    assert_eq!(encoding.header.ekey_page_count, 0);
    assert_eq!(encoding.header.espec_block_size, 0x11223344);
}

#[test]
fn test_multiple_ekeys_per_ckey() {
    // Some content can have multiple encoding keys (different compression methods)
    let ckey = vec![0x11u8; 16];
    let ekeys = vec![
        vec![0x21u8; 16],
        vec![0x22u8; 16],
        vec![0x23u8; 16],
        vec![0x24u8; 16],
    ];
    
    let entries = vec![(ckey.clone(), ekeys.clone(), 5000)];
    let data = create_test_encoding_file(entries);
    let encoding = EncodingFile::parse(&data).unwrap();
    
    let entry = encoding.lookup_by_ckey(&ckey).unwrap();
    assert_eq!(entry.encoding_keys.len(), 4);
    
    // All EKeys should map back to the same CKey
    for ekey in &ekeys {
        assert_eq!(encoding.lookup_by_ekey(ekey), Some(&ckey));
    }
    
    // First EKey should be returned by helper
    assert_eq!(encoding.get_ekey_for_ckey(&ckey), Some(&ekeys[0]));
}

#[test]
fn test_40bit_integer_in_encoding() {
    // Test various 40-bit integer values
    let test_sizes = vec![
        0,                  // Minimum
        0xFF,               // 1 byte max
        0xFFFF,             // 2 bytes max
        0xFFFFFF,           // 3 bytes max
        0xFFFFFFFF,         // 4 bytes max
        0x1234567890,       // Arbitrary 40-bit value
        0xFFFFFFFFFF,       // Maximum 40-bit value
    ];
    
    for (i, &size) in test_sizes.iter().enumerate() {
        let ckey = vec![i as u8; 16];
        let ekey = vec![(i + 100) as u8; 16];
        
        let entries = vec![(ckey.clone(), vec![ekey.clone()], size)];
        let data = create_test_encoding_file(entries);
        let encoding = EncodingFile::parse(&data).unwrap();
        
        assert_eq!(
            encoding.get_file_size(&ckey),
            Some(size),
            "Failed for size {:#x}",
            size
        );
    }
}