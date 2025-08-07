//! Test compression and checksum calculation

use blte::compress::compress_chunk;
use blte::decompress::decompress_blte;
use blte::{BLTEBuilder, CompressionMode};

fn main() {
    println!("Testing BLTE compression and checksums...\n");

    // Test 1: Single chunk compression
    let data = b"Test data".to_vec();
    let compressed = compress_chunk(&data, CompressionMode::None, None).unwrap();
    println!("Single chunk compressed:");
    println!("  Data: {data:?}");
    println!("  Compressed (with mode): {compressed:?}");
    println!("  Checksum of compressed: {:x}", md5::compute(&compressed));
    println!();

    // Test 2: Build single chunk BLTE
    let blte = BLTEBuilder::new()
        .with_compression(CompressionMode::None)
        .add_data(data.clone())
        .build_single()
        .unwrap();

    println!("Single BLTE file:");
    println!("  BLTE size: {}", blte.len());
    println!("  BLTE header: {:?}", &blte[0..8]);
    println!("  BLTE data: {:?}", &blte[8..20.min(blte.len())]);
    println!();

    // Test 3: Multi-chunk
    let chunk1 = b"First".to_vec();
    let chunk2 = b"Second".to_vec();

    let compressed1 = compress_chunk(&chunk1, CompressionMode::None, None).unwrap();
    let compressed2 = compress_chunk(&chunk2, CompressionMode::None, None).unwrap();

    println!("Multi-chunk compressed:");
    println!("  Chunk 1: {chunk1:?} -> {compressed1:?}");
    println!("  Checksum 1: {:x}", md5::compute(&compressed1));
    println!("  Chunk 2: {chunk2:?} -> {compressed2:?}");
    println!("  Checksum 2: {:x}", md5::compute(&compressed2));
    println!();

    // Build multi-chunk BLTE
    let multi_blte = BLTEBuilder::new()
        .with_compression(CompressionMode::None)
        .add_data(chunk1.clone())
        .add_data(chunk2.clone())
        .build_multi()
        .unwrap();

    println!("Multi-chunk BLTE:");
    println!("  Total size: {}", multi_blte.len());
    println!(
        "  Header size: {}",
        u32::from_be_bytes([multi_blte[4], multi_blte[5], multi_blte[6], multi_blte[7]])
    );

    // Try to decompress
    match decompress_blte(multi_blte.clone(), None) {
        Ok(decompressed) => {
            println!("  Decompressed: {decompressed:?}");
            println!("  Success!");
        }
        Err(e) => {
            println!("  Decompression failed: {e}");

            // Debug: show the actual BLTE structure
            println!("\nDebug - BLTE structure:");
            println!("  Magic: {:?}", &multi_blte[0..4]);
            println!("  Header size: {:?}", &multi_blte[4..8]);
            let header_size =
                u32::from_be_bytes([multi_blte[4], multi_blte[5], multi_blte[6], multi_blte[7]])
                    as usize;
            if header_size > 0 {
                println!("  Flags: 0x{:02x}", multi_blte[8]);
                println!(
                    "  Chunk count: {}",
                    (multi_blte[9] as u32) << 16
                        | (multi_blte[10] as u32) << 8
                        | (multi_blte[11] as u32)
                );

                // Show chunk table
                let mut offset = 12;
                let chunk_count = (multi_blte[9] as u32) << 16
                    | (multi_blte[10] as u32) << 8
                    | (multi_blte[11] as u32);
                for i in 0..chunk_count {
                    let comp_size = u32::from_be_bytes([
                        multi_blte[offset],
                        multi_blte[offset + 1],
                        multi_blte[offset + 2],
                        multi_blte[offset + 3],
                    ]);
                    let decomp_size = u32::from_be_bytes([
                        multi_blte[offset + 4],
                        multi_blte[offset + 5],
                        multi_blte[offset + 6],
                        multi_blte[offset + 7],
                    ]);
                    let checksum = &multi_blte[offset + 8..offset + 24];
                    println!(
                        "  Chunk {}: comp_size={}, decomp_size={}, checksum={:x}",
                        i,
                        comp_size,
                        decomp_size,
                        md5::Digest(checksum.try_into().unwrap())
                    );
                    offset += 24;
                }

                // Show actual chunk data
                let data_offset = 8 + header_size;
                println!("  Data starts at offset: {data_offset}");
                println!(
                    "  First 20 bytes of data: {:?}",
                    &multi_blte[data_offset..data_offset + 20.min(multi_blte.len() - data_offset)]
                );
            }
        }
    }
}
