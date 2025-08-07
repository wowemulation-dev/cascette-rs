//! Test round-trip compression/decompression with real WoW BLTE files

use blte::{BLTEFile, CompressionMode, compress_data_multi, compress_data_single, decompress_blte};
use std::fs;
use std::io::Read;
use std::time::Instant;

fn download_blte_file() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("Downloading a real BLTE file from WoW CDN...");

    // This is a smaller BLTE file from WoW Classic Era CDN
    // We'll try several until we find one that's reasonable size
    let urls = [
        // Try some smaller files first
        "http://level3.blizzard.com/tpr/wow/data/f9/75/f9753a1b7a9876651d16b7ddada891f2",
        "http://level3.blizzard.com/tpr/wow/data/e1/94/e194cceb85e0c1d9e28ef529fb7d3cf7",
        "http://level3.blizzard.com/tpr/wow/data/30/1d/301024def5755d8529d944f88fdb4b3c",
    ];

    for url in &urls {
        println!("Trying {url}");
        let response = ureq::get(url)
            .timeout(std::time::Duration::from_secs(10))
            .call();

        match response {
            Ok(resp) => {
                let len = resp
                    .header("content-length")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                if len > 0 && len < 10_000_000 {
                    // Less than 10MB
                    println!("  Found file: {len} bytes");
                    let mut data = Vec::with_capacity(len);
                    resp.into_reader().read_to_end(&mut data)?;
                    return Ok(data);
                } else {
                    println!("  File too large: {len} bytes, skipping");
                }
            }
            Err(e) => {
                println!("  Failed: {e}");
            }
        }
    }

    Err("Could not find a suitable BLTE file".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Try to use a local file first, or download one
    let data = if let Ok(local_data) = fs::read("test_blte/small.blte") {
        println!("Using local BLTE file: test_blte/small.blte");
        println!("File size: {} bytes", local_data.len());
        local_data
    } else {
        match download_blte_file() {
            Ok(d) => d,
            Err(_) => {
                // Create a test BLTE file with known data
                println!("Creating test BLTE file with known data...");
                create_test_blte()?
            }
        }
    };

    // Parse and analyze the BLTE structure
    println!("\n=== Analyzing BLTE Structure ===");
    let blte = BLTEFile::parse(data.clone())?;

    println!("Header size: {}", blte.header.header_size);
    println!("Single chunk: {}", blte.is_single_chunk());
    println!("Chunk count: {}", blte.chunk_count());

    if !blte.is_single_chunk() && blte.chunk_count() <= 10 {
        for (i, chunk_info) in blte.header.chunks.iter().enumerate() {
            println!(
                "  Chunk {}: {} compressed, {} decompressed",
                i, chunk_info.compressed_size, chunk_info.decompressed_size
            );
        }
    }

    // Decompress the BLTE file
    println!("\n=== Decompressing BLTE ===");
    let start = Instant::now();
    let decompressed = decompress_blte(data.clone(), None)?;
    let decompress_time = start.elapsed();

    println!("Decompressed size: {} bytes", decompressed.len());
    println!("Decompression time: {decompress_time:?}");

    // Calculate compression ratio
    let ratio = data.len() as f64 / decompressed.len() as f64;
    println!(
        "Compression ratio: {:.2}% ({}x smaller)",
        ratio * 100.0,
        1.0 / ratio
    );

    // Try to recompress with our implementation
    println!("\n=== Recompressing Data ===");

    // Determine compression mode from first chunk
    let mode = if let Ok(chunk) = blte.get_chunk_data(0) {
        match chunk.data.first() {
            Some(b'Z') => CompressionMode::ZLib,
            Some(b'4') => CompressionMode::LZ4,
            Some(b'N') => CompressionMode::None,
            _ => CompressionMode::ZLib, // Default
        }
    } else {
        CompressionMode::ZLib
    };

    println!("Using compression mode: {mode:?}");

    // For single chunk, use single compression
    let recompressed = if blte.is_single_chunk() {
        let start = Instant::now();
        let result = compress_data_single(decompressed.clone(), mode, None)?;
        let compress_time = start.elapsed();
        println!("Compression time: {compress_time:?}");
        result
    } else {
        // For multi-chunk, try to use similar chunk size
        let avg_chunk_size = if !blte.header.chunks.is_empty() {
            let total: u32 = blte.header.chunks.iter().map(|c| c.decompressed_size).sum();
            (total / blte.header.chunks.len() as u32) as usize
        } else {
            256 * 1024 // Default 256KB chunks
        };

        println!("Using chunk size: {avg_chunk_size} bytes");
        let start = Instant::now();
        let result = compress_data_multi(decompressed.clone(), avg_chunk_size, mode, None)?;
        let compress_time = start.elapsed();
        println!("Compression time: {compress_time:?}");
        result
    };

    println!("Recompressed size: {} bytes", recompressed.len());

    // Compare sizes
    let size_diff = recompressed.len() as i64 - data.len() as i64;
    let size_diff_pct = (size_diff as f64 / data.len() as f64) * 100.0;
    println!("\nSize difference: {size_diff} bytes ({size_diff_pct:+.2}%)");

    // Verify round-trip
    println!("\n=== Verifying Round-Trip ===");
    let re_decompressed = decompress_blte(recompressed.clone(), None)?;

    if re_decompressed == decompressed {
        println!("✓ Round-trip successful! Data matches perfectly.");
    } else {
        println!("✗ Round-trip failed! Data mismatch.");
        println!("  Original decompressed: {} bytes", decompressed.len());
        println!("  Re-decompressed: {} bytes", re_decompressed.len());
    }

    // Save files for inspection
    if data.len() < 1_000_000 {
        // Only save if less than 1MB
        fs::create_dir_all("test_output")?;
        fs::write("test_output/original.blte", &data)?;
        fs::write("test_output/decompressed.bin", &decompressed)?;
        fs::write("test_output/recompressed.blte", &recompressed)?;
        println!("\nFiles saved to test_output/ directory for inspection.");
    }

    Ok(())
}

fn create_test_blte() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use blte::BLTEBuilder;

    // Create test data that's somewhat realistic
    let mut test_data = Vec::new();

    // Add some repetitive data (simulating game data patterns)
    for i in 0..1000 {
        test_data.extend_from_slice(b"SPELL_CAST_SUCCESS,");
        test_data.extend_from_slice(format!("Player-{i},").as_bytes());
        test_data.extend_from_slice(b"0x0000000000000000,");
        test_data.extend_from_slice(b"\"Unknown\",0x0,");
        test_data.extend_from_slice(format!("{},", i * 100).as_bytes());
        test_data.extend_from_slice(b"\"Fireball\",0x1\n");
    }

    // Build a multi-chunk BLTE
    let blte = BLTEBuilder::new()
        .with_compression(CompressionMode::ZLib)
        .with_chunk_size(8192) // 8KB chunks
        .add_large_data(test_data)
        .build()?;

    println!("Created test BLTE: {} bytes", blte.len());
    Ok(blte)
}
