//! Test with large real WoW BLTE files

use blte::{BLTEFile, CompressionMode, compress_data_multi, compress_data_single, decompress_blte};
use std::fs;
use std::io::Write;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_blte/real_full.blte";

    // Check if file exists
    if !std::path::Path::new(path).exists() {
        println!("File not found: {path}");
        println!("Please download a real BLTE file first:");
        println!(
            "  curl -s \"http://level3.blizzard.com/tpr/wow/data/00/17/0017a402f556fbece46c38dc431a2c9b\" -o test_blte/real_full.blte"
        );
        return Ok(());
    }

    println!("Loading real WoW BLTE file: {path}");
    let start = Instant::now();
    let data = fs::read(path)?;
    let load_time = start.elapsed();

    println!("File loaded: {} bytes in {:?}", data.len(), load_time);
    println!("File size: {:.2} MB", data.len() as f64 / 1_048_576.0);

    // Parse BLTE structure
    println!("\n=== Parsing BLTE Structure ===");
    let start = Instant::now();
    let blte = BLTEFile::parse(data.clone())?;
    let parse_time = start.elapsed();

    println!("Parse time: {parse_time:?}");
    println!("Header size: {} bytes", blte.header.header_size);
    println!("Single chunk: {}", blte.is_single_chunk());
    println!("Chunk count: {}", blte.chunk_count());

    // Show chunk statistics
    if !blte.is_single_chunk() {
        let mut total_compressed = 0u64;
        let mut total_decompressed = 0u64;
        let mut min_chunk = u32::MAX;
        let mut max_chunk = 0u32;

        for chunk_info in &blte.header.chunks {
            total_compressed += chunk_info.compressed_size as u64;
            total_decompressed += chunk_info.decompressed_size as u64;
            min_chunk = min_chunk.min(chunk_info.compressed_size);
            max_chunk = max_chunk.max(chunk_info.compressed_size);
        }

        let avg_chunk = total_compressed / blte.header.chunks.len() as u64;

        println!("\nChunk Statistics:");
        println!(
            "  Total compressed: {} bytes ({:.2} MB)",
            total_compressed,
            total_compressed as f64 / 1_048_576.0
        );
        println!(
            "  Total decompressed: {} bytes ({:.2} MB)",
            total_decompressed,
            total_decompressed as f64 / 1_048_576.0
        );
        println!("  Average chunk size: {avg_chunk} bytes");
        println!("  Min chunk size: {min_chunk} bytes");
        println!("  Max chunk size: {max_chunk} bytes");
        println!(
            "  Compression ratio: {:.2}%",
            (total_compressed as f64 / total_decompressed as f64) * 100.0
        );

        // Show first few chunks as examples
        println!("\nFirst 5 chunks:");
        for (i, chunk_info) in blte.header.chunks.iter().take(5).enumerate() {
            println!(
                "  Chunk {}: {} -> {} bytes (ratio: {:.2}%)",
                i,
                chunk_info.compressed_size,
                chunk_info.decompressed_size,
                (chunk_info.compressed_size as f64 / chunk_info.decompressed_size as f64) * 100.0
            );
        }
    }

    // Decompress the entire file
    println!("\n=== Decompressing BLTE ===");
    let start = Instant::now();
    let decompressed = decompress_blte(data.clone(), None)?;
    let decompress_time = start.elapsed();

    println!(
        "Decompressed size: {} bytes ({:.2} MB)",
        decompressed.len(),
        decompressed.len() as f64 / 1_048_576.0
    );
    println!("Decompression time: {decompress_time:?}");

    let decompress_speed = (data.len() as f64 / decompress_time.as_secs_f64()) / 1_048_576.0;
    println!("Decompression speed: {decompress_speed:.2} MB/s");

    // Determine compression mode from first chunk
    let mode = if let Ok(chunk) = blte.get_chunk_data(0) {
        match chunk.data.first() {
            Some(b'Z') => {
                println!("Detected compression: ZLib");
                CompressionMode::ZLib
            }
            Some(b'4') => {
                println!("Detected compression: LZ4");
                CompressionMode::LZ4
            }
            Some(b'N') => {
                println!("Detected compression: None");
                CompressionMode::None
            }
            Some(b'E') => {
                println!("Detected compression: Encrypted (cannot recompress)");
                return Ok(());
            }
            _ => {
                println!("Unknown compression mode, using ZLib");
                CompressionMode::ZLib
            }
        }
    } else {
        CompressionMode::ZLib
    };

    // Recompress the data
    println!("\n=== Recompressing Data ===");

    let recompressed = if blte.is_single_chunk() {
        println!("Using single-chunk compression...");
        let start = Instant::now();
        let result = compress_data_single(decompressed.clone(), mode, None)?;
        let compress_time = start.elapsed();
        println!("Compression time: {compress_time:?}");

        let compress_speed =
            (decompressed.len() as f64 / compress_time.as_secs_f64()) / 1_048_576.0;
        println!("Compression speed: {compress_speed:.2} MB/s");
        result
    } else {
        // Calculate average chunk size from original
        let avg_chunk_size = if !blte.header.chunks.is_empty() {
            let total: u32 = blte.header.chunks.iter().map(|c| c.decompressed_size).sum();
            (total / blte.header.chunks.len() as u32) as usize
        } else {
            256 * 1024 // Default 256KB
        };

        println!(
            "Using multi-chunk compression with {avg_chunk_size} byte chunks..."
        );
        let start = Instant::now();

        // Show progress for large files
        let total_chunks = decompressed.len().div_ceil(avg_chunk_size);
        println!("Expected chunks: {total_chunks}");

        let result = compress_data_multi(decompressed.clone(), avg_chunk_size, mode, None)?;
        let compress_time = start.elapsed();

        println!("Compression time: {compress_time:?}");
        let compress_speed =
            (decompressed.len() as f64 / compress_time.as_secs_f64()) / 1_048_576.0;
        println!("Compression speed: {compress_speed:.2} MB/s");
        result
    };

    println!(
        "Recompressed size: {} bytes ({:.2} MB)",
        recompressed.len(),
        recompressed.len() as f64 / 1_048_576.0
    );

    // Compare sizes
    let size_diff = recompressed.len() as i64 - data.len() as i64;
    let size_diff_pct = (size_diff as f64 / data.len() as f64) * 100.0;
    println!("\nSize comparison:");
    println!("  Original: {} bytes", data.len());
    println!("  Recompressed: {} bytes", recompressed.len());
    println!("  Difference: {size_diff} bytes ({size_diff_pct:+.2}%)");

    // Verify round-trip
    println!("\n=== Verifying Round-Trip ===");
    print!("Decompressing recompressed data...");
    std::io::stdout().flush()?;

    let start = Instant::now();
    let re_decompressed = decompress_blte(recompressed.clone(), None)?;
    let verify_time = start.elapsed();
    println!(" done in {verify_time:?}");

    if re_decompressed == decompressed {
        println!("✓ Round-trip successful! Data matches perfectly.");
        println!("  Original decompressed: {} bytes", decompressed.len());
        println!("  Re-decompressed: {} bytes", re_decompressed.len());
    } else {
        println!("✗ Round-trip failed! Data mismatch.");
        println!("  Original decompressed: {} bytes", decompressed.len());
        println!("  Re-decompressed: {} bytes", re_decompressed.len());

        // Find first difference
        for (i, (a, b)) in decompressed.iter().zip(re_decompressed.iter()).enumerate() {
            if a != b {
                println!("  First difference at byte {i}: {a} vs {b}");
                break;
            }
        }
    }

    // Performance summary
    println!("\n=== Performance Summary ===");
    println!("Original file: {:.2} MB", data.len() as f64 / 1_048_576.0);
    println!(
        "Decompressed: {:.2} MB",
        decompressed.len() as f64 / 1_048_576.0
    );
    println!(
        "Compression ratio: {:.2}%",
        (data.len() as f64 / decompressed.len() as f64) * 100.0
    );
    println!(
        "Decompression speed: {:.2} MB/s",
        (data.len() as f64 / decompress_time.as_secs_f64()) / 1_048_576.0
    );
    println!(
        "Compression speed: {:.2} MB/s",
        (decompressed.len() as f64
            / if blte.is_single_chunk() {
                recompressed.len() as f64 / (decompressed.len() as f64 / 1_048_576.0)
            } else {
                recompressed.len() as f64 / (decompressed.len() as f64 / 1_048_576.0)
            })
            / 1_048_576.0
    );

    // Optional: Save recompressed file for comparison
    println!("\nSaving recompressed file for inspection...");
    fs::write("test_blte/recompressed.blte", &recompressed)?;
    println!("Saved to: test_blte/recompressed.blte");

    Ok(())
}
