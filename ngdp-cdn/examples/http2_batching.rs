//! Example demonstrating HTTP/2 request batching for CDN downloads

use ngdp_cdn::CdnClient;
use std::time::Instant;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debugging
    use tracing_subscriber::{EnvFilter, fmt};

    fmt()
        .with_env_filter(EnvFilter::new("ngdp_cdn=debug,tact_client=debug"))
        .init();

    println!("HTTP/2 Request Batching Example");
    println!("================================");

    let mut client = CdnClient::builder()
        .max_retries(3)
        .pool_max_idle_per_host(50) // Higher connection pool for batching
        .build()?;

    // Example: Download multiple game assets
    let asset_hashes = vec![
        "abcd1234567890ef".to_string(),
        "1234567890abcdef".to_string(),
        "567890abcdef1234".to_string(),
        "90abcdef12345678".to_string(),
        "cdef123456789012".to_string(),
        "ef123456789012ab".to_string(),
        "23456789012abcde".to_string(),
        "456789012abcdef1".to_string(),
        "6789012abcdef123".to_string(),
        "89012abcdef12345".to_string(),
    ];

    println!(
        "\nDownloading {} files using different methods...",
        asset_hashes.len()
    );

    // Method 1: Traditional parallel downloads
    println!("\n1. Traditional Parallel Downloads:");
    let start = Instant::now();
    let parallel_results = client
        .download_data_parallel("example-cdn.com", "wow/data", &asset_hashes, Some(5))
        .await;
    let parallel_duration = start.elapsed();

    println!("   - Duration: {:?}", parallel_duration);
    println!(
        "   - Success: {}/{}",
        parallel_results.iter().filter(|r| r.is_ok()).count(),
        parallel_results.len()
    );

    // Method 2: HTTP/2 Request Batching (optimized)
    println!("\n2. HTTP/2 Request Batching:");
    let start = Instant::now();
    let batch_results = client
        .download_data_batched("example-cdn.com", "wow/data", &asset_hashes)
        .await;
    let batch_duration = start.elapsed();

    println!("   - Duration: {:?}", batch_duration);
    println!(
        "   - Success: {}/{}",
        batch_results.iter().filter(|r| r.is_ok()).count(),
        batch_results.len()
    );

    // Performance comparison
    if parallel_duration > batch_duration {
        let improvement = parallel_duration.as_millis() as f64 / batch_duration.as_millis() as f64;
        println!("   - HTTP/2 batching is {:.2}x faster!", improvement);
    } else if batch_duration > parallel_duration {
        let overhead = batch_duration.as_millis() as f64 / parallel_duration.as_millis() as f64;
        println!(
            "   - HTTP/2 batching has {:.2}x overhead (possibly due to network conditions)",
            overhead
        );
    } else {
        println!("   - Performance is equivalent");
    }

    // Show detailed batch statistics
    if let Some(stats) = client.get_batch_stats().await {
        println!("\nHTTP/2 Batching Statistics:");
        println!("   - Batches processed: {}", stats.batches_processed);
        println!("   - Total requests: {}", stats.requests_processed);
        println!("   - Average batch size: {:.1}", stats.avg_batch_size);
        println!("   - Average batch time: {:?}", stats.avg_batch_time);
        println!("   - HTTP/2 connections: {}", stats.http2_connections);
        println!("   - Total processing time: {:?}", stats.total_batch_time);
    }

    // Example with different file types
    println!("\nBatching Different File Types:");
    println!("==============================");

    let config_hashes = vec![
        "config1234567890".to_string(),
        "config2345678901".to_string(),
        "config3456789012".to_string(),
    ];

    let patch_hashes = vec![
        "patch1234567890a".to_string(),
        "patch2345678901b".to_string(),
    ];

    // Download config files using batching
    println!("\nDownloading config files...");
    let start = Instant::now();
    let config_results = client
        .download_config_batched("example-cdn.com", "wow", &config_hashes)
        .await;
    println!("Config download time: {:?}", start.elapsed());
    println!(
        "Config results: {}/{} successful",
        config_results.iter().filter(|r| r.is_ok()).count(),
        config_results.len()
    );

    // Download patch files using batching
    println!("\nDownloading patch files...");
    let start = Instant::now();
    let patch_results = client
        .download_patch_batched("example-cdn.com", "wow", &patch_hashes)
        .await;
    println!("Patch download time: {:?}", start.elapsed());
    println!(
        "Patch results: {}/{} successful",
        patch_results.iter().filter(|r| r.is_ok()).count(),
        patch_results.len()
    );

    // Show final statistics after all operations
    if let Some(final_stats) = client.get_batch_stats().await {
        println!("\nFinal Statistics After All Operations:");
        println!("   - Total batches: {}", final_stats.batches_processed);
        println!("   - Total requests: {}", final_stats.requests_processed);
        println!(
            "   - Overall avg batch size: {:.1}",
            final_stats.avg_batch_size
        );
        println!(
            "   - Overall avg batch time: {:?}",
            final_stats.avg_batch_time
        );
        println!(
            "   - Total HTTP/2 connections used: {}",
            final_stats.http2_connections
        );

        if final_stats.http2_connections > 0 {
            println!("   ✓ HTTP/2 multiplexing was successfully used");
        } else {
            println!("   ⚠ HTTP/2 connections not detected (may be fallback HTTP/1.1)");
        }
    }

    // Demonstration of concurrent batching
    println!("\nConcurrent Batching Example:");
    println!("=============================");

    let batch1 = vec!["concurrent_a1".to_string(), "concurrent_a2".to_string()];
    let batch2 = vec!["concurrent_b1".to_string(), "concurrent_b2".to_string()];
    let batch3 = vec!["concurrent_c1".to_string(), "concurrent_c2".to_string()];

    let start = Instant::now();
    let (results1, results2, results3) = tokio::join!(
        client.download_data_batched("example-cdn.com", "wow/data", &batch1),
        client.download_config_batched("example-cdn.com", "wow", &batch2),
        client.download_patch_batched("example-cdn.com", "wow", &batch3)
    );
    let concurrent_duration = start.elapsed();

    println!(
        "Concurrent batching completed in: {:?}",
        concurrent_duration
    );
    println!(
        "Results: {}/{}, {}/{}, {}/{}",
        results1.iter().filter(|r| r.is_ok()).count(),
        results1.len(),
        results2.iter().filter(|r| r.is_ok()).count(),
        results2.len(),
        results3.iter().filter(|r| r.is_ok()).count(),
        results3.len(),
    );

    println!("\nExample completed!");
    println!("\nKey Benefits of HTTP/2 Request Batching:");
    println!("- Reduced connection overhead through multiplexing");
    println!("- Lower latency by reusing established connections");
    println!("- Better CDN server resource utilization");
    println!("- Automatic fallback to traditional parallel downloads if needed");
    println!("- Comprehensive performance statistics for monitoring");

    Ok(())
}
