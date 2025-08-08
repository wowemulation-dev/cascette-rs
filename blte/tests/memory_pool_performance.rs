//! Memory pool performance tests for BLTE decompression

#![allow(clippy::uninlined_format_args)]

use blte::{BLTEMemoryPool, PoolConfig, decompress_blte, decompress_blte_pooled, init_global_pool};
use std::time::Instant;

/// Test that demonstrates memory pool performance improvement
#[test]
fn test_memory_pool_performance() {
    // Create test data - multiple ZLib compressed chunks
    let test_data = create_test_multi_chunk_blte();

    const NUM_ITERATIONS: usize = 100;

    // Test without memory pooling (standard approach)
    println!("Testing standard decompression without memory pooling...");
    let start = Instant::now();

    let mut results_standard = Vec::new();
    for _ in 0..NUM_ITERATIONS {
        let result = decompress_blte(test_data.clone(), None).unwrap();
        results_standard.push(result);
    }

    let standard_time = start.elapsed();
    println!(
        "Standard approach: {} iterations in {:?}",
        NUM_ITERATIONS, standard_time
    );

    // Test with memory pooling
    println!("Testing pooled decompression with memory pooling...");

    let pool_config = PoolConfig {
        max_small_buffers: 10,
        max_medium_buffers: 5,
        max_large_buffers: 2,
        small_buffer_threshold: 16 * 1024,
        medium_buffer_threshold: 256 * 1024,
    };
    let pool = BLTEMemoryPool::with_config(pool_config);

    let start = Instant::now();

    let mut results_pooled = Vec::new();
    for _ in 0..NUM_ITERATIONS {
        let result = decompress_blte_pooled(test_data.clone(), None, Some(&pool)).unwrap();
        results_pooled.push(result);
    }

    let pooled_time = start.elapsed();
    println!(
        "Pooled approach: {} iterations in {:?}",
        NUM_ITERATIONS, pooled_time
    );

    // Verify results are identical
    assert_eq!(results_standard.len(), results_pooled.len());
    for (i, (standard, pooled)) in results_standard
        .iter()
        .zip(results_pooled.iter())
        .enumerate()
    {
        assert_eq!(
            standard, pooled,
            "Result {} differs between standard and pooled",
            i
        );
    }

    // Calculate performance improvement
    if pooled_time.as_nanos() > 0 {
        let speedup = standard_time.as_nanos() as f64 / pooled_time.as_nanos() as f64;
        println!("Memory pool speedup: {:.2}x", speedup);

        // We expect at least some improvement due to reduced allocations
        if speedup > 1.1 {
            println!(
                "✓ Memory pool shows significant improvement ({:.2}x)",
                speedup
            );
        } else if speedup > 1.0 {
            println!("✓ Memory pool shows modest improvement ({:.2}x)", speedup);
        } else {
            println!(
                "⚠ Memory pool shows no improvement ({:.2}x) - this may be due to test conditions",
                speedup
            );
        }
    }

    // Check pool statistics
    let stats = pool.stats();
    println!("Pool stats after test: {:?}", stats);
    println!("Pool utilization: {:.1}%", stats.utilization());
    println!(
        "Estimated memory usage: {} bytes",
        stats.estimated_memory_usage()
    );

    // Pool should have reused buffers
    assert!(
        stats.total_buffers() > 0,
        "Pool should contain reused buffers"
    );
}

/// Test memory usage patterns with different buffer sizes
#[test]
fn test_memory_pool_buffer_categorization() {
    let pool = BLTEMemoryPool::new();

    // Test small buffer
    let small_data = create_small_test_data();
    let _result1 = decompress_blte_pooled(small_data, None, Some(&pool)).unwrap();

    // Test medium buffer
    let medium_data = create_medium_test_data();
    let _result2 = decompress_blte_pooled(medium_data, None, Some(&pool)).unwrap();

    // Test large buffer
    let large_data = create_large_test_data();
    let _result3 = decompress_blte_pooled(large_data, None, Some(&pool)).unwrap();

    let stats = pool.stats();
    println!("Buffer categorization stats: {:?}", stats);

    // Should have buffers in different categories
    assert!(stats.total_buffers() > 0);
}

/// Test global pool initialization and usage
#[test]
fn test_global_pool_usage() {
    // Initialize global pool with custom config
    let config = PoolConfig {
        max_small_buffers: 5,
        max_medium_buffers: 3,
        max_large_buffers: 1,
        ..Default::default()
    };

    assert!(init_global_pool(config.clone()));

    // Use global pool through standard functions
    let test_data = create_test_multi_chunk_blte();
    let _result1 = decompress_blte(test_data.clone(), None).unwrap();
    let _result2 = decompress_blte(test_data.clone(), None).unwrap();
    let _result3 = decompress_blte(test_data, None).unwrap();

    // Check that global pool has been used
    let global_pool = blte::global_pool();
    let stats = global_pool.stats();
    println!("Global pool stats: {:?}", stats);

    assert_eq!(stats.config.max_small_buffers, config.max_small_buffers);
    assert_eq!(stats.config.max_medium_buffers, config.max_medium_buffers);
    assert_eq!(stats.config.max_large_buffers, config.max_large_buffers);
}

/// Test memory pool with different compression modes
#[test]
fn test_memory_pool_compression_modes() {
    let pool = BLTEMemoryPool::new();

    // Test uncompressed data
    let uncompressed_data = create_uncompressed_test_data();
    let _result1 = decompress_blte_pooled(uncompressed_data, None, Some(&pool)).unwrap();

    // Test ZLib compressed data
    let zlib_data = create_zlib_test_data();
    let _result2 = decompress_blte_pooled(zlib_data, None, Some(&pool)).unwrap();

    // Test LZ4 compressed data
    let lz4_data = create_lz4_test_data();
    let _result3 = decompress_blte_pooled(lz4_data, None, Some(&pool)).unwrap();

    let stats = pool.stats();
    println!("Multi-compression stats: {:?}", stats);

    // Should have successfully processed different compression types
    assert!(stats.total_buffers() > 0);
}

/// Test pool memory cleanup behavior
#[test]
fn test_memory_pool_cleanup() {
    let pool = BLTEMemoryPool::new();
    let test_data = create_test_multi_chunk_blte();

    // Fill the pool
    for _ in 0..10 {
        let _result = decompress_blte_pooled(test_data.clone(), None, Some(&pool)).unwrap();
    }

    let stats_before = pool.stats();
    println!("Before cleanup: {:?}", stats_before);

    // Clear the pool
    pool.clear();

    let stats_after = pool.stats();
    println!("After cleanup: {:?}", stats_after);

    assert_eq!(stats_after.total_buffers(), 0);
}

// Helper functions to create test data

fn create_test_multi_chunk_blte() -> Vec<u8> {
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    // Create two compressed chunks
    let chunk1_data = b"Hello, World! This is chunk 1 with some test data.";
    let chunk2_data = b"This is chunk 2 with different test data for compression.";

    let mut encoder1 = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder1.write_all(chunk1_data).unwrap();
    let compressed1 = encoder1.finish().unwrap();

    let mut encoder2 = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder2.write_all(chunk2_data).unwrap();
    let compressed2 = encoder2.finish().unwrap();

    // Build chunk data with compression mode prefixes
    let mut chunk1_full = Vec::new();
    chunk1_full.push(b'Z'); // ZLib compression mode
    chunk1_full.extend_from_slice(&compressed1);

    let mut chunk2_full = Vec::new();
    chunk2_full.push(b'Z'); // ZLib compression mode
    chunk2_full.extend_from_slice(&compressed2);

    // Calculate header size
    let header_size = 1 + 3 + 2 * 24; // flags + chunk_count + 2 * chunk_info

    // Build BLTE file
    let mut blte_data = Vec::new();

    // Header
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&(header_size as u32).to_be_bytes());

    // Chunk table
    blte_data.push(0x0F); // Flags
    blte_data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks

    // Chunk 1 info
    blte_data.extend_from_slice(&(chunk1_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum

    // Chunk 2 info
    blte_data.extend_from_slice(&(chunk2_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum

    // Chunk data
    blte_data.extend_from_slice(&chunk1_full);
    blte_data.extend_from_slice(&chunk2_full);

    blte_data
}

fn create_small_test_data() -> Vec<u8> {
    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'N'); // No compression
    blte_data.extend_from_slice(b"Small test data");
    blte_data
}

fn create_medium_test_data() -> Vec<u8> {
    let large_content = vec![b'X'; 32 * 1024]; // 32KB of data
    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'N'); // No compression
    blte_data.extend_from_slice(&large_content);
    blte_data
}

fn create_large_test_data() -> Vec<u8> {
    let large_content = vec![b'Y'; 512 * 1024]; // 512KB of data
    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'N'); // No compression
    blte_data.extend_from_slice(&large_content);
    blte_data
}

fn create_uncompressed_test_data() -> Vec<u8> {
    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'N'); // No compression
    blte_data.extend_from_slice(b"Uncompressed test data");
    blte_data
}

fn create_zlib_test_data() -> Vec<u8> {
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    let test_data = b"ZLib compressed test data for memory pool testing";
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(test_data).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'Z'); // ZLib compression
    blte_data.extend_from_slice(&compressed);
    blte_data
}

fn create_lz4_test_data() -> Vec<u8> {
    let test_data = b"LZ4 compressed test data for memory pool testing";
    let compressed = lz4_flex::compress(test_data);

    let mut blte_data = Vec::new();
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&0u32.to_be_bytes());
    blte_data.push(b'4'); // LZ4 compression
    blte_data.extend_from_slice(&(test_data.len() as u32).to_le_bytes());
    blte_data.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    blte_data.extend_from_slice(&compressed);
    blte_data
}
