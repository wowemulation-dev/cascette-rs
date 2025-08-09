//! Integration tests for resumable downloads

use tact_client::{DownloadProgress, HttpClient, ProtocolVersion, Region, ResumableDownload};
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn test_download_progress_creation() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let progress = DownloadProgress::new(
        "abcdef123456".to_string(),
        "cdn.example.com".to_string(),
        "/data".to_string(),
        target_file.clone(),
    );

    assert_eq!(progress.file_hash, "abcdef123456");
    assert_eq!(progress.cdn_host, "cdn.example.com");
    assert_eq!(progress.cdn_path, "/data");
    assert_eq!(progress.target_file, target_file);
    assert_eq!(progress.bytes_downloaded, 0);
    assert!(progress.total_size.is_none());
    assert!(!progress.is_complete);
}

#[tokio::test]
async fn test_progress_persistence_round_trip() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    // Create initial progress
    let mut progress = DownloadProgress::new(
        "testfilehash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file.clone(),
    );

    progress.total_size = Some(4096);
    progress.bytes_downloaded = 2048;

    // Save to disk
    progress.save_to_file().await.unwrap();
    assert!(progress.progress_file.exists());

    // Load from disk
    let loaded_progress = DownloadProgress::load_from_file(&progress.progress_file)
        .await
        .unwrap();

    assert_eq!(loaded_progress.file_hash, "testfilehash");
    assert_eq!(loaded_progress.cdn_host, "cdn.test.com");
    assert_eq!(loaded_progress.cdn_path, "/data");
    assert_eq!(loaded_progress.total_size, Some(4096));
    assert_eq!(loaded_progress.bytes_downloaded, 2048);
    assert!(!loaded_progress.is_complete);
}

#[tokio::test]
async fn test_progress_completion_percentage() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let mut progress = DownloadProgress::new(
        "hash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file,
    );

    // No total size - should return None
    assert!(progress.completion_percentage().is_none());

    // With total size
    progress.total_size = Some(1000);
    progress.bytes_downloaded = 0;
    assert_eq!(progress.completion_percentage(), Some(0.0));

    progress.bytes_downloaded = 250;
    assert_eq!(progress.completion_percentage(), Some(25.0));

    progress.bytes_downloaded = 500;
    assert_eq!(progress.completion_percentage(), Some(50.0));

    progress.bytes_downloaded = 1000;
    assert_eq!(progress.completion_percentage(), Some(100.0));
}

#[tokio::test]
async fn test_progress_string_formatting() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let mut progress = DownloadProgress::new(
        "hash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file,
    );

    // No total size
    progress.bytes_downloaded = 512;
    let progress_str = progress.progress_string();
    assert!(progress_str.contains("512 B"));

    // With total size
    progress.total_size = Some(1024);
    let progress_str = progress.progress_string();
    assert!(progress_str.contains("512 B"));
    assert!(progress_str.contains("1.00 KB"));
    assert!(progress_str.contains("50.0%"));
}

#[tokio::test]
async fn test_verify_existing_file() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let mut progress = DownloadProgress::new(
        "hash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file.clone(),
    );

    // No file exists
    assert!(!progress.verify_existing_file().await.unwrap());

    // Create a file with some content
    let test_data = b"Hello, World! This is test data for resume testing.";
    let mut file = File::create(&target_file).await.unwrap();
    file.write_all(test_data).await.unwrap();
    file.flush().await.unwrap();

    // Set expected total size
    progress.total_size = Some(test_data.len() as u64);
    progress.bytes_downloaded = test_data.len() as u64;

    // Should verify correctly
    assert!(progress.verify_existing_file().await.unwrap());

    // Wrong total size should fail verification
    progress.total_size = Some(1000);
    assert!(!progress.verify_existing_file().await.unwrap());
}

#[tokio::test]
async fn test_resumable_download_creation() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let progress = DownloadProgress::new(
        "hash123".to_string(),
        "cdn.example.com".to_string(),
        "/data".to_string(),
        target_file,
    );

    let client = HttpClient::new(Region::US, ProtocolVersion::V2).unwrap();
    let download = ResumableDownload::new(client, progress);

    assert_eq!(download.progress().file_hash, "hash123");
    assert_eq!(download.progress().bytes_downloaded, 0);
}

#[tokio::test]
async fn test_find_resumable_downloads() {
    let temp_dir = TempDir::new().unwrap();

    // Create some test progress files
    for i in 0..3 {
        let target_file = temp_dir.path().join(format!("file{i}.dat"));
        let mut progress = DownloadProgress::new(
            format!("hash{i}"),
            "cdn.test.com".to_string(),
            "/data".to_string(),
            target_file,
        );

        progress.bytes_downloaded = i * 1024;
        progress.total_size = Some(4096);

        // Mark one as complete
        if i == 1 {
            progress.is_complete = true;
        }

        progress.save_to_file().await.unwrap();
    }

    // Find incomplete downloads
    let downloads = tact_client::resumable::find_resumable_downloads(temp_dir.path())
        .await
        .unwrap();

    // Should find 2 incomplete downloads (i=0 and i=2)
    assert_eq!(downloads.len(), 2);

    // Verify the downloads found are not complete
    for download in &downloads {
        assert!(!download.is_complete);
    }
}

#[tokio::test]
async fn test_cleanup_old_progress_files() {
    let temp_dir = TempDir::new().unwrap();

    // Test cleanup on empty directory doesn't crash
    let cleaned = tact_client::resumable::cleanup_old_progress_files(temp_dir.path(), 1)
        .await
        .unwrap();
    assert_eq!(cleaned, 0);

    // Test cleanup on non-existent directory doesn't crash
    let non_existent = temp_dir.path().join("non_existent");
    let cleaned = tact_client::resumable::cleanup_old_progress_files(&non_existent, 1)
        .await
        .unwrap();
    assert_eq!(cleaned, 0);
}

#[tokio::test]
async fn test_download_cancel() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let progress = DownloadProgress::new(
        "hash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file,
    );

    // Save progress file
    progress.save_to_file().await.unwrap();
    assert!(progress.progress_file.exists());

    let client = HttpClient::new(Region::US, ProtocolVersion::V2).unwrap();
    let download = ResumableDownload::new(client, progress);

    // Cancel download
    download.cancel().await.unwrap();
    assert!(!download.progress().progress_file.exists());
}

#[tokio::test]
async fn test_byte_formatting() {
    use tact_client::resumable::*;

    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("test.dat");

    let mut progress = DownloadProgress::new(
        "hash".to_string(),
        "cdn.test.com".to_string(),
        "/data".to_string(),
        target_file,
    );

    // Test different byte sizes in progress string
    progress.bytes_downloaded = 512;
    assert!(progress.progress_string().contains("512 B"));

    progress.bytes_downloaded = 1536; // 1.5 KB
    let progress_str = progress.progress_string();
    assert!(progress_str.contains("1.50 KB") || progress_str.contains("1536 B"));

    progress.bytes_downloaded = 1048576; // 1 MB
    progress.total_size = Some(2097152); // 2 MB
    let progress_str = progress.progress_string();
    assert!(progress_str.contains("MB"));
    assert!(progress_str.contains("50.0%"));
}
