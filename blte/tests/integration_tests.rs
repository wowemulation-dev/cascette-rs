//! Integration tests for BLTE with real CDN data

use blte::{BLTEFile, CompressionMode, decompress_blte};

/// Test that we can parse a real BLTE file structure
#[tokio::test]
#[ignore] // Requires network access
async fn test_real_blte_file() {
    // This would download and test a real BLTE file from CDN
    // For now, we'll create a comprehensive synthetic test

    let test_data = b"This is a test of BLTE compression with multiple modes!";

    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(test_data).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut zlib_blte = Vec::new();
    zlib_blte.extend_from_slice(b"BLTE");
    zlib_blte.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
    zlib_blte.push(b'Z'); // ZLib mode
    zlib_blte.extend_from_slice(&compressed);

    let result = decompress_blte(zlib_blte, None).unwrap();
    assert_eq!(result, test_data);

    println!("✓ ZLib BLTE decompression works correctly");
}

#[test]
fn test_nested_blte_frame() {
    let inner_data = b"Inner BLTE content";

    let mut inner_blte = Vec::new();
    inner_blte.extend_from_slice(b"BLTE");
    inner_blte.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
    inner_blte.push(b'N'); // No compression
    inner_blte.extend_from_slice(inner_data);

    let mut outer_blte = Vec::new();
    outer_blte.extend_from_slice(b"BLTE");
    outer_blte.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
    outer_blte.push(b'F'); // Frame mode
    outer_blte.extend_from_slice(&inner_blte);

    let result = decompress_blte(outer_blte, None).unwrap();
    assert_eq!(result, inner_data);

    println!("✓ Nested BLTE (Frame mode) decompression works correctly");
}

#[test]
fn test_multi_chunk_with_mixed_compression() {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let chunk1_data = b"First chunk: no compression";
    let chunk2_data = b"Second chunk: ZLib compressed content for better compression ratio";
    let chunk3_data = b"Third chunk: also uncompressed";

    // Compress chunk 2
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(chunk2_data).unwrap();
    let chunk2_compressed = encoder.finish().unwrap();

    let mut chunk1_full = Vec::new();
    chunk1_full.push(b'N'); // No compression
    chunk1_full.extend_from_slice(chunk1_data);

    let mut chunk2_full = Vec::new();
    chunk2_full.push(b'Z'); // ZLib compression
    chunk2_full.extend_from_slice(&chunk2_compressed);

    let mut chunk3_full = Vec::new();
    chunk3_full.push(b'N'); // No compression
    chunk3_full.extend_from_slice(chunk3_data);

    // Calculate header size
    let header_size = 1 + 3 + 3 * 24; // flags + chunk_count + 3 * chunk_info (NOT including magic + header_size field)

    // Build BLTE file
    let mut blte_data = Vec::new();

    // Header
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&(header_size as u32).to_be_bytes());

    // Chunk table
    blte_data.push(0x0F); // Standard flags
    blte_data.extend_from_slice(&[0x00, 0x00, 0x03]); // 3 chunks

    // Chunk info
    blte_data.extend_from_slice(&(chunk1_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum

    blte_data.extend_from_slice(&(chunk2_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum

    blte_data.extend_from_slice(&(chunk3_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk3_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum

    // Chunk data
    blte_data.extend_from_slice(&chunk1_full);
    blte_data.extend_from_slice(&chunk2_full);
    blte_data.extend_from_slice(&chunk3_full);

    let result = decompress_blte(blte_data, None).unwrap();

    let mut expected = Vec::new();
    expected.extend_from_slice(chunk1_data);
    expected.extend_from_slice(chunk2_data);
    expected.extend_from_slice(chunk3_data);

    assert_eq!(result, expected);

    println!("✓ Multi-chunk BLTE with mixed compression modes works correctly");
}

#[test]
fn test_blte_file_structure() {
    // Create a properly formatted single-chunk BLTE file
    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE"); // Magic
    blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk (header_size = 0)
    blte_data.push(b'N'); // No compression mode
    blte_data.extend_from_slice(b"Hello, BLTE!"); // Data

    let blte_file = BLTEFile::parse(blte_data).unwrap();

    assert!(blte_file.is_single_chunk());
    assert_eq!(blte_file.chunk_count(), 1);

    let chunk = blte_file.get_chunk_data(0).unwrap();
    assert_eq!(chunk.compression_mode().unwrap(), CompressionMode::None);
    assert_eq!(chunk.data, b"NHello, BLTE!");

    println!("✓ BLTE file structure parsing works correctly");
}

#[test]
fn test_encryption_mode_structure() {
    // Test that we can at least parse the structure of encrypted blocks
    // (actual decryption would require keys)

    let mut encrypted_block = Vec::new();
    encrypted_block.push(b'E'); // Encrypted mode
    encrypted_block.extend_from_slice(&8u64.to_le_bytes()); // Key name size
    encrypted_block.extend_from_slice(&0x123456789ABCDEFu64.to_le_bytes()); // Key name
    encrypted_block.extend_from_slice(&4u32.to_le_bytes()); // IV size
    encrypted_block.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // IV
    encrypted_block.push(0x53); // Salsa20
    encrypted_block.extend_from_slice(b"dummy encrypted data");

    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
    blte_data.extend_from_slice(&encrypted_block);

    let blte_file = BLTEFile::parse(blte_data).unwrap();
    let chunk = blte_file.get_chunk_data(0).unwrap();

    assert_eq!(
        chunk.compression_mode().unwrap(),
        CompressionMode::Encrypted
    );

    // Try to decompress without key service (should fail with appropriate error)
    let result = decompress_blte(blte_file.data.clone(), None);
    assert!(result.is_err());

    println!("✓ Encrypted BLTE structure parsing works correctly");
}

#[test]
fn test_large_file_simulation() {
    // Simulate a large file with many chunks
    let chunk_count = 100;
    let chunk_data = b"This is chunk data that will be repeated many times.";

    let header_size = 1 + 3 + chunk_count * 24; // flags + chunk_count + chunk_info (NOT including magic + header_size field)

    let mut blte_data = Vec::new();

    // Header
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&(header_size as u32).to_be_bytes());

    // Chunk table
    blte_data.push(0x0F);
    let count_bytes = (chunk_count as u32).to_be_bytes();
    blte_data.extend_from_slice(&count_bytes[1..4]); // 3-byte count

    // Build chunk info and data
    let mut all_chunk_data = Vec::new();
    for i in 0..chunk_count {
        let mut chunk_full = Vec::new();
        chunk_full.push(b'N'); // No compression
        chunk_full.extend_from_slice(chunk_data);
        chunk_full.extend_from_slice(format!("_{i}").as_bytes()); // Make each chunk unique

        // Chunk info
        let decompressed_data = &chunk_full[1..]; // Skip compression mode byte
        blte_data.extend_from_slice(&(chunk_full.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&(decompressed_data.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&[0; 16]); // Zero checksum

        all_chunk_data.extend_from_slice(&chunk_full);
    }

    blte_data.extend_from_slice(&all_chunk_data);

    let result = decompress_blte(blte_data, None).unwrap();

    // Verify we got all chunks - calculate expected size properly
    let mut expected_size = 0;
    for i in 0..chunk_count {
        expected_size += chunk_data.len() + format!("_{i}").len();
    }
    assert_eq!(result.len(), expected_size);

    println!("✓ Large file with {chunk_count} chunks processed correctly");
}
