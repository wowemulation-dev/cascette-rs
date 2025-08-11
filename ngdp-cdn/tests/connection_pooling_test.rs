//! Test CDN client connection pooling integration

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::bool_assert_comparison)]

use ngdp_cdn::CdnClient;
use std::time::Instant;
use tempfile::TempDir;

#[tokio::test]
async fn test_cdn_resumable_download_reuses_connections() {
    use tact_client::{PoolConfig, init_global_pool};

    let pool_config = PoolConfig::new()
        .with_max_idle_connections(Some(20))
        .with_max_idle_connections_per_host(10)
        .with_user_agent("CDNPoolTest/1.0".to_string());

    init_global_pool(pool_config);

    let mut client = CdnClient::new().expect("Failed to create CDN client");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // This tests that the TACT client is reused rather than recreated each time
    let start = Instant::now();

    const NUM_DOWNLOADS: usize = 5;
    let mut download_results = Vec::new();

    for i in 0..NUM_DOWNLOADS {
        let output_path = temp_dir.path().join(format!("test_file_{}.bin", i));

        let download_result = client.create_resumable_download(
            "level3.blizzard.com",                  // Example CDN host
            "tpr/wow",                              // Example path
            &format!("deadbeefcafebabe{:016x}", i), // Fake hash
            &output_path,
        );

        download_results.push(download_result);
    }

    let creation_time = start.elapsed();

    println!(
        "Created {} resumable downloads in {:?}",
        NUM_DOWNLOADS, creation_time
    );

    for (i, result) in download_results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "Download {} creation failed: {:?}",
            i,
            result
        );
    }

    // Creating resumable downloads should be very fast since we're reusing connections
    assert!(
        creation_time.as_millis() < 100,
        "Creating resumable downloads took too long: {:?}",
        creation_time
    );

    use tact_client::resumable::DownloadProgress;
    let target_file = temp_dir.path().join("resume_test.bin");
    let progress = DownloadProgress::new(
        "deadbeefcafebabe0000000000000000".to_string(),
        "level3.blizzard.com".to_string(),
        "tpr/wow".to_string(),
        target_file.clone(),
    );

    // Save progress file (uses target_file.with_extension("download") internally)
    progress
        .save_to_file()
        .await
        .expect("Failed to save progress file");

    let temp_progress = target_file.with_extension("download");

    let start = Instant::now();

    // Resume the download - should also be fast
    let resume_result = client.resume_download(&temp_progress).await;

    let resume_time = start.elapsed();

    println!("Resumed download in {:?}", resume_time);

    assert!(
        resume_result.is_ok(),
        "Resume download failed: {:?}",
        resume_result
    );
    assert!(
        resume_time.as_millis() < 50,
        "Resuming download took too long: {:?}",
        resume_time
    );

    println!("✓ CDN client connection pooling test completed successfully");
}

#[tokio::test]
async fn test_cdn_client_tact_client_sharing() {
    let mut client = CdnClient::new().expect("Failed to create CDN client");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    let file1 = temp_dir.path().join("file1.bin");
    let file2 = temp_dir.path().join("file2.bin");

    let download1 = client.create_resumable_download(
        "level3.blizzard.com",
        "tpr/wow",
        "deadbeefcafebabe1111111111111111",
        &file1,
    );

    let download2 = client.create_resumable_download(
        "level3.blizzard.com",
        "tpr/wow",
        "deadbeefcafebabe2222222222222222",
        &file2,
    );

    assert!(download1.is_ok(), "First download creation failed");
    assert!(download2.is_ok(), "Second download creation failed");

    // Both downloads should be created successfully, reusing the same TACT client internally
    let _resumable1 = download1.unwrap();
    let _resumable2 = download2.unwrap();

    println!("✓ CDN client TACT client sharing test completed");
}

#[test]
fn test_cdn_client_builder_with_pooling() {
    let client = CdnClient::builder()
        .max_retries(5)
        .initial_backoff_ms(200)
        .user_agent("TestBuilder/1.0")
        .build()
        .expect("Failed to build CDN client");

    // The actual pooling benefit is tested in integration tests
    assert_eq!(format!("{:?}", client).contains("CdnClient"), true);

    println!("✓ CDN client builder test completed");
}
