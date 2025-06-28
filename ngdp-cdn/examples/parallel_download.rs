//! Example of parallel downloading using ngdp-cdn

use ngdp_cdn::CdnClient;
use std::time::Instant;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create CDN client
    let client = CdnClient::builder()
        .max_retries(3)
        .pool_max_idle_per_host(50) // Increase pool size for parallel downloads
        .build()
        .expect("Failed to create CDN client");

    // Example: Download multiple files in parallel
    let cdn_host = "blzddist1-a.akamaihd.net";
    let path = "tpr/wow";

    // Some example hashes (these won't work without a real CDN)
    let hashes = vec![
        "2e9c1e3b5f5a0c9d9e8f1234567890ab".to_string(),
        "3fa2b4c6d7e8f9a0b1c2d3e4f5678901".to_string(),
        "4ab5c6d7e8f9a0b1c2d3e4f567890123".to_string(),
        "5bc6d7e8f9a0b1c2d3e4f56789012345".to_string(),
        "6cd7e8f9a0b1c2d3e4f567890123456a".to_string(),
    ];

    println!("📥 Downloading {} files in parallel...", hashes.len());

    // Method 1: Simple parallel download
    let start = Instant::now();
    let results = client
        .download_parallel(cdn_host, path, &hashes, Some(3))
        .await;
    let elapsed = start.elapsed();

    println!("⏱️  Completed in {elapsed:.2?}");
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(data) => println!("   ✅ Hash {}: {} bytes", i + 1, data.len()),
            Err(e) => println!("   ❌ Hash {}: {}", i + 1, e),
        }
    }

    println!("\n📥 Downloading with progress tracking...");

    // Method 2: Parallel download with progress callback
    let start = Instant::now();
    let results = client
        .download_parallel_with_progress(cdn_host, path, &hashes, Some(3), |completed, total| {
            println!(
                "   Progress: {}/{} ({:.0}%)",
                completed,
                total,
                (completed as f64 / total as f64) * 100.0
            );
        })
        .await;
    let elapsed = start.elapsed();

    println!("⏱️  Completed in {elapsed:.2?}");

    // Count successes and failures
    let successes = results.iter().filter(|r| r.is_ok()).count();
    let failures = results.iter().filter(|r| r.is_err()).count();

    println!("📊 Summary: {successes} succeeded, {failures} failed");

    // Example: Download specific file types in parallel
    println!("\n📥 Downloading data files in parallel...");

    let data_hashes = vec![
        "1234567890abcdef1234567890abcdef".to_string(),
        "abcdef1234567890abcdef1234567890".to_string(),
    ];

    let start = Instant::now();
    let data_results = client
        .download_data_parallel(cdn_host, path, &data_hashes, Some(5))
        .await;
    let elapsed = start.elapsed();

    println!("⏱️  Data downloads completed in {elapsed:.2?}");
    for (i, result) in data_results.iter().enumerate() {
        match result {
            Ok(data) => println!("   ✅ Data file {}: {} bytes", i + 1, data.len()),
            Err(e) => println!("   ❌ Data file {}: {}", i + 1, e),
        }
    }

    // Performance comparison
    println!("\n📊 Performance Comparison:");
    println!(
        "   Sequential download time (estimated): {:.2?}",
        std::time::Duration::from_secs(hashes.len() as u64 * 2)
    ); // Assume 2s per file
    println!("   Parallel download time (actual): {elapsed:.2?}");
    println!("   Speedup: ~{}x", hashes.len() as f64 / 3.0); // With concurrency of 3
}
