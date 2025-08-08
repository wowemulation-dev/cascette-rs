//! Example demonstrating progressive file loading with size hints
//!
//! This example shows how to use progressive file loading to efficiently
//! read large files in chunks, reducing memory usage and improving
//! performance for partial file access.

use casc_storage::*;
use std::io::Write;
use std::time::Instant;
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better debugging
    tracing_subscriber::fmt::init();

    println!("=== Progressive File Loading Example ===\n");

    // Create a temporary storage for demonstration
    let temp_dir = tempfile::tempdir()?;
    let data_path = temp_dir.path().to_path_buf();
    
    // Create directory structure
    let data_subdir = data_path.join("data");
    std::fs::create_dir_all(&data_subdir)?;

    let config = types::CascConfig {
        data_path,
        read_only: false,
        cache_size_mb: 64, // Small cache to demonstrate progressive loading benefits
        max_archive_size: 16 * 1024 * 1024,
        compression_level: 6,
    };

    let mut storage = CascStorage::new(config)?;

    // Initialize progressive loading with custom configuration
    let progressive_config = ProgressiveConfig {
        chunk_size: 128 * 1024,        // 128KB chunks
        max_prefetch_chunks: 3,        // Prefetch up to 3 chunks ahead
        min_progressive_size: 512 * 1024, // Use progressive for files >512KB
        use_predictive_prefetch: true,  // Enable smart prefetching
        ..ProgressiveConfig::default()
    };
    
    storage.init_progressive_loading(progressive_config);
    println!("✅ Initialized progressive loading (chunk_size: 128KB, prefetch: 3 chunks)");

    // Create sample files of different sizes to demonstrate progressive loading
    let samples = vec![
        ("small_file", create_sample_data(256 * 1024, "Small file content")),      // 256KB - won't use progressive
        ("medium_file", create_sample_data(1 * 1024 * 1024, "Medium file content")), // 1MB - will use progressive
        ("large_file", create_sample_data(5 * 1024 * 1024, "Large file content")),   // 5MB - will use progressive
    ];

    let mut ekeys = Vec::new();
    
    println!("\n📝 Creating sample files...");
    for (name, data) in &samples {
        let ekey = types::EKey::from_slice(&blake3::hash(name.as_bytes()).as_bytes()[0..16]);
        storage.write(&ekey, data)?;
        ekeys.push((name, ekey, data.len()));
        println!("  - {}: {} bytes -> EKey {}", name, data.len(), ekey);
    }

    println!("\n🔍 Testing progressive loading vs traditional loading...\n");
    
    for (name, ekey, size) in &ekeys {
        println!("=== Testing {} ({} bytes) ===", name, size);
        
        // Get size hint from storage
        let size_hint = storage.get_size_hint_for_ekey(ekey);
        let should_use_progressive = size_hint.should_use_progressive(&storage.progressive_manager.as_ref().unwrap().config);
        
        println!("  Size hint: {:?}", size_hint);
        println!("  Will use progressive loading: {}", should_use_progressive);
        
        if should_use_progressive {
            // Test progressive loading
            await_progressive_loading_demo(&storage, ekey, *size, size_hint).await?;
        } else {
            println!("  ⚠️ File too small for progressive loading, using traditional read");
            let start = Instant::now();
            let data = storage.read(ekey)?;
            let duration = start.elapsed();
            println!("  📊 Traditional read: {} bytes in {:?}", data.len(), duration);
        }
        
        println!();
    }

    // Demonstrate concurrent progressive reads
    if let Some((_, large_ekey, large_size)) = ekeys.iter().find(|(name, _, _)| name == &"large_file") {
        println!("🚀 Testing concurrent progressive reads on large file...");
        await_concurrent_progressive_demo(&storage, large_ekey, *large_size).await?;
    }

    // Show global statistics
    println!("📈 Progressive Loading Statistics:");
    let global_stats = storage.get_progressive_stats().await;
    for (ekey, stats) in global_stats {
        println!("  EKey {}: {:#?}", ekey, stats);
    }

    // Cleanup demonstration
    println!("\n🧹 Cleaning up inactive progressive files...");
    storage.cleanup_progressive_files().await;
    
    println!("✅ Progressive loading example completed successfully!");
    Ok(())
}

async fn await_progressive_loading_demo(
    storage: &CascStorage,
    ekey: &types::EKey,
    file_size: usize,
    size_hint: SizeHint,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create progressive file
    let start = Instant::now();
    let progressive_file = storage.read_progressive(ekey, size_hint).await?;
    let creation_time = start.elapsed();
    
    println!("  📂 Created progressive file in {:?}", creation_time);
    
    // Test different read patterns
    let scenarios = vec![
        ("beginning", 0, 4096),
        ("middle", file_size / 2, 4096),
        ("near_end", file_size.saturating_sub(8192), 4096),
        ("large_chunk", 0, 64 * 1024), // 64KB
    ];
    
    for (scenario_name, offset, length) in scenarios {
        let length = length.min(file_size - offset); // Don't read past end
        if length == 0 {
            continue;
        }
        
        let start = Instant::now();
        let chunk_data = progressive_file.read(offset as u64, length).await?;
        let read_time = start.elapsed();
        
        println!("  📖 {} read: {} bytes from offset {} in {:?}", 
                scenario_name, chunk_data.len(), offset, read_time);
        
        // Verify data integrity
        let expected_pattern = (offset % 256) as u8;
        if !chunk_data.is_empty() && chunk_data[0] != expected_pattern {
            println!("  ⚠️ Data integrity check failed!");
        }
    }
    
    // Show progressive file statistics
    let stats = progressive_file.get_stats().await;
    println!("  📊 Progressive file stats: chunks_loaded={}, bytes_loaded={}, cache_hits={}, cache_misses={}", 
            stats.chunks_loaded, stats.bytes_loaded, stats.cache_hits, stats.cache_misses);
    
    // Compare with traditional reading for full file
    let start = Instant::now();
    let _traditional_data = storage.read(ekey)?;
    let traditional_time = start.elapsed();
    
    let start = Instant::now();
    let _progressive_data = progressive_file.read(0, file_size).await?;
    let progressive_full_time = start.elapsed();
    
    println!("  ⚖️ Performance comparison (full file):");
    println!("     Traditional: {:?}", traditional_time);
    println!("     Progressive: {:?}", progressive_full_time);
    let ratio = progressive_full_time.as_nanos() as f64 / traditional_time.as_nanos() as f64;
    println!("     Ratio: {:.2}x", ratio);
    
    Ok(())
}

async fn await_concurrent_progressive_demo(
    storage: &CascStorage,
    ekey: &types::EKey,
    file_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let progressive_file = storage.read_progressive(ekey, SizeHint::Exact(file_size as u64)).await?;
    
    // Launch concurrent reads from different parts of the file
    let file1 = progressive_file.clone();
    let file2 = progressive_file.clone();
    let file3 = progressive_file.clone();
    
    let start = Instant::now();
    
    let (result1, result2, result3) = tokio::join!(
        async move {
            let data = file1.read(0, 16 * 1024).await.expect("Concurrent read 1 failed");
            (data.len(), "beginning")
        },
        async move {
            let offset = file_size / 3;
            let data = file2.read(offset as u64, 16 * 1024).await.expect("Concurrent read 2 failed");
            (data.len(), "middle")
        },
        async move {
            let offset = (file_size * 2) / 3;
            let data = file3.read(offset as u64, 16 * 1024).await.expect("Concurrent read 3 failed");
            (data.len(), "end")
        }
    );
    
    let concurrent_time = start.elapsed();
    
    println!("  🔀 Concurrent reads completed in {:?}:", concurrent_time);
    println!("     - {}: {} bytes", result1.1, result1.0);
    println!("     - {}: {} bytes", result2.1, result2.0);
    println!("     - {}: {} bytes", result3.1, result3.0);
    
    let final_stats = progressive_file.get_stats().await;
    println!("  📊 Final concurrent stats: chunks_loaded={}, total_time={:?}, avg_chunk_time={:?}",
            final_stats.chunks_loaded, final_stats.total_load_time, final_stats.avg_chunk_load_time);
    
    Ok(())
}

/// Create sample data with a recognizable pattern for testing
fn create_sample_data(size: usize, prefix: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    
    // Add a small header with the prefix
    let header = format!("{}\n", prefix);
    data.extend_from_slice(header.as_bytes());
    
    // Fill the rest with a pattern that makes it easy to verify reads
    for i in data.len()..size {
        data.push((i % 256) as u8);
    }
    
    data
}