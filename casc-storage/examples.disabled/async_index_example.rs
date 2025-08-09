//! Example demonstrating async-first index operations
//!
//! This example shows how to use the new async index manager for:
//! - Parallel index loading
//! - Concurrent lookups
//! - Batch operations
//! - Background updates

use casc_storage::{CascStorage, types::CascConfig};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (tracing subscriber not available in examples)

    info!("CASC Async Index Operations Example");
    info!("====================================");

    // Create CASC storage configuration
    let config = CascConfig {
        data_path: PathBuf::from("./test-data"),
        max_archive_size: 1024 * 1024 * 1024, // 1GB
        use_memory_mapping: true,
        cache_size_mb: 512,
        read_only: false,
    };

    // Initialize storage
    let mut storage = CascStorage::new(config)?;
    info!("Storage initialized");

    // === Async Index Loading ===
    info!("\n1. Loading indices asynchronously...");
    let start = Instant::now();

    // Initialize async index manager (loads all indices in parallel)
    storage.init_async_indices().await?;

    let elapsed = start.elapsed();
    info!("Indices loaded in {:?}", elapsed);

    // Get statistics
    if let Some(stats) = storage.get_async_index_stats().await {
        info!("Index stats:");
        info!("  Total entries: {}", stats.total_entries);
        info!("  Total buckets: {}", stats.total_buckets);
        info!("  Cache size: {}", stats.cache_size);
    }

    // === Single Async Lookup ===
    info!("\n2. Testing single async lookup...");

    // Create a test EKey
    let mut test_key = [0u8; 16];
    test_key[0] = 0xAB;
    test_key[1] = 0xCD;
    let ekey = casc_storage::types::EKey::new(test_key);

    let start = Instant::now();
    let location = storage.lookup_async(&ekey).await;
    let elapsed = start.elapsed();

    match location {
        Some(loc) => {
            info!(
                "Found location for {}: archive={}, offset={}, size={}",
                ekey, loc.archive_id, loc.offset, loc.size
            );
        }
        None => {
            info!("No location found for {}", ekey);
        }
    }
    info!("Lookup completed in {:?}", elapsed);

    // === Batch Async Lookups ===
    info!("\n3. Testing batch async lookups...");

    // Create multiple test keys
    let mut test_keys = Vec::new();
    for i in 0..100 {
        let mut key = [0u8; 16];
        key[0] = (i % 256) as u8;
        key[1] = ((i / 256) % 256) as u8;
        test_keys.push(casc_storage::types::EKey::new(key));
    }

    let start = Instant::now();
    let results = storage.lookup_batch_async(&test_keys).await;
    let elapsed = start.elapsed();

    let found_count = results.iter().filter(|r| r.is_some()).count();
    info!(
        "Batch lookup results: {}/{} found",
        found_count,
        test_keys.len()
    );
    info!(
        "Batch lookup completed in {:?} ({:.2} lookups/ms)",
        elapsed,
        test_keys.len() as f64 / elapsed.as_millis() as f64
    );

    // === Cache Performance ===
    info!("\n4. Testing cache performance...");

    // First lookup (cache miss)
    let start = Instant::now();
    let _ = storage.lookup_async(&ekey).await;
    let first_lookup = start.elapsed();

    // Second lookup (cache hit)
    let start = Instant::now();
    let _ = storage.lookup_async(&ekey).await;
    let second_lookup = start.elapsed();

    info!("First lookup (cache miss): {:?}", first_lookup);
    info!("Second lookup (cache hit): {:?}", second_lookup);
    if second_lookup < first_lookup {
        let speedup = first_lookup.as_nanos() as f64 / second_lookup.as_nanos() as f64;
        info!("Cache speedup: {:.2}x", speedup);
    }

    // === Background Updates ===
    info!("\n5. Starting background index updates...");

    // Start background updates every 30 seconds
    storage
        .start_index_background_updates(Duration::from_secs(30))
        .await;
    info!("Background updates started (30 second interval)");

    // Simulate some work
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop background updates
    storage.stop_index_background_updates().await;
    info!("Background updates stopped");

    // === Parallel Performance Comparison ===
    info!("\n6. Comparing sync vs async performance...");

    // Load indices using traditional sync method
    let sync_storage = CascStorage::new(CascConfig {
        data_path: PathBuf::from("./test-data"),
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
        cache_size_mb: 512,
        read_only: false,
    })?;

    let start = Instant::now();
    sync_storage.load_indices()?;
    let sync_time = start.elapsed();

    // Compare with async loading
    let mut async_storage = CascStorage::new(CascConfig {
        data_path: PathBuf::from("./test-data"),
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
        cache_size_mb: 512,
        read_only: false,
    })?;

    let start = Instant::now();
    async_storage.init_async_indices().await?;
    let async_time = start.elapsed();

    info!("Sync loading time: {:?}", sync_time);
    info!("Async loading time: {:?}", async_time);
    if async_time < sync_time {
        let speedup = sync_time.as_millis() as f64 / async_time.as_millis() as f64;
        info!("Async speedup: {:.2}x faster", speedup);
    }

    // === Clear Cache ===
    info!("\n7. Clearing async index cache...");
    storage.clear_async_index_cache().await;
    info!("Cache cleared");

    if let Some(stats) = storage.get_async_index_stats().await {
        info!("Cache size after clear: {}", stats.cache_size);
    }

    info!("\n✅ Async index operations example completed successfully!");

    Ok(())
}
