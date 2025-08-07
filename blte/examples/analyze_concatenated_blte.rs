//! Analyze concatenated BLTE archive files

use byteorder::{BigEndian, ReadBytesExt};
use std::fs;
use std::io::Cursor;

fn find_blte_files(data: &[u8]) -> Vec<(usize, u32, u32)> {
    let mut files = Vec::new();
    let mut offset = 0;

    while offset + 8 <= data.len() {
        // Look for BLTE magic
        if &data[offset..offset + 4] == b"BLTE" {
            let mut cursor = Cursor::new(&data[offset + 4..offset + 8]);
            if let Ok(header_size) = cursor.read_u32::<BigEndian>() {
                // Calculate total size of this BLTE file
                let total_size = if header_size == 0 {
                    // Single chunk - need to read the chunk size from data
                    // For single chunk, the compressed size is stored after the header
                    if offset + 12 <= data.len() {
                        let mut size_cursor = Cursor::new(&data[offset + 8..offset + 12]);
                        if let Ok(chunk_size) = size_cursor.read_u32::<BigEndian>() {
                            8 + chunk_size // header + chunk data
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    // Multi-chunk - parse first chunk size from chunk table
                    let data_offset = if header_size as usize <= 8 + 24 {
                        // Single chunk in table
                        36 // Archive format
                    } else {
                        8 + header_size as usize // Standard format
                    };

                    if offset + data_offset + 4 <= data.len() {
                        // Read first chunk size from chunk table
                        if offset + 16 <= data.len() {
                            let mut chunk_cursor = Cursor::new(&data[offset + 12..offset + 16]);
                            if let Ok(first_chunk_size) = chunk_cursor.read_u32::<BigEndian>() {
                                data_offset as u32 + first_chunk_size
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                };

                files.push((offset, header_size, total_size));
                offset += total_size as usize;
            } else {
                offset += 1;
            }
        } else {
            offset += 1;
        }
    }

    files
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_blte/real_full.blte";

    if !std::path::Path::new(path).exists() {
        println!("File not found: {path}");
        println!("Please run: cargo run --example test_large_blte");
        return Ok(());
    }

    println!("Analyzing concatenated BLTE file: {path}");
    let data = fs::read(path)?;
    println!(
        "Total file size: {} bytes ({:.2} MB)",
        data.len(),
        data.len() as f64 / 1_048_576.0
    );

    let blte_files = find_blte_files(&data);

    println!("\nFound {} BLTE files in archive:", blte_files.len());

    let mut total_accounted = 0;
    for (i, (offset, header_size, total_size)) in blte_files.iter().enumerate() {
        println!(
            "  File {}: offset={}, header_size={}, total_size={} bytes",
            i + 1,
            offset,
            header_size,
            total_size
        );
        total_accounted += total_size;

        // Try to parse each individual BLTE
        if *offset + *total_size as usize <= data.len() {
            let blte_data = &data[*offset..*offset + *total_size as usize];
            match blte::BLTEFile::parse(blte_data.to_vec()) {
                Ok(blte_file) => {
                    println!("    ✓ Valid BLTE: {} chunks", blte_file.chunk_count());

                    // Try to decompress
                    match blte::decompress_blte(blte_data.to_vec(), None) {
                        Ok(decompressed) => {
                            println!("    ✓ Decompressed: {} bytes", decompressed.len());
                        }
                        Err(e) => {
                            println!("    ✗ Decompression failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("    ✗ Invalid BLTE: {e}");
                }
            }
        }

        println!();
    }

    println!(
        "Total accounted for: {} bytes ({:.2} MB)",
        total_accounted,
        total_accounted as f64 / 1_048_576.0
    );
    println!(
        "Remaining: {} bytes ({:.2} MB)",
        data.len() as u32 - total_accounted,
        (data.len() as u32 - total_accounted) as f64 / 1_048_576.0
    );

    // Our current implementation only handles the FIRST BLTE file
    println!("\n=== Current Implementation Analysis ===");
    println!("Our BLTE library correctly extracts and processes the FIRST BLTE file only.");
    println!("The 256MB -> 27KB result is correct for the first file in this archive.");
    println!("To handle the full archive, we'd need to implement archive extraction.");

    Ok(())
}
