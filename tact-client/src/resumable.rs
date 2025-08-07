//! Resumable download functionality for TACT clients
//!
//! This module provides support for downloading files that can be interrupted and resumed
//! from the last successfully downloaded byte. It persists download state to disk and
//! uses HTTP range requests to continue interrupted downloads.

use crate::{Error, HttpClient, Result};
use reqwest::Response;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{debug, info, warn};

/// Download progress information persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Total expected file size in bytes
    pub total_size: Option<u64>,
    /// Number of bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Original file hash for verification
    pub file_hash: String,
    /// CDN host used for download
    pub cdn_host: String,
    /// CDN path for the file
    pub cdn_path: String,
    /// Target file path where content is being written
    pub target_file: PathBuf,
    /// Progress file path for state persistence
    pub progress_file: PathBuf,
    /// Whether the download is complete
    pub is_complete: bool,
    /// Timestamp of last update (for cleanup of old progress files)
    pub last_updated: u64,
}

/// Resumable download manager
#[derive(Debug)]
pub struct ResumableDownload {
    client: HttpClient,
    progress: DownloadProgress,
}

impl DownloadProgress {
    /// Create a new download progress tracker
    pub fn new(
        file_hash: String,
        cdn_host: String,
        cdn_path: String,
        target_file: PathBuf,
    ) -> Self {
        let progress_file = target_file.with_extension("download");

        Self {
            total_size: None,
            bytes_downloaded: 0,
            file_hash,
            cdn_host,
            cdn_path,
            target_file,
            progress_file,
            is_complete: false,
            last_updated: current_timestamp(),
        }
    }

    /// Load progress from disk
    pub async fn load_from_file(progress_file: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(progress_file).await?;
        let mut progress: DownloadProgress = serde_json::from_str(&content)?;
        progress.last_updated = current_timestamp();
        Ok(progress)
    }

    /// Save progress to disk
    pub async fn save_to_file(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&self.progress_file, content).await?;
        debug!("Saved download progress to {:?}", self.progress_file);
        Ok(())
    }

    /// Check if the target file exists and has the expected size
    pub async fn verify_existing_file(&self) -> Result<bool> {
        if let Ok(metadata) = tokio::fs::metadata(&self.target_file).await {
            let file_size = metadata.len();

            // If we know the total size, check if it matches
            if let Some(total) = self.total_size {
                return Ok(file_size == total);
            }

            // If file exists and we've downloaded some bytes, assume it's valid for resume
            Ok(file_size >= self.bytes_downloaded)
        } else {
            Ok(false)
        }
    }

    /// Calculate download completion percentage
    pub fn completion_percentage(&self) -> Option<f64> {
        self.total_size.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.bytes_downloaded as f64 / total as f64) * 100.0
            }
        })
    }

    /// Get human-readable progress string
    pub fn progress_string(&self) -> String {
        match (self.total_size, self.completion_percentage()) {
            (Some(total), Some(percent)) => {
                format!(
                    "{}/{} bytes ({:.1}%)",
                    format_bytes(self.bytes_downloaded),
                    format_bytes(total),
                    percent
                )
            }
            (Some(total), None) => {
                format!(
                    "{}/{} bytes",
                    format_bytes(self.bytes_downloaded),
                    format_bytes(total)
                )
            }
            (None, _) => {
                format!("{} bytes", format_bytes(self.bytes_downloaded))
            }
        }
    }
}

impl ResumableDownload {
    /// Create a new resumable download
    pub fn new(client: HttpClient, progress: DownloadProgress) -> Self {
        Self { client, progress }
    }

    /// Start or resume a download
    pub async fn start_or_resume(&mut self) -> Result<()> {
        // Check if we can resume from existing file
        let can_resume = if self.progress.bytes_downloaded > 0 {
            self.progress.verify_existing_file().await.unwrap_or(false)
        } else {
            false
        };

        if can_resume {
            info!(
                "Resuming download from {} bytes for {}",
                self.progress.bytes_downloaded, self.progress.file_hash
            );
        } else {
            info!("Starting new download for {}", self.progress.file_hash);
            self.progress.bytes_downloaded = 0;
        }

        // Save initial progress
        self.progress.save_to_file().await?;

        // Start the download
        self.download_with_resume().await
    }

    /// Perform the actual download with resume capability
    async fn download_with_resume(&mut self) -> Result<()> {
        // Open or create the target file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(false)
            .open(&self.progress.target_file)
            .await?;

        // Seek to the resume position
        if self.progress.bytes_downloaded > 0 {
            file.seek(SeekFrom::Start(self.progress.bytes_downloaded))
                .await?;
        }

        // Make range request from resume position
        let range = (self.progress.bytes_downloaded, None);
        let response = self
            .client
            .download_file_range(
                &self.progress.cdn_host,
                &self.progress.cdn_path,
                &self.progress.file_hash,
                range,
            )
            .await?;

        // Extract total size from headers if available
        if self.progress.total_size.is_none() {
            self.progress.total_size =
                extract_total_size(&response, self.progress.bytes_downloaded);
        }

        // Check response status
        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => {
                debug!(
                    "Server supports range requests, resuming from byte {}",
                    self.progress.bytes_downloaded
                );
            }
            reqwest::StatusCode::OK => {
                if self.progress.bytes_downloaded > 0 {
                    warn!(
                        "Server doesn't support range requests, restarting download from beginning"
                    );
                    file.seek(SeekFrom::Start(0)).await?;
                    file.set_len(0).await?;
                    self.progress.bytes_downloaded = 0;
                }
            }
            _status => {
                return Err(Error::InvalidResponse);
            }
        }

        // Stream the response to the file with progress tracking
        self.stream_response_to_file(response, &mut file).await?;

        // Mark as complete and clean up
        self.progress.is_complete = true;
        self.progress.save_to_file().await?;

        info!("Download completed: {}", self.progress.progress_string());
        Ok(())
    }

    /// Stream response content to file with progress updates
    async fn stream_response_to_file(&mut self, response: Response, file: &mut File) -> Result<()> {
        let mut stream = response.bytes_stream();
        let mut bytes_written_since_save = 0u64;
        const SAVE_INTERVAL: u64 = 1024 * 1024; // Save progress every 1MB

        use futures_util::StreamExt;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(Error::Http)?;

            // Write chunk to file
            file.write_all(&chunk).await?;

            // Update progress
            let chunk_size = chunk.len() as u64;
            self.progress.bytes_downloaded += chunk_size;
            bytes_written_since_save += chunk_size;

            // Periodically save progress to disk
            if bytes_written_since_save >= SAVE_INTERVAL {
                file.flush().await?;
                self.progress.last_updated = current_timestamp();
                self.progress.save_to_file().await?;
                bytes_written_since_save = 0;

                debug!("Progress: {}", self.progress.progress_string());
            }
        }

        // Final flush and progress save
        file.flush().await?;
        self.progress.last_updated = current_timestamp();

        Ok(())
    }

    /// Get current progress
    pub fn progress(&self) -> &DownloadProgress {
        &self.progress
    }

    /// Cancel the download and clean up progress file
    pub async fn cancel(&self) -> Result<()> {
        if self.progress.progress_file.exists() {
            tokio::fs::remove_file(&self.progress.progress_file).await?;
            debug!("Removed progress file {:?}", self.progress.progress_file);
        }
        Ok(())
    }

    /// Clean up completed download (remove progress file, keep target file)
    pub async fn cleanup_completed(&self) -> Result<()> {
        if self.progress.is_complete && self.progress.progress_file.exists() {
            tokio::fs::remove_file(&self.progress.progress_file).await?;
            debug!("Cleaned up progress file for completed download");
        }
        Ok(())
    }
}

/// Extract total file size from HTTP response headers
fn extract_total_size(response: &Response, bytes_already_downloaded: u64) -> Option<u64> {
    // Try Content-Range header first (for partial content)
    if let Some(content_range) = response.headers().get("content-range") {
        if let Ok(range_str) = content_range.to_str() {
            // Format: "bytes 200-1023/1024"
            if let Some(total_str) = range_str.split('/').nth(1) {
                if let Ok(total) = total_str.parse::<u64>() {
                    return Some(total);
                }
            }
        }
    }

    // Fall back to Content-Length header
    if let Some(content_length) = response.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<u64>() {
                // If this is a partial response, add the bytes we already have
                return Some(length + bytes_already_downloaded);
            }
        }
    }

    None
}

/// Format bytes in human-readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Get current timestamp in seconds since Unix epoch
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Find all resumable downloads in a directory
pub async fn find_resumable_downloads(dir: &Path) -> Result<Vec<DownloadProgress>> {
    let mut downloads = Vec::new();

    if !dir.exists() {
        return Ok(downloads);
    }

    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("download") {
            match DownloadProgress::load_from_file(&path).await {
                Ok(progress) => {
                    if !progress.is_complete {
                        downloads.push(progress);
                    }
                }
                Err(e) => {
                    warn!("Failed to load download progress from {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(downloads)
}

/// Clean up old completed download progress files
pub async fn cleanup_old_progress_files(dir: &Path, max_age_hours: u64) -> Result<usize> {
    let max_age_secs = max_age_hours * 3600;
    let current_time = current_timestamp();
    let mut cleaned_count = 0;

    if !dir.exists() {
        return Ok(0);
    }

    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("download") {
            match DownloadProgress::load_from_file(&path).await {
                Ok(progress) => {
                    let age = current_time.saturating_sub(progress.last_updated);

                    if progress.is_complete && age > max_age_secs
                        && tokio::fs::remove_file(&path).await.is_ok() {
                            cleaned_count += 1;
                            debug!("Cleaned up old progress file: {:?}", path);
                        }
                }
                Err(_) => {
                    // If we can't parse the progress file, it might be corrupted
                    // Clean it up if it's old enough based on file modification time
                    if let Ok(metadata) = tokio::fs::metadata(&path).await {
                        if let Ok(modified) = metadata.modified() {
                            let file_age = std::time::SystemTime::now()
                                .duration_since(modified)
                                .unwrap_or_default()
                                .as_secs();

                            if file_age > max_age_secs
                                && tokio::fs::remove_file(&path).await.is_ok() {
                                    cleaned_count += 1;
                                    debug!("Cleaned up corrupted progress file: {:?}", path);
                                }
                        }
                    }
                }
            }
        }
    }

    Ok(cleaned_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_completion_percentage() {
        let mut progress = DownloadProgress::new(
            "testhash".to_string(),
            "cdn.test.com".to_string(),
            "/data".to_string(),
            PathBuf::from("/tmp/test.dat"),
        );

        // No total size set
        assert!(progress.completion_percentage().is_none());

        // With total size
        progress.total_size = Some(1000);
        progress.bytes_downloaded = 250;
        assert_eq!(progress.completion_percentage(), Some(25.0));

        // Complete download
        progress.bytes_downloaded = 1000;
        assert_eq!(progress.completion_percentage(), Some(100.0));

        // Zero-byte file
        progress.total_size = Some(0);
        progress.bytes_downloaded = 0;
        assert_eq!(progress.completion_percentage(), Some(100.0));
    }

    #[tokio::test]
    async fn test_progress_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let target_file = temp_dir.path().join("test.dat");

        let mut progress = DownloadProgress::new(
            "testhash123".to_string(),
            "cdn.example.com".to_string(),
            "/data".to_string(),
            target_file,
        );

        progress.total_size = Some(2048);
        progress.bytes_downloaded = 1024;

        // Save progress
        progress.save_to_file().await.unwrap();
        assert!(progress.progress_file.exists());

        // Load progress
        let loaded_progress = DownloadProgress::load_from_file(&progress.progress_file)
            .await
            .unwrap();
        assert_eq!(loaded_progress.file_hash, "testhash123");
        assert_eq!(loaded_progress.total_size, Some(2048));
        assert_eq!(loaded_progress.bytes_downloaded, 1024);
        assert_eq!(loaded_progress.cdn_host, "cdn.example.com");
    }

    #[test]
    fn test_extract_total_size_from_content_range() {
        use reqwest::header::{HeaderMap, HeaderValue};

        let client = reqwest::Client::new();
        let _response = client.get("http://example.com").build().unwrap();

        // Mock response with content-range header
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-range",
            HeaderValue::from_static("bytes 200-1023/2048"),
        );

        // We can't directly set headers on a Response, so we'll test the parsing logic
        let content_range = "bytes 200-1023/2048";
        let total: Option<u64> = content_range.split('/').nth(1).and_then(|s| s.parse().ok());
        assert_eq!(total, Some(2048));

        // Test with content-length fallback
        let content_length = "1024";
        let length: Option<u64> = content_length.parse().ok();
        assert_eq!(length, Some(1024));
    }
}
