//! Analyze large BLTE file structure without full decompression

use blte::BLTEFile;
use std::fs;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_blte/real_full.blte";

    println!("Analyzing real WoW BLTE file: {path}");
    let data = fs::read(path)?;
    println!(
        "File size: {} bytes ({:.2} MB)",
        data.len(),
        data.len() as f64 / 1_048_576.0
    );

    // Show first few bytes
    println!("\nFirst 32 bytes (hex):");
    for i in 0..2 {
        print!("  {:04x}: ", i * 16);
        for j in 0..16 {
            let idx = i * 16 + j;
            if idx < data.len() {
                print!("{:02x} ", data[idx]);
            }
        }
        println!();
    }

    // Parse just the header
    println!("\n=== BLTE Header Analysis ===");
    let start = Instant::now();
    let blte = BLTEFile::parse(data.clone())?;
    let parse_time = start.elapsed();

    println!("Parse time: {parse_time:?}");
    println!("Magic: {:?}", &data[0..4]);
    println!(
        "Header size field: {} (0x{:08x})",
        blte.header.header_size, blte.header.header_size
    );
    println!("Is single chunk: {}", blte.is_single_chunk());
    println!("Chunk count: {}", blte.chunk_count());
    println!("Data offset: {}", blte.header.data_offset());

    if !blte.is_single_chunk() {
        println!("\n=== Chunk Table ===");
        let header_data = &data[8..8 + blte.header.header_size as usize];
        println!("Flags byte: 0x{:02x}", header_data[0]);

        // Parse chunk count (3 bytes big-endian)
        let chunk_count = ((header_data[1] as u32) << 16)
            | ((header_data[2] as u32) << 8)
            | (header_data[3] as u32);
        println!("Chunk count from header: {chunk_count}");

        // Show all chunks (since there's only 1)
        for (i, chunk_info) in blte.header.chunks.iter().enumerate() {
            println!("\nChunk {i}:");
            println!("  Compressed size: {} bytes", chunk_info.compressed_size);
            println!(
                "  Decompressed size: {} bytes",
                chunk_info.decompressed_size
            );
            println!("  Checksum: {:02x?}", &chunk_info.checksum);
            println!(
                "  Compression ratio: {:.2}%",
                (chunk_info.compressed_size as f64 / chunk_info.decompressed_size as f64) * 100.0
            );
        }

        // Check if the data section looks correct
        let data_start = blte.header.data_offset();
        println!("\n=== Data Section ===");
        println!("Data starts at byte: {data_start}");
        println!("Expected data size: {} bytes", data.len() - data_start);

        if data_start < data.len() {
            println!("First 32 bytes of data section:");
            for i in 0..2 {
                print!("  {:04x}: ", data_start + i * 16);
                for j in 0..16 {
                    let idx = data_start + i * 16 + j;
                    if idx < data.len() {
                        print!("{:02x} ", data[idx]);
                    }
                }
                println!();
            }

            // Check compression mode of first chunk
            if data_start < data.len() {
                let mode_byte = data[data_start];
                print!("First byte of chunk data: 0x{mode_byte:02x} ");
                match mode_byte {
                    b'N' => println!("(No compression)"),
                    b'Z' => println!("(ZLib compression)"),
                    b'4' => println!("(LZ4 compression)"),
                    b'F' => println!("(Frame/recursive BLTE)"),
                    b'E' => println!("(Encrypted)"),
                    _ => println!("(Unknown compression mode)"),
                }
            }
        }

        // The issue: this file claims to have 1 chunk of 26,767 bytes compressed
        // But the file is 268MB. This suggests it might be an archive containing multiple BLTEs
        println!("\n=== Anomaly Detection ===");
        let total_chunk_size: u32 = blte.header.chunks.iter().map(|c| c.compressed_size).sum();
        let expected_file_size = data_start + total_chunk_size as usize;
        let actual_file_size = data.len();

        println!("Expected file size (header + chunks): {expected_file_size} bytes");
        println!("Actual file size: {actual_file_size} bytes");

        if actual_file_size > expected_file_size {
            println!(
                "⚠ File is {} bytes larger than expected!",
                actual_file_size - expected_file_size
            );
            println!("This might be an archive containing multiple BLTE files.");

            // Check if there's another BLTE after the first one
            let next_blte_pos = expected_file_size;
            if next_blte_pos + 4 < data.len() {
                let next_magic = &data[next_blte_pos..next_blte_pos + 4];
                if next_magic == b"BLTE" {
                    println!("\n✓ Found another BLTE file at offset {next_blte_pos}!");
                    println!("This is definitely an archive file containing multiple BLTEs.");
                } else {
                    println!("\nNext 4 bytes at expected position: {next_magic:02x?}");
                }
            }
        }
    }

    Ok(())
}
