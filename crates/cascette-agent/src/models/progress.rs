//! Progress tracking for operations.
//!
//! Tracks download/verification progress with speed calculation and ETA.

use serde::{Deserialize, Serialize};

/// Progress metrics for an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    /// Current phase description (e.g., "downloading", "verifying").
    pub phase: String,
    /// Completion percentage (0.0 - 100.0).
    pub percentage: f64,
    /// Bytes processed so far.
    pub bytes_done: u64,
    /// Total bytes to process.
    pub bytes_total: u64,
    /// Files completed so far.
    pub files_done: usize,
    /// Total files to process.
    pub files_total: usize,
    /// File currently being processed.
    pub current_file: Option<String>,
    /// Download speed in bytes per second.
    pub speed_bps: Option<u64>,
    /// Estimated time remaining in seconds.
    pub eta_seconds: Option<u64>,
}

impl Progress {
    /// Create a new progress tracker for a phase.
    #[must_use]
    pub fn new(phase: String, bytes_total: u64, files_total: usize) -> Self {
        Self {
            phase,
            percentage: 0.0,
            bytes_done: 0,
            bytes_total,
            files_done: 0,
            files_total,
            current_file: None,
            speed_bps: None,
            eta_seconds: None,
        }
    }

    /// Update byte progress and recalculate percentage.
    #[allow(clippy::cast_precision_loss)]
    pub fn update_bytes(&mut self, bytes_done: u64) {
        self.bytes_done = bytes_done;
        if self.bytes_total > 0 {
            self.percentage = (bytes_done as f64 / self.bytes_total as f64) * 100.0;
        }
        self.recalculate_eta();
    }

    /// Update file progress and recalculate percentage.
    #[allow(clippy::cast_precision_loss)]
    pub fn update_files(&mut self, files_done: usize) {
        self.files_done = files_done;
        if self.files_total > 0 && self.bytes_total == 0 {
            self.percentage = (files_done as f64 / self.files_total as f64) * 100.0;
        }
    }

    /// Set download speed and recalculate ETA.
    pub fn set_speed(&mut self, speed_bps: u64) {
        self.speed_bps = Some(speed_bps);
        self.recalculate_eta();
    }

    /// Whether the operation is complete (100%).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        (self.percentage - 100.0).abs() < f64::EPSILON
    }

    fn recalculate_eta(&mut self) {
        if let Some(speed) = self.speed_bps
            && speed > 0
        {
            let remaining = self.bytes_total.saturating_sub(self.bytes_done);
            self.eta_seconds = Some(remaining / speed);
        }
    }

    /// Format bytes as human-readable string (e.g., "1.50 GB").
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{bytes} B")
        }
    }

    /// Format speed as human-readable string (e.g., "1.00 MB/s").
    #[must_use]
    pub fn format_speed(bps: u64) -> String {
        format!("{}/s", Self::format_bytes(bps))
    }

    /// Format ETA as human-readable string (e.g., "1h 2m 30s").
    #[must_use]
    pub fn format_eta(seconds: u64) -> String {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;

        if hours > 0 {
            format!("{hours}h {minutes}m {secs}s")
        } else if minutes > 0 {
            format!("{minutes}m {secs}s")
        } else {
            format!("{secs}s")
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_update_bytes() {
        let mut progress = Progress::new("downloading".to_string(), 1000, 10);
        progress.update_bytes(500);
        assert!((progress.percentage - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_progress_update_files() {
        let mut progress = Progress::new("verifying".to_string(), 0, 100);
        progress.update_files(75);
        assert!((progress.percentage - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_progress_eta() {
        let mut progress = Progress::new("downloading".to_string(), 1000, 10);
        progress.update_bytes(500);
        progress.set_speed(100);
        assert_eq!(progress.eta_seconds, Some(5));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(Progress::format_bytes(500), "500 B");
        assert_eq!(Progress::format_bytes(1536), "1.50 KB");
        assert_eq!(Progress::format_bytes(1_572_864), "1.50 MB");
        assert_eq!(Progress::format_bytes(1_610_612_736), "1.50 GB");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(Progress::format_speed(1_048_576), "1.00 MB/s");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(Progress::format_eta(30), "30s");
        assert_eq!(Progress::format_eta(90), "1m 30s");
        assert_eq!(Progress::format_eta(3690), "1h 1m 30s");
    }

    #[test]
    fn test_is_complete() {
        let mut progress = Progress::new("downloading".to_string(), 100, 1);
        assert!(!progress.is_complete());
        progress.update_bytes(100);
        assert!(progress.is_complete());
    }
}
