//! Test compression mode detection with real archive files

use blte::{BLTEArchive, CompressionMode, Result, detect_compression_mode};
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

fn main() -> Result<()> {
    let path = "test_blte/real_full.blte";

    if !std::path::Path::new(path).exists() {
        println!("File not found: {}", path);
        println!("Please run: cargo run --example test_large_blte");
        return Ok(());
    }

    println!("=== Testing Compression Mode Detection ===");
    println!("Loading 256MB archive: {}", path);

    let data = fs::read(path)?;
    println!("Loaded {} bytes", data.len());

    // Parse archive
    let mut archive = BLTEArchive::parse(data)?;
    println!("Found {} BLTE files in archive", archive.file_count());

    // Test compression mode detection on first 100 files
    println!("\n=== Analyzing Compression Modes ===");
    let sample_size = 100.min(archive.file_count());
    let mut mode_counts = HashMap::new();
    let mut detection_errors = 0;
    let mut total_time = std::time::Duration::new(0, 0);

    println!("Testing first {} files...", sample_size);

    for i in 0..sample_size {
        let start = Instant::now();

        match archive.get_file(i) {
            Ok(blte) => match detect_compression_mode(blte) {
                Ok(mode) => {
                    *mode_counts.entry(mode).or_insert(0) += 1;

                    if i < 10 {
                        println!(
                            "File {}: {:?} (size: {} bytes, chunks: {})",
                            i,
                            mode,
                            blte.total_size(),
                            blte.chunk_count()
                        );
                    }
                }
                Err(e) => {
                    detection_errors += 1;
                    if detection_errors <= 5 {
                        println!("File {}: Detection error: {}", i, e);
                    }
                }
            },
            Err(e) => {
                println!("File {}: Failed to load: {}", i, e);
                detection_errors += 1;
            }
        }

        total_time += start.elapsed();
    }

    // Summary statistics
    println!("\n=== Compression Mode Statistics ===");
    let mut total_detected = 0;
    for (mode, count) in &mode_counts {
        println!(
            "{:?}: {} files ({:.1}%)",
            mode,
            count,
            (*count as f64 / sample_size as f64) * 100.0
        );
        total_detected += count;
    }

    println!("\nSummary:");
    println!("  Total files analyzed: {}", sample_size);
    println!("  Successfully detected: {}", total_detected);
    println!("  Detection errors: {}", detection_errors);
    println!(
        "  Success rate: {:.1}%",
        (total_detected as f64 / sample_size as f64) * 100.0
    );
    println!(
        "  Average detection time: {:?}",
        total_time / sample_size as u32
    );

    // Test extraction with metadata on a few files
    println!("\n=== Testing Metadata Extraction ===");
    for i in 0..5.min(archive.file_count()) {
        println!("Testing file {}...", i);

        let start = Instant::now();
        match archive.extract_file_with_metadata(i) {
            Ok(extracted) => {
                let extraction_time = start.elapsed();
                let meta = &extracted.metadata;

                println!("  ✓ Extracted file {} in {:?}", i, extraction_time);
                println!("    Original index: {}", extracted.original_index);
                println!("    Data size: {} bytes", extracted.data.len());
                println!("    Compression mode: {:?}", meta.compression_mode);
                println!("    Header format: {:?}", meta.header_format);
                println!("    Original offset: {}", meta.original_offset);
                println!("    Original size: {} bytes", meta.original_size);

                match &meta.chunk_structure {
                    blte::ChunkStructure::SingleChunk { decompressed_size } => {
                        println!("    Chunk structure: Single ({} bytes)", decompressed_size);
                    }
                    blte::ChunkStructure::MultiChunk {
                        chunk_count,
                        decompressed_sizes,
                        ..
                    } => {
                        println!(
                            "    Chunk structure: Multi ({} chunks, {:?} bytes)",
                            chunk_count, decompressed_sizes
                        );
                    }
                }

                println!("    Checksums: {} entries", meta.checksums.len());
                println!("    Compressed sizes: {:?}", meta.compressed_sizes);
                println!();
            }
            Err(e) => {
                println!("  ✗ Failed to extract file {}: {}", i, e);
            }
        }
    }

    println!("✅ Compression mode detection test completed!");

    // Verify our detection matches expected patterns
    if mode_counts.contains_key(&CompressionMode::ZLib) {
        println!("✅ ZLib compression detected - this is expected for WoW archives");
    }
    if mode_counts.contains_key(&CompressionMode::None) {
        println!("✅ No compression detected - this is expected for some files");
    }
    if mode_counts.contains_key(&CompressionMode::LZ4) {
        println!("✅ LZ4 compression detected - this may be present");
    }

    let success_rate = (total_detected as f64 / sample_size as f64) * 100.0;
    if success_rate > 90.0 {
        println!("🎉 Excellent detection rate: {:.1}%", success_rate);
    } else if success_rate > 75.0 {
        println!("✅ Good detection rate: {:.1}%", success_rate);
    } else {
        println!("⚠️  Detection rate needs improvement: {:.1}%", success_rate);
    }

    Ok(())
}
