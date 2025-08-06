use tact_parser::tvfs::{TVFSManifest, VFSEntryType};
use tact_parser::utils::write_varint;

/// Create test TVFS data with minimal structure
fn create_minimal_tvfs_data() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(b"TVFS");         // Magic
    data.push(1);                             // Version
    data.push(38);                            // Header size
    data.push(0);                             // ESpec size (0 = no ESpec)
    
    // Path table offset and size (40-bit LE)
    let path_offset = 38u64; // Right after header
    data.extend_from_slice(&path_offset.to_le_bytes()[..5]);
    
    // Calculate actual path table size
    let path1 = b"test.txt";
    let path2 = b"folder/file.dat";
    let path_size = write_varint(path1.len() as u32).len() + path1.len() +
                    write_varint(path2.len() as u32).len() + path2.len();
    data.extend_from_slice(&(path_size as u64).to_le_bytes()[..5]);
    
    // VFS table offset and size
    let vfs_offset = path_offset + path_size as u64;
    data.extend_from_slice(&vfs_offset.to_le_bytes()[..5]);
    
    // Calculate actual VFS table size (2 file entries)
    let vfs_size = 1 + write_varint(0).len() + write_varint(1).len() + write_varint(0).len() + // Entry 1
                   1 + write_varint(1).len() + write_varint(1).len() + write_varint(1).len(); // Entry 2
    data.extend_from_slice(&(vfs_size as u64).to_le_bytes()[..5]);
    
    // CFT table offset and size
    let cft_offset = vfs_offset + vfs_size as u64;
    data.extend_from_slice(&cft_offset.to_le_bytes()[..5]);
    let cft_size = 42u64; // 2 entries * 21 bytes each
    data.extend_from_slice(&cft_size.to_le_bytes()[..5]);
    
    // Max path depth
    data.extend_from_slice(&10u16.to_le_bytes());
    
    // Flags (0 = no special features)
    data.push(0);
    
    // Path table
    // Entry 1: "test.txt"
    data.extend_from_slice(&write_varint(path1.len() as u32));
    data.extend_from_slice(path1);
    
    // Entry 2: "folder/file.dat"
    data.extend_from_slice(&write_varint(path2.len() as u32));
    data.extend_from_slice(path2);
    
    // VFS table
    // Entry 1: Regular file, 1 span
    data.push(0x00); // Type = FILE
    data.extend_from_slice(&write_varint(0)); // Span offset = 0
    data.extend_from_slice(&write_varint(1)); // Span count = 1
    data.extend_from_slice(&write_varint(0)); // Path index = 0
    
    // Entry 2: Regular file, 1 span
    data.push(0x00); // Type = FILE
    data.extend_from_slice(&write_varint(1)); // Span offset = 1
    data.extend_from_slice(&write_varint(1)); // Span count = 1
    data.extend_from_slice(&write_varint(1)); // Path index = 1
    
    // CFT table
    // Entry 1: EKey + size
    data.extend_from_slice(&[0xAA; 16]); // Dummy EKey
    data.extend_from_slice(&1024u64.to_le_bytes()[..5]); // File size
    
    // Entry 2: EKey + size
    data.extend_from_slice(&[0xBB; 16]); // Dummy EKey
    data.extend_from_slice(&2048u64.to_le_bytes()[..5]); // File size
    
    data
}

/// Create TVFS data with inline entries
fn create_tvfs_with_inline() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(b"TVFS");
    data.push(1);
    data.push(38);
    data.push(0);
    
    let path_offset = 38u64;
    data.extend_from_slice(&path_offset.to_le_bytes()[..5]);
    let path_size = 20u64;
    data.extend_from_slice(&path_size.to_le_bytes()[..5]);
    
    let vfs_offset = path_offset + path_size;
    data.extend_from_slice(&vfs_offset.to_le_bytes()[..5]);
    let vfs_size = 20u64;
    data.extend_from_slice(&vfs_size.to_le_bytes()[..5]);
    
    let cft_offset = vfs_offset + vfs_size;
    data.extend_from_slice(&cft_offset.to_le_bytes()[..5]);
    let cft_size = 0u64; // No CFT entries for inline
    data.extend_from_slice(&cft_size.to_le_bytes()[..5]);
    
    data.extend_from_slice(&5u16.to_le_bytes());
    data.push(0);
    
    // Path table
    let path = b"inline.txt";
    data.extend_from_slice(&write_varint(path.len() as u32));
    data.extend_from_slice(path);
    
    while data.len() < vfs_offset as usize {
        data.push(0);
    }
    
    // VFS table - inline entry
    data.push(0x02); // Type = INLINE
    data.extend_from_slice(&write_varint(0)); // Path index = 0
    
    // Inline data location
    let inline_offset = 200u64;
    data.extend_from_slice(&inline_offset.to_le_bytes()[..5]);
    let inline_size = 100u32;
    data.extend_from_slice(&inline_size.to_le_bytes());
    
    while data.len() < cft_offset as usize {
        data.push(0);
    }
    
    data
}

/// Create TVFS data with ESpec table
fn create_tvfs_with_espec() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(b"TVFS");
    data.push(1);
    data.push(48); // Larger header for ESpec offsets
    data.push(1);  // ESpec size = 1 byte per entry
    
    let path_offset = 48u64;
    data.extend_from_slice(&path_offset.to_le_bytes()[..5]);
    let path_size = 15u64;
    data.extend_from_slice(&path_size.to_le_bytes()[..5]);
    
    let vfs_offset = path_offset + path_size;
    data.extend_from_slice(&vfs_offset.to_le_bytes()[..5]);
    let vfs_size = 10u64;
    data.extend_from_slice(&vfs_size.to_le_bytes()[..5]);
    
    let cft_offset = vfs_offset + vfs_size;
    data.extend_from_slice(&cft_offset.to_le_bytes()[..5]);
    let cft_size = 22u64; // 1 entry with ESpec
    data.extend_from_slice(&cft_size.to_le_bytes()[..5]);
    
    data.extend_from_slice(&3u16.to_le_bytes());
    data.push(0x04); // Flags = has ESpec table
    
    // ESpec table offset and size
    let espec_offset = cft_offset + cft_size;
    data.extend_from_slice(&espec_offset.to_le_bytes()[..5]);
    let espec_size = 20u64;
    data.extend_from_slice(&espec_size.to_le_bytes()[..5]);
    
    // Path table
    let path = b"espec.dat";
    data.extend_from_slice(&write_varint(path.len() as u32));
    data.extend_from_slice(path);
    
    while data.len() < vfs_offset as usize {
        data.push(0);
    }
    
    // VFS table
    data.push(0x00); // Type = FILE
    data.extend_from_slice(&write_varint(0)); // Span offset = 0
    data.extend_from_slice(&write_varint(1)); // Span count = 1
    data.extend_from_slice(&write_varint(0)); // Path index = 0
    
    while data.len() < cft_offset as usize {
        data.push(0);
    }
    
    // CFT table with ESpec
    data.extend_from_slice(&[0xCC; 16]); // EKey
    data.extend_from_slice(&512u64.to_le_bytes()[..5]); // File size
    data.push(0); // ESpec index = 0
    
    while data.len() < espec_offset as usize {
        data.push(0);
    }
    
    // ESpec table
    let espec = b"compression:zlib";
    data.extend_from_slice(&write_varint(espec.len() as u32));
    data.extend_from_slice(espec);
    
    data
}

#[test]
fn test_parse_minimal_tvfs() {
    let data = create_minimal_tvfs_data();
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // Check header
    assert_eq!(tvfs.header.version, 1);
    assert_eq!(tvfs.header.espec_size, 0);
    assert!(!tvfs.header.has_write_support());
    assert!(!tvfs.header.has_patch_references());
    assert!(!tvfs.header.has_espec_table());
    
    // Debug output
    println!("Path table entries: {}", tvfs.path_table.len());
    for (i, entry) in tvfs.path_table.iter().enumerate() {
        println!("  Path {}: '{}'", i, entry.path);
    }
    
    // Check path table
    assert_eq!(tvfs.path_table.len(), 2);
    assert_eq!(tvfs.path_table[0].path, "test.txt");
    assert_eq!(tvfs.path_table[1].path, "folder/file.dat");
    
    // Check VFS table
    assert_eq!(tvfs.vfs_table.len(), 2);
    assert_eq!(tvfs.vfs_table[0].entry_type, VFSEntryType::File);
    assert_eq!(tvfs.vfs_table[0].span_count, 1);
    
    // Check CFT table
    assert_eq!(tvfs.cft_table.len(), 2);
    assert_eq!(tvfs.cft_table[0].file_size, 1024);
    assert_eq!(tvfs.cft_table[1].file_size, 2048);
    
    // Check file count
    assert_eq!(tvfs.file_count(), 2);
    assert_eq!(tvfs.total_size(), 3072);
}

#[test]
fn test_resolve_path() {
    let data = create_minimal_tvfs_data();
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // Resolve existing file
    let file_info = tvfs.resolve_path("test.txt");
    assert!(file_info.is_some());
    
    let info = file_info.unwrap();
    assert_eq!(info.path, "test.txt");
    assert_eq!(info.entry_type, VFSEntryType::File);
    assert_eq!(info.spans.len(), 1);
    assert_eq!(info.spans[0].file_size, 1024);
    
    // Try non-existent file
    assert!(tvfs.resolve_path("nonexistent.txt").is_none());
}

#[test]
fn test_list_directory() {
    let data = create_minimal_tvfs_data();
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // List root directory
    let entries = tvfs.list_directory("");
    assert_eq!(entries.len(), 1); // Only test.txt, folder/file.dat is in subfolder
    assert_eq!(entries[0].name, "test.txt");
    assert_eq!(entries[0].size, 1024);
    assert!(!entries[0].is_directory);
    
    // List folder directory
    let entries = tvfs.list_directory("folder");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "file.dat");
    assert_eq!(entries[0].size, 2048);
}

#[test]
fn test_inline_entry() {
    let data = create_tvfs_with_inline();
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // Check inline entry
    assert_eq!(tvfs.vfs_table.len(), 1);
    assert_eq!(tvfs.vfs_table[0].entry_type, VFSEntryType::Inline);
    assert_eq!(tvfs.vfs_table[0].file_offset, Some(200));
    assert_eq!(tvfs.vfs_table[0].file_size, Some(100));
    
    // Resolve inline file
    let file_info = tvfs.resolve_path("inline.txt");
    assert!(file_info.is_some());
    
    let info = file_info.unwrap();
    assert_eq!(info.entry_type, VFSEntryType::Inline);
    assert_eq!(info.inline_data, Some((200, 100)));
    assert!(info.spans.is_empty());
}

#[test]
fn test_espec_table() {
    let data = create_tvfs_with_espec();
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // Check header flags
    assert!(tvfs.header.has_espec_table());
    assert_eq!(tvfs.header.espec_size, 1);
    
    // Check ESpec table
    assert!(tvfs.espec_table.is_some());
    let espec_table = tvfs.espec_table.as_ref().unwrap();
    assert_eq!(espec_table.len(), 1);
    assert_eq!(espec_table[0], "compression:zlib");
    
    // Check CFT entry has ESpec reference
    assert_eq!(tvfs.cft_table[0].espec_index, Some(0));
    
    // Resolve file with ESpec
    let file_info = tvfs.resolve_path("espec.dat");
    assert!(file_info.is_some());
    
    let info = file_info.unwrap();
    assert_eq!(info.spans[0].espec, Some("compression:zlib".to_string()));
}

#[test]
fn test_empty_tvfs() {
    let mut data = Vec::new();
    
    // Minimal valid header
    data.extend_from_slice(b"TVFS");
    data.push(1);
    data.push(38);
    data.push(0);
    
    // All tables at end with 0 size
    let offset = 38u64;
    for _ in 0..3 {
        data.extend_from_slice(&offset.to_le_bytes()[..5]);
        data.extend_from_slice(&0u64.to_le_bytes()[..5]);
    }
    
    data.extend_from_slice(&0u16.to_le_bytes());
    data.push(0);
    
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    assert_eq!(tvfs.path_table.len(), 0);
    assert_eq!(tvfs.vfs_table.len(), 0);
    assert_eq!(tvfs.cft_table.len(), 0);
    assert_eq!(tvfs.file_count(), 0);
    assert_eq!(tvfs.total_size(), 0);
}

#[test]
fn test_multi_span_file() {
    let mut data = Vec::new();
    
    // Header
    data.extend_from_slice(b"TVFS");
    data.push(1);
    data.push(38);
    data.push(0);
    
    let path_offset = 38u64;
    data.extend_from_slice(&path_offset.to_le_bytes()[..5]);
    let path_size = 15u64;
    data.extend_from_slice(&path_size.to_le_bytes()[..5]);
    
    let vfs_offset = path_offset + path_size;
    data.extend_from_slice(&vfs_offset.to_le_bytes()[..5]);
    let vfs_size = 10u64;
    data.extend_from_slice(&vfs_size.to_le_bytes()[..5]);
    
    let cft_offset = vfs_offset + vfs_size;
    data.extend_from_slice(&cft_offset.to_le_bytes()[..5]);
    let cft_size = 63u64; // 3 entries
    data.extend_from_slice(&cft_size.to_le_bytes()[..5]);
    
    data.extend_from_slice(&5u16.to_le_bytes());
    data.push(0);
    
    // Path table
    let path = b"large.bin";
    data.extend_from_slice(&write_varint(path.len() as u32));
    data.extend_from_slice(path);
    
    while data.len() < vfs_offset as usize {
        data.push(0);
    }
    
    // VFS table - file with 3 spans
    data.push(0x00); // Type = FILE
    data.extend_from_slice(&write_varint(0)); // Span offset = 0
    data.extend_from_slice(&write_varint(3)); // Span count = 3
    data.extend_from_slice(&write_varint(0)); // Path index = 0
    
    while data.len() < cft_offset as usize {
        data.push(0);
    }
    
    // CFT table - 3 spans
    for i in 0..3 {
        data.extend_from_slice(&[0xDD + i; 16]); // Different EKeys
        let size = 1024 * (i as u64 + 1); // 1KB, 2KB, 3KB
        data.extend_from_slice(&size.to_le_bytes()[..5]);
    }
    
    let tvfs = TVFSManifest::parse(&data).unwrap();
    
    // Check multi-span file
    let file_info = tvfs.resolve_path("large.bin").unwrap();
    assert_eq!(file_info.spans.len(), 3);
    assert_eq!(file_info.spans[0].file_size, 1024);
    assert_eq!(file_info.spans[1].file_size, 2048);
    assert_eq!(file_info.spans[2].file_size, 3072);
    
    // Total size should be sum of all spans
    assert_eq!(tvfs.total_size(), 6144);
}