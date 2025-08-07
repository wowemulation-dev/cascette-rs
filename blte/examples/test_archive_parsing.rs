//! Test archive parsing with the real 256MB WoW file

use blte::{BLTEArchive, Result};
use std::fs;
use std::time::Instant;

fn main() -> Result<()> {
    let path = "test_blte/real_full.blte";

    if !std::path::Path::new(path).exists() {
        println!("File not found: {}", path);
        println!("Please run: cargo run --example test_large_blte");
        return Ok(());
    }

    println!("=== Testing BLTE Archive Parsing ===");
    println!("Loading 256MB archive: {}", path);

    let start = Instant::now();
    let data = fs::read(path)?;
    let load_time = start.elapsed();

    let data_len = data.len();
    println!("Loaded {} bytes in {:?}", data_len, load_time);

    // Parse archive
    println!("\n=== Parsing Archive Structure ===");
    let start = Instant::now();
    let archive = BLTEArchive::parse(data)?;
    let parse_time = start.elapsed();

    println!("Parse time: {:?}", parse_time);
    println!("Found {} BLTE files in archive", archive.file_count());

    // Get archive statistics
    let stats = archive.stats();
    println!("\n=== Archive Statistics ===");
    println!("Total files: {}", stats.file_count);
    println!(
        "Total size: {} bytes ({:.2} MB)",
        stats.total_size,
        stats.total_size as f64 / 1_048_576.0
    );
    println!(
        "Compressed size: {} bytes ({:.2} MB)",
        stats.compressed_size,
        stats.compressed_size as f64 / 1_048_576.0
    );

    if let Some(decompressed) = stats.decompressed_size {
        println!(
            "Decompressed size: {} bytes ({:.2} MB)",
            decompressed,
            decompressed as f64 / 1_048_576.0
        );

        if let Some(ratio) = stats.compression_ratio {
            println!("Compression ratio: {:.2}%", ratio);
        }
    }

    // Show file size distribution
    let dist = &stats.size_distribution;
    println!("\n=== File Size Distribution ===");
    println!("Smallest file: {} bytes", dist.min_size);
    println!(
        "Largest file: {} bytes ({:.2} KB)",
        dist.max_size,
        dist.max_size as f64 / 1024.0
    );
    println!("Average file: {:.2} bytes", dist.avg_size);
    println!("Median file: {} bytes", dist.median_size);
    println!("Standard deviation: {:.2} bytes", dist.std_dev);

    // Show details of first few files
    println!("\n=== First 10 Files ===");
    for i in 0..10.min(archive.file_count()) {
        if let Ok(entry) = archive.file_info(i) {
            println!(
                "File {}: offset={}, size={} bytes, chunks={}",
                i + 1,
                entry.offset,
                entry.size,
                entry.metadata.chunk_count
            );
        }
    }

    println!("\n=== Performance Summary ===");
    println!(
        "Archive parsing: {:.2} files/ms",
        archive.file_count() as f64 / parse_time.as_millis() as f64
    );
    println!(
        "Data throughput: {:.2} MB/s",
        (data_len as f64 / 1_048_576.0) / parse_time.as_secs_f64()
    );

    // Test individual file access (just metadata, don't load full files)
    println!("\n=== Testing File Access ===");
    let test_indices = [0, 10, 100, 1000, archive.file_count() - 1]
        .iter()
        .filter(|&&i| i < archive.file_count())
        .copied()
        .collect::<Vec<_>>();

    for &index in &test_indices {
        let start = Instant::now();
        let entry = archive.file_info(index)?;
        let access_time = start.elapsed();

        println!(
            "File {} access: {:?} (size: {} bytes)",
            index, access_time, entry.size
        );
    }

    println!("\n✅ Archive parsing test completed successfully!");
    println!("The archive module can successfully parse and analyze 256MB CDN archives.");

    Ok(())
}
