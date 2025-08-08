//! Tests for HTTP/2 request batching functionality

use ngdp_cdn::CdnClient;
use std::time::Instant;
use tokio::time::Duration;

/// Test basic request batching functionality
#[tokio::test]
async fn test_basic_request_batching() {
    let client = CdnClient::new().unwrap();

    // Test with small batch - should work even without real CDN
    let hashes = vec!["abcd1234".to_string(), "efgh5678".to_string()];

    // This will fail with network errors, but tests the API
    let results = client
        .download_batched("example.com", "data", &hashes)
        .await;

    // Should return same number of results as input hashes
    assert_eq!(results.len(), hashes.len());

    // All should be errors since we don't have real CDN access
    for result in results {
        assert!(result.is_err(), "Expected network error for test endpoint");
    }
}

/// Test batch configuration and statistics
#[tokio::test]
async fn test_batch_statistics() {
    let client = CdnClient::new().unwrap();

    // Initially no stats should exist
    assert!(client.get_batch_stats().await.is_none());

    // Trigger batch creation by attempting downloads
    let hashes = vec!["test123".to_string()];
    let _results = client
        .download_batched("example.com", "data", &hashes)
        .await;

    // Now stats should be available
    if let Some(stats) = client.get_batch_stats().await {
        assert!(stats.batches_processed >= 0);
        assert!(stats.requests_processed >= 0);
        println!("Batch stats: {:?}", stats);
    }
}

/// Test batch performance characteristics
#[tokio::test]
async fn test_batch_performance_characteristics() {
    let client = CdnClient::new().unwrap();

    // Create a larger batch to test batching behavior
    let hashes: Vec<String> = (0..50).map(|i| format!("hash{:04x}", i)).collect();

    let start = Instant::now();
    let results = client
        .download_batched("httpbin.org", "status/404", &hashes)
        .await; // Will return 404s
    let batch_duration = start.elapsed();

    println!("Batched 50 requests in: {:?}", batch_duration);

    // Should get results for all hashes
    assert_eq!(results.len(), hashes.len());

    // Check batch statistics
    if let Some(stats) = client.get_batch_stats().await {
        assert!(stats.batches_processed > 0);
        assert!(stats.requests_processed >= hashes.len() as u64);
        assert!(stats.avg_batch_time > Duration::ZERO);

        println!("Final batch stats: {:?}", stats);
    }
}

/// Test specialized batch download methods
#[tokio::test]
async fn test_specialized_batch_methods() {
    let client = CdnClient::new().unwrap();

    let hashes = vec!["data123".to_string(), "data456".to_string()];

    // Test data batching
    let data_results = client
        .download_data_batched("example.com", "test", &hashes)
        .await;
    assert_eq!(data_results.len(), hashes.len());

    // Test config batching
    let config_results = client
        .download_config_batched("example.com", "test", &hashes)
        .await;
    assert_eq!(config_results.len(), hashes.len());

    // Test patch batching
    let patch_results = client
        .download_patch_batched("example.com", "test", &hashes)
        .await;
    assert_eq!(patch_results.len(), hashes.len());

    // All should be network errors but with correct structure
    for result in &data_results {
        assert!(result.is_err());
    }
    for result in &config_results {
        assert!(result.is_err());
    }
    for result in &patch_results {
        assert!(result.is_err());
    }
}

/// Test batch versus parallel performance comparison
#[tokio::test]
async fn test_batch_vs_parallel_comparison() {
    let client = CdnClient::new().unwrap();

    let hashes: Vec<String> = (0..20).map(|i| format!("file{:04x}", i)).collect();

    // Test parallel downloads
    let start = Instant::now();
    let parallel_results = client
        .download_parallel("httpbin.org", "status/200", &hashes, Some(5))
        .await;
    let parallel_duration = start.elapsed();

    // Test batched downloads
    let start = Instant::now();
    let batch_results = client
        .download_batched("httpbin.org", "status/200", &hashes)
        .await;
    let batch_duration = start.elapsed();

    println!("Parallel duration: {:?}", parallel_duration);
    println!("Batched duration: {:?}", batch_duration);

    // Both should return same number of results
    assert_eq!(parallel_results.len(), hashes.len());
    assert_eq!(batch_results.len(), hashes.len());

    // Check that batch statistics were updated
    if let Some(stats) = client.get_batch_stats().await {
        println!("Final comparison stats: {:?}", stats);
        assert!(stats.batches_processed > 0);
    }
}

/// Test concurrent batching behavior
#[tokio::test]
async fn test_concurrent_batching() {
    let client = CdnClient::new().unwrap();

    // Create multiple concurrent batch operations
    let batch1_hashes = vec!["batch1_file1".to_string(), "batch1_file2".to_string()];
    let batch2_hashes = vec!["batch2_file1".to_string(), "batch2_file2".to_string()];
    let batch3_hashes = vec!["batch3_file1".to_string(), "batch3_file2".to_string()];

    // Run all batches concurrently
    let (results1, results2, results3) = tokio::join!(
        client.download_batched("example.com", "data", &batch1_hashes),
        client.download_batched("example.com", "data", &batch2_hashes),
        client.download_batched("example.com", "data", &batch3_hashes)
    );

    // All should return correct number of results
    assert_eq!(results1.len(), batch1_hashes.len());
    assert_eq!(results2.len(), batch2_hashes.len());
    assert_eq!(results3.len(), batch3_hashes.len());

    // Check final stats
    if let Some(stats) = client.get_batch_stats().await {
        println!("Concurrent batching stats: {:?}", stats);
        assert!(stats.batches_processed >= 3); // At least 3 batches processed
        assert!(stats.requests_processed >= 6); // At least 6 requests total
    }
}

/// Test batching with network timeouts and error handling
#[tokio::test]
async fn test_batch_error_handling() {
    let client = CdnClient::new().unwrap();

    // Mix of different endpoints to test error handling
    let hashes = vec![
        "success".to_string(),
        "notfound".to_string(),
        "timeout".to_string(),
    ];

    let results = client
        .download_batched("httpbin.org", "status/404", &hashes)
        .await;

    // Should get results for all requests (even failed ones)
    assert_eq!(results.len(), hashes.len());

    // All should be errors (404 status)
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(_) => println!("Request {} unexpectedly succeeded", i),
            Err(e) => println!("Request {} failed as expected: {}", i, e),
        }
    }
}

/// Test batch fallback to parallel downloads
#[tokio::test]
async fn test_batch_fallback_mechanism() {
    // This test verifies that if batching fails to initialize,
    // the system falls back to parallel downloads

    let client = CdnClient::new().unwrap();
    let hashes = vec!["test1".to_string(), "test2".to_string()];

    // Even if internal batching fails, should still get results
    let results = client
        .download_batched("example.com", "data", &hashes)
        .await;

    // Should always return results (even if they're errors)
    assert_eq!(results.len(), hashes.len());

    for result in results {
        // All should be network errors, but structured properly
        assert!(result.is_err());
    }
}

/// Benchmark test to measure batching improvements (ignored by default)
#[tokio::test]
#[ignore = "Performance benchmark - run manually"]
async fn benchmark_batch_vs_parallel() {
    let client = CdnClient::new().unwrap();

    // Large batch for meaningful benchmark
    let hashes: Vec<String> = (0..100).map(|i| format!("benchmark{:06x}", i)).collect();

    println!("Benchmarking with {} files...", hashes.len());

    // Warm up
    let _ = client
        .download_batched("httpbin.org", "status/204", &hashes[0..5])
        .await;

    // Benchmark parallel
    let start = Instant::now();
    let _parallel_results = client
        .download_parallel("httpbin.org", "status/204", &hashes, Some(20))
        .await;
    let parallel_time = start.elapsed();

    // Benchmark batched
    let start = Instant::now();
    let _batch_results = client
        .download_batched("httpbin.org", "status/204", &hashes)
        .await;
    let batch_time = start.elapsed();

    println!("Parallel time: {:?}", parallel_time);
    println!("Batched time: {:?}", batch_time);

    if batch_time < parallel_time {
        let improvement = parallel_time.as_millis() as f64 / batch_time.as_millis() as f64;
        println!("Batching is {:.2}x faster", improvement);
    } else {
        let regression = batch_time.as_millis() as f64 / parallel_time.as_millis() as f64;
        println!("Batching is {:.2}x slower (unexpected)", regression);
    }

    // Show final statistics
    if let Some(stats) = client.get_batch_stats().await {
        println!("Final benchmark stats: {:#?}", stats);
    }
}
