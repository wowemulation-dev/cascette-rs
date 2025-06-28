//! Demonstration of streaming I/O operations in ngdp-cache

use ngdp_cache::generic::GenericCache;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 ngdp-cache Streaming I/O Demo");
    println!("=================================\n");

    // Create cache
    let cache = GenericCache::with_subdirectory("streaming_demo").await?;

    // Test data sizes
    let test_sizes = [
        ("Small", 1024),            // 1KB
        ("Medium", 1024 * 1024),    // 1MB
        ("Large", 5 * 1024 * 1024), // 5MB
    ];

    for (name, size) in &test_sizes {
        println!("📊 Testing {name} files ({size} bytes):");

        // Generate test data
        let test_data = generate_test_data(*size);
        let key = format!("test_{}", name.to_lowercase());

        // Test 1: Regular write vs streaming write
        let start = Instant::now();
        cache.write(&key, &test_data).await?;
        let regular_write_time = start.elapsed();
        cache.delete(&key).await?;

        let start = Instant::now();
        let mut reader = std::io::Cursor::new(test_data.clone());
        cache.write_streaming(&key, &mut reader).await?;
        let streaming_write_time = start.elapsed();

        println!("   Write Performance:");
        println!("     Regular:   {regular_write_time:>8.2?}");
        println!(
            "     Streaming: {:>8.2?} ({:.1}x)",
            streaming_write_time,
            regular_write_time.as_nanos() as f64 / streaming_write_time.as_nanos() as f64
        );

        // Test 2: Regular read vs streaming read
        let start = Instant::now();
        let _data = cache.read(&key).await?;
        let regular_read_time = start.elapsed();

        let start = Instant::now();
        let mut output = Vec::new();
        cache.read_streaming(&key, &mut output).await?;
        let streaming_read_time = start.elapsed();

        println!("   Read Performance:");
        println!("     Regular:   {regular_read_time:>8.2?}");
        println!(
            "     Streaming: {:>8.2?} ({:.1}x)",
            streaming_read_time,
            regular_read_time.as_nanos() as f64 / streaming_read_time.as_nanos() as f64
        );

        // Test 3: Chunked processing
        let start = Instant::now();
        let mut chunk_count = 0;
        let mut total_bytes = 0u64;

        cache
            .read_chunked(&key, |chunk| {
                chunk_count += 1;
                total_bytes += chunk.len() as u64;
                Ok(())
            })
            .await?;

        let chunked_time = start.elapsed();

        println!("   Chunked Processing:");
        println!("     Time:      {chunked_time:>8.2?}");
        println!("     Chunks:    {chunk_count:>8}");
        println!("     Bytes:     {total_bytes:>8}");

        // Test 4: Memory usage estimation
        let file_size = cache.size(&key).await?;
        println!("   Memory Efficiency:");
        println!("     File size: {file_size:>8} bytes");
        println!("     Regular:   ~{file_size:>7} bytes (loads entire file)");
        println!("     Streaming: ~{:>7} bytes (8KB buffer)", 8192);

        cache.delete(&key).await?;
        println!();
    }

    // Demonstrate advanced features
    println!("⚡ Advanced Features Demo:");
    println!("========================\n");

    // Chunked write demo
    println!("📝 Chunked Write:");
    let chunks = create_chunked_data(1024 * 1024); // 1MB in chunks
    let start = Instant::now();
    cache.write_chunked("chunked_demo", chunks).await?;
    let chunked_write_time = start.elapsed();
    println!("   Wrote 1MB in chunks: {chunked_write_time:?}");

    // Copy operation demo
    println!("\n📋 Copy Operation:");
    let start = Instant::now();
    cache.copy("chunked_demo", "copied_demo").await?;
    let copy_time = start.elapsed();
    println!("   Copied 1MB file: {copy_time:?}");

    // Buffered streaming demo
    println!("\n🚄 Buffered Streaming:");
    let start = Instant::now();
    let mut output = Vec::new();
    cache
        .read_streaming_buffered("copied_demo", &mut output, 64 * 1024)
        .await?; // 64KB buffer
    let buffered_time = start.elapsed();
    println!("   Read with 64KB buffer: {buffered_time:?}");

    // Progress tracking demo
    println!("\n📈 Progress Tracking Demo:");
    demonstrate_progress_tracking(&cache).await?;

    // Cleanup
    cache.delete("chunked_demo").await?;
    cache.delete("copied_demo").await?;

    println!("\n✅ Demo completed successfully!");
    println!("\n💡 Key Benefits of Streaming I/O:");
    println!("   • Constant memory usage regardless of file size");
    println!("   • Better performance for large files");
    println!("   • Progress tracking capabilities");
    println!("   • Efficient copy operations");
    println!("   • Chunked processing for data transformation");

    Ok(())
}

fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

fn create_chunked_data(total_size: usize) -> Vec<Result<Vec<u8>, ngdp_cache::Error>> {
    let chunk_size = 8192; // 8KB chunks
    let num_chunks = total_size / chunk_size;

    (0..num_chunks)
        .map(|i| Ok(vec![(i % 256) as u8; chunk_size]))
        .collect()
}

async fn demonstrate_progress_tracking(
    cache: &GenericCache,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a file for progress demo
    let test_data = generate_test_data(512 * 1024); // 512KB
    cache.write("progress_demo", &test_data).await?;

    println!("   Reading with progress tracking:");

    let mut progress_bytes = 0u64;
    let file_size = cache.size("progress_demo").await?;

    cache
        .read_chunked("progress_demo", |chunk| {
            progress_bytes += chunk.len() as u64;
            let percentage = (progress_bytes as f64 / file_size as f64) * 100.0;

            // Update progress (simplified - in real usage you'd use a proper progress bar)
            if progress_bytes % (64 * 1024) == 0 || progress_bytes == file_size {
                println!("     Progress: {percentage:.1}% ({progress_bytes}/{file_size})");
            }

            Ok(())
        })
        .await?;

    cache.delete("progress_demo").await?;
    Ok(())
}
