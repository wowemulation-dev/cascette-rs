//! Example demonstrating resumable download functionality
//!
//! This example shows how to download a file with resume capability,
//! including how to handle interruptions and resume from the last
//! successfully downloaded byte.

use std::path::PathBuf;
use tact_client::{DownloadProgress, HttpClient, ProtocolVersion, Region, ResumableDownload};
use tokio::signal;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().init();

    // Example file to download (using a hypothetical CDN file)
    let file_hash = "1a2b3c4d5e6f7890abcdef1234567890abcdef12";
    let cdn_host = "level3.blizzard.com";
    let cdn_path = "tpr/wow/config";
    let output_file = PathBuf::from("downloaded_file.bin");

    info!("Starting resumable download example");
    info!("File hash: {}", file_hash);
    info!("Output: {:?}", output_file);

    // Create HTTP client with retry configuration
    let client = HttpClient::new(Region::US, ProtocolVersion::V2)?
        .with_max_retries(3)
        .with_initial_backoff_ms(500)
        .with_user_agent("resumable-download-example/1.0");

    // Check for existing progress
    let progress = if output_file.with_extension("download").exists() {
        info!("Found existing download progress, attempting to resume...");
        match DownloadProgress::load_from_file(&output_file.with_extension("download")).await {
            Ok(progress) => {
                info!("Loaded progress: {}", progress.progress_string());
                progress
            }
            Err(e) => {
                warn!("Failed to load progress file, starting fresh: {}", e);
                DownloadProgress::new(
                    file_hash.to_string(),
                    cdn_host.to_string(),
                    cdn_path.to_string(),
                    output_file.clone(),
                )
            }
        }
    } else {
        info!("Starting new download");
        DownloadProgress::new(
            file_hash.to_string(),
            cdn_host.to_string(),
            cdn_path.to_string(),
            output_file.clone(),
        )
    };

    // Create resumable download
    let mut download = ResumableDownload::new(client, progress);

    // Set up Ctrl+C handler for graceful cancellation
    let mut download_handle = tokio::spawn(async move {
        match download.start_or_resume().await {
            Ok(()) => {
                info!("Download completed successfully!");
                download.cleanup_completed().await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            }
            Err(e) => {
                error!("Download failed: {}", e);
                Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    });

    let ctrl_c = tokio::spawn(async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        warn!("Ctrl+C received, download will be paused...");
    });

    // Wait for either download completion or Ctrl+C
    tokio::select! {
        result = &mut download_handle => {
            match result {
                Ok(Ok(())) => info!("Download finished successfully"),
                Ok(Err(e)) => error!("Download error: {}", e),
                Err(e) => error!("Task error: {}", e),
            }
        }
        _ = ctrl_c => {
            info!("Download interrupted, progress saved for later resume");
            download_handle.abort();
        }
    }

    // Show how to find and clean up resumable downloads
    info!("Checking for resumable downloads in current directory...");
    let current_dir = std::env::current_dir()?;
    let resumable_downloads =
        tact_client::resumable::find_resumable_downloads(&current_dir).await?;

    if resumable_downloads.is_empty() {
        info!("No resumable downloads found");
    } else {
        info!("Found {} resumable download(s):", resumable_downloads.len());
        for download in &resumable_downloads {
            info!("  - {}: {}", download.file_hash, download.progress_string());
        }
    }

    // Clean up old progress files (older than 24 hours)
    let cleaned = tact_client::resumable::cleanup_old_progress_files(&current_dir, 24).await?;
    if cleaned > 0 {
        info!("Cleaned up {} old progress file(s)", cleaned);
    }

    Ok(())
}

/// Example of programmatic download management
#[allow(dead_code)]
async fn download_with_progress_callback() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;

    let file_hash = "example_file_hash";
    let cdn_host = "cdn.example.com";
    let cdn_path = "/data";
    let output_file = PathBuf::from("example_file.dat");

    let client = HttpClient::new(Region::US, ProtocolVersion::V2)?;
    let progress = DownloadProgress::new(
        file_hash.to_string(),
        cdn_host.to_string(),
        cdn_path.to_string(),
        output_file,
    );

    let mut download = ResumableDownload::new(client, progress);

    // Start download in a separate task
    let download_task = tokio::spawn(async move { download.start_or_resume().await });

    // Monitor progress (in a real application, you'd get progress updates differently)
    let progress_monitor = tokio::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            // In a real implementation, you'd access progress through a shared state
            info!("Progress monitoring tick...");
        }
    });

    // Wait for download to complete
    let result = tokio::select! {
        result = download_task => {
            progress_monitor.abort();
            result?
        }
        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            info!("Download timeout for demonstration");
            progress_monitor.abort();
            return Ok(());
        }
    };

    match result {
        Ok(()) => info!("Download completed!"),
        Err(e) => error!("Download failed: {}", e),
    }

    Ok(())
}

/// Example of batch download management
#[allow(dead_code)]
async fn batch_resumable_downloads() -> Result<(), Box<dyn std::error::Error>> {
    let files_to_download = vec![
        ("hash1", "file1.dat"),
        ("hash2", "file2.dat"),
        ("hash3", "file3.dat"),
    ];

    let client = HttpClient::new(Region::US, ProtocolVersion::V2)?
        .with_max_retries(3)
        .with_initial_backoff_ms(1000);

    let mut download_tasks = Vec::new();

    for (hash, filename) in files_to_download {
        let client_clone = client.clone();
        let output_file = PathBuf::from(filename);

        let progress = DownloadProgress::new(
            hash.to_string(),
            "cdn.example.com".to_string(),
            "/data".to_string(),
            output_file,
        );

        let mut download = ResumableDownload::new(client_clone, progress);

        let task = tokio::spawn(async move {
            info!("Starting download for {}", hash);
            match download.start_or_resume().await {
                Ok(()) => {
                    info!("Completed download for {}", hash);
                    download.cleanup_completed().await?;
                    Ok::<String, Box<dyn std::error::Error + Send + Sync>>(hash.to_string())
                }
                Err(e) => {
                    error!("Failed download for {}: {}", hash, e);
                    Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        });

        download_tasks.push(task);
    }

    // Wait for all downloads to complete
    let results = futures_util::future::join_all(download_tasks).await;

    let mut completed = 0;
    let mut failed = 0;

    for result in results {
        match result {
            Ok(Ok(hash)) => {
                info!("Successfully completed: {}", hash);
                completed += 1;
            }
            Ok(Err(e)) => {
                error!("Download error: {}", e);
                failed += 1;
            }
            Err(e) => {
                error!("Task error: {}", e);
                failed += 1;
            }
        }
    }

    info!(
        "Batch download complete: {} succeeded, {} failed",
        completed, failed
    );
    Ok(())
}
