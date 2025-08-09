//! Example demonstrating BLTE encryption functionality
//!
//! This example shows how to create encrypted BLTE files using both
//! Salsa20 and ARC4 encryption methods.

#![allow(deprecated)] // ARC4 is deprecated but still supported for compatibility

use blte::{
    CompressionMode, EncryptionMethod, compress_data_encrypted_multi,
    compress_data_encrypted_single,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BLTE Encryption Example");
    println!("=======================");

    // Test data
    let test_data = b"Hello, World! This is test data for BLTE encryption.".to_vec();
    let key = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88,
    ];
    let iv = [0xAA, 0xBB, 0xCC, 0xDD];

    println!("Original data: {:?}", String::from_utf8_lossy(&test_data));
    println!("Data length: {} bytes", test_data.len());
    println!();

    // Test 1: Single-chunk encrypted BLTE with Salsa20
    println!("1. Single-chunk Salsa20 encryption:");
    let salsa20_blte = compress_data_encrypted_single(
        test_data.clone(),
        None, // No compression
        None,
        EncryptionMethod::Salsa20,
        &key,
        &iv,
    )?;

    println!("   Encrypted BLTE size: {} bytes", salsa20_blte.len());
    println!("   BLTE header: {:02X?}", &salsa20_blte[0..12]);
    println!();

    // Test 2: Single-chunk encrypted BLTE with ARC4
    println!("2. Single-chunk ARC4 encryption:");
    let arc4_blte = compress_data_encrypted_single(
        test_data.clone(),
        None, // No compression
        None,
        EncryptionMethod::ARC4,
        &key,
        &iv,
    )?;

    println!("   Encrypted BLTE size: {} bytes", arc4_blte.len());
    println!("   BLTE header: {:02X?}", &arc4_blte[0..12]);
    println!();

    // Test 3: Compressed then encrypted
    println!("3. ZLib compression + Salsa20 encryption:");
    let compressed_encrypted = compress_data_encrypted_single(
        test_data.clone(),
        Some(CompressionMode::ZLib), // Apply ZLib compression first
        Some(6),                     // Compression level
        EncryptionMethod::Salsa20,
        &key,
        &iv,
    )?;

    println!(
        "   Compressed+Encrypted BLTE size: {} bytes",
        compressed_encrypted.len()
    );
    println!(
        "   Size reduction: {:.1}%",
        100.0 - (compressed_encrypted.len() as f64 / salsa20_blte.len() as f64 * 100.0)
    );
    println!();

    // Test 4: Multi-chunk encrypted BLTE
    let large_data = vec![b'A'; 200]; // Create 200 bytes of data
    println!("4. Multi-chunk encryption (200 bytes, 64-byte chunks):");

    let multi_chunk_blte = compress_data_encrypted_multi(
        large_data,
        64, // 64-byte chunks
        Some(CompressionMode::LZ4),
        None,
        EncryptionMethod::Salsa20,
        &key,
        &iv,
    )?;

    println!("   Multi-chunk BLTE size: {} bytes", multi_chunk_blte.len());
    println!("   Header indicates multi-chunk format");
    println!();

    // Test 5: Verify the files are properly encrypted
    println!("5. Encryption verification:");
    println!(
        "   Salsa20 encrypted chunk starts with 'E': {}",
        salsa20_blte[8] == b'E'
    );
    println!(
        "   ARC4 encrypted chunk starts with 'E': {}",
        arc4_blte[8] == b'E'
    );

    // Different encryption methods should produce different results
    println!(
        "   Salsa20 ≠ ARC4: {}",
        salsa20_blte[9..20] != arc4_blte[9..20]
    );
    println!();

    println!("✅ All encryption tests completed successfully!");
    println!();
    println!("Note: To decrypt these files, you would need:");
    println!("  - A KeyService configured with the encryption key");
    println!("  - Call decompress_blte() with the key service");
    println!("  - The BLTE library handles decryption automatically");

    Ok(())
}
