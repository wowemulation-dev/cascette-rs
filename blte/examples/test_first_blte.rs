//! Test with the first BLTE extracted from archive

use blte::{BLTEFile, CompressionMode, compress_data_single, decompress_blte};
use std::fs;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_blte/first.blte";

    println!("Testing extracted BLTE file: {path}");
    let data = fs::read(path)?;
    println!("File size: {} bytes", data.len());

    // Parse structure
    let blte = BLTEFile::parse(data.clone())?;
    println!("Header size: {} bytes", blte.header.header_size);
    println!("Chunk count: {}", blte.chunk_count());

    if let Ok(chunk) = blte.get_chunk_data(0) {
        println!("\nFirst chunk:");
        println!("  Data size: {} bytes", chunk.data.len());
        println!("  Compressed size: {} bytes", chunk.compressed_size);
        println!("  Decompressed size: {} bytes", chunk.decompressed_size);
        println!("  Checksum (stored): {:02x?}", chunk.checksum);

        // Calculate actual checksum
        let actual_checksum = md5::compute(&chunk.data);
        println!("  Checksum (actual): {:02x?}", actual_checksum.0);

        if !chunk.data.is_empty() {
            println!("  First byte: 0x{:02x}", chunk.data[0]);

            // The issue is the chunk data starts with 0x5A which is 'Z'
            // So it's ZLib compressed data
            if chunk.data[0] == b'Z' {
                println!("  ✓ Detected ZLib compression");

                // Try to decompress just this chunk
                println!("\nAttempting chunk decompression...");
                match blte::decompress_chunk(&chunk.data, 0, None) {
                    Ok(decompressed) => {
                        println!("  ✓ Chunk decompressed: {} bytes", decompressed.len());

                        // Now try full round-trip
                        println!("\n=== Full Round-Trip Test ===");

                        // Decompress entire BLTE
                        let start = Instant::now();
                        match decompress_blte(data.clone(), None) {
                            Ok(decompressed) => {
                                let decompress_time = start.elapsed();
                                println!(
                                    "Decompressed: {} bytes in {:?}",
                                    decompressed.len(),
                                    decompress_time
                                );

                                // Recompress
                                let start = Instant::now();
                                let recompressed = compress_data_single(
                                    decompressed.clone(),
                                    CompressionMode::ZLib,
                                    None,
                                )?;
                                let compress_time = start.elapsed();
                                println!(
                                    "Recompressed: {} bytes in {:?}",
                                    recompressed.len(),
                                    compress_time
                                );

                                // Verify
                                match decompress_blte(recompressed.clone(), None) {
                                    Ok(re_decompressed) => {
                                        if re_decompressed == decompressed {
                                            println!("✓ Round-trip successful!");
                                        } else {
                                            println!("✗ Round-trip failed: size mismatch");
                                        }
                                    }
                                    Err(e) => {
                                        println!("✗ Re-decompression failed: {e}");
                                    }
                                }
                            }
                            Err(e) => {
                                println!("✗ Decompression failed: {e}");

                                // Debug: Let's check what's happening
                                println!("\nDebug info:");
                                println!("  Chunk compressed size: {}", chunk.compressed_size);
                                println!("  Chunk data actual size: {}", chunk.data.len());
                                println!("  Expected checksum: {:02x?}", chunk.checksum);
                                println!("  Actual checksum: {:02x?}", actual_checksum.0);

                                // The checksums don't match - this suggests the chunk boundaries are wrong
                                // or the data is being read incorrectly
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Chunk decompression failed: {e}");
                    }
                }
            }
        }
    }

    Ok(())
}
