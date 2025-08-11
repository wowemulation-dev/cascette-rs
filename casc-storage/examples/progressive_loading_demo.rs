//! Example demonstrating progressive file loading for large files
//!
//! This example shows how to use the progressive loading infrastructure
//! to efficiently handle large game files without loading them entirely into memory.

use casc_storage::types::CascConfig;
use casc_storage::{CascStorage, EKey, ProgressiveConfig, SizeHint};
use std::path::Path;
use std::time::Instant;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Path to WoW data directory (adjust as needed)
    let data_path =
        Path::new("/home/danielsreichenbach/Downloads/wow/1.13.2.31650.windows-win64/Data");

    if !data_path.exists() {
        warn!("Data path does not exist: {:?}", data_path);
        warn!("Please adjust the path to point to your WoW Data directory");
        return Ok(());
    }

    info!("Opening CASC storage at {:?}", data_path);
    let config = CascConfig {
        data_path: data_path.to_path_buf(),
        cache_size_mb: 100,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
        read_only: false,
    };
    let mut storage = CascStorage::new(config)?;

    // Configure progressive loading
    let progressive_config = ProgressiveConfig {
        chunk_size: 256 * 1024, // 256KB chunks
        max_prefetch_chunks: 4, // Prefetch up to 4 chunks ahead
        chunk_timeout: std::time::Duration::from_secs(30),
        use_predictive_prefetch: true, // Enable predictive prefetching
        min_progressive_size: 1024 * 1024, // Only use for files > 1MB
    };

    storage.init_progressive_loading(progressive_config.clone());
    info!("Progressive loading initialized");

    // Example: Find a large file in the storage
    // In a real scenario, you'd have specific EKeys for large game assets
    let files = storage.get_all_ekeys();

    // Find files that might be large (this is just for demonstration)
    let large_file_candidates: Vec<EKey> = files
        .into_iter()
        .take(10) // Just check first 10 files for demo
        .collect();

    if large_file_candidates.is_empty() {
        warn!("No files found in storage");
        return Ok(());
    }

    info!("Found {} file candidates", large_file_candidates.len());

    // Demonstrate progressive loading
    for (index, ekey) in large_file_candidates.iter().enumerate() {
        info!(
            "\n--- File {}/{} ---",
            index + 1,
            large_file_candidates.len()
        );
        info!("Processing file: {}", ekey);

        // For demo, we'll use an unknown size hint
        // In a real scenario, you might have this information from manifests
        let size_hint = SizeHint::Unknown;
        info!("Using size hint: {:?}", size_hint);

        // Check if progressive loading should be used
        if !size_hint.should_use_progressive(&progressive_config) {
            info!("File too small for progressive loading, using regular read");

            // Regular read for small files
            let start = Instant::now();
            match storage.read(ekey) {
                Ok(data) => {
                    info!(
                        "Regular read completed: {} bytes in {:?}",
                        data.len(),
                        start.elapsed()
                    );
                }
                Err(e) => {
                    warn!("Failed to read file: {}", e);
                }
            }
        } else {
            info!("Using progressive loading for large file");

            // Progressive read for large files
            let start = Instant::now();
            match storage.read_progressive(ekey, size_hint).await {
                Ok(progressive_file) => {
                    info!("Progressive file handle created");

                    // Read first chunk
                    let chunk_start = Instant::now();
                    match progressive_file.read(0, 1024).await {
                        Ok(_data) => {
                            info!(
                                "Read first 1KB in {:?} (chunk load time)",
                                chunk_start.elapsed()
                            );

                            // Demonstrate reading from different positions
                            if let SizeHint::Exact(size) = progressive_file.get_size_hint()
                                && size > 10240
                            {
                                // Read from middle of file
                                let middle_offset = size / 2;
                                let middle_start = Instant::now();
                                match progressive_file.read(middle_offset, 1024).await {
                                    Ok(_) => {
                                        info!(
                                            "Read 1KB from middle (offset {}) in {:?}",
                                            middle_offset,
                                            middle_start.elapsed()
                                        );
                                    }
                                    Err(e) => {
                                        warn!("Failed to read from middle: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read first chunk: {}", e);
                        }
                    }

                    // Get statistics
                    let stats = progressive_file.get_stats().await;
                    info!("Progressive loading statistics:");
                    info!("  Chunks loaded: {}", stats.chunks_loaded);
                    info!("  Bytes loaded: {}", stats.bytes_loaded);
                    info!("  Cache hits: {}", stats.cache_hits);
                    info!("  Cache misses: {}", stats.cache_misses);
                    info!("  Prefetch hits: {}", stats.prefetch_hits);
                    info!("  Average chunk load time: {:?}", stats.avg_chunk_load_time);
                    info!("  Total time: {:?}", start.elapsed());

                    // Check if file is fully loaded
                    if progressive_file.is_fully_loaded().await {
                        info!("File is now fully loaded in memory");
                    } else {
                        info!("File is partially loaded (progressive mode active)");
                    }
                }
                Err(e) => {
                    warn!("Failed to create progressive file handle: {}", e);
                }
            }
        }

        // Only process first few files for demo
        if index >= 2 {
            break;
        }
    }

    // Cleanup and show global statistics
    info!("\n--- Global Progressive Loading Statistics ---");
    let global_stats = storage.get_progressive_stats().await;
    for (ekey, stats) in global_stats {
        info!("File {}:", ekey);
        info!("  Total chunks: {}", stats.chunks_loaded);
        info!("  Total bytes: {}", stats.bytes_loaded);
        info!(
            "  Cache efficiency: {:.1}%",
            (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses).max(1) as f64)
                * 100.0
        );
        info!(
            "  Prefetch efficiency: {:.1}%",
            (stats.prefetch_hits as f64
                / (stats.prefetch_hits + stats.prefetch_misses).max(1) as f64)
                * 100.0
        );
    }

    // Cleanup inactive files
    storage.cleanup_progressive_files().await;
    info!("Cleaned up inactive progressive files");

    Ok(())
}
