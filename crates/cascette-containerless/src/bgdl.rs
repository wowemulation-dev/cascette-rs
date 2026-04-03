//! Background download (BGDL) state machine.
//!
//! Agent.exe implements 15 async state machine classes for containerless
//! file lifecycle management. This module provides the BGDL orchestrator
//! with configuration, progress tracking, speed limiting, and batch
//! failure tracking matching agent.exe behavior.
//!
//! Agent.exe state classes (from binanana symbol analysis):
//! - `ContainerlessBgdlState` -- background download orchestrator
//! - `ContainerlessBuildGetSizeState` -- manifest parsing for size estimation
//! - `ContainerlessBuildUpdateState` -- build update flow
//! - `ContainerlessBuildUpdateInitState` -- update initialization
//! - `ContainerlessBuildExtractState` -- file extraction
//! - `ContainerlessBuildPreserveState` -- preservation during maintenance
//! - `ContainerlessFileIdentifyState` -- content hash verification
//! - `ContainerlessFileMakeResidentState` -- mark files as locally available
//! - `ContainerlessFileUpdateState` -- individual file updates
//! - `ContainerlessHeadersFetchState` -- e-header retrieval
//! - `ContainerlessBlockExtractState` -- block-level extraction
//! - `FileDbReadState` -- database read operations
//! - Infrastructure: `ContainerlessBlockMover`, `LooseFileStreamer`,
//!   `FileIdentifyQueue`, `FileUpdateQueue`

use std::time::{Duration, Instant};

/// Background download configuration.
///
/// Derived from agent.exe config keys:
/// - `default_bgdl_speed` -- default speed limit in bytes/sec
/// - `use_default_bgdl_limit` -- whether to use the default limit
/// - `activeBgdlKey` -- currently active BGDL key
/// - `completedBgdlKeys` -- list of completed BGDL keys
#[derive(Debug, Clone)]
pub struct BgdlConfig {
    /// Speed limit in bytes/sec. `None` means unlimited.
    pub speed_limit: Option<u64>,
    /// Use the default speed limit from agent config.
    pub use_default_limit: bool,
    /// Active BGDL key (format: `bgdl-{product}-{build}`).
    pub active_key: Option<String>,
    /// Previously completed BGDL keys.
    pub completed_keys: Vec<String>,
}

impl Default for BgdlConfig {
    fn default() -> Self {
        Self {
            speed_limit: None,
            use_default_limit: true,
            active_key: None,
            completed_keys: Vec::new(),
        }
    }
}

/// Background download progress.
#[derive(Debug, Clone)]
pub struct BgdlProgress {
    /// Bytes downloaded so far.
    pub downloaded_bytes: u64,
    /// Total bytes to download.
    pub total_bytes: u64,
    /// Files completed (success or failure).
    pub files_completed: usize,
    /// Total files to download.
    pub files_total: usize,
    /// Files that failed.
    pub files_failed: usize,
    /// Smoothed download speed in bytes/sec.
    pub current_speed: f64,
    /// Time elapsed since download started.
    pub elapsed: Duration,
}

impl BgdlProgress {
    /// Create a new progress tracker for the given totals.
    fn new(total_bytes: u64, files_total: usize) -> Self {
        Self {
            downloaded_bytes: 0,
            total_bytes,
            files_completed: 0,
            files_total,
            files_failed: 0,
            current_speed: 0.0,
            elapsed: Duration::ZERO,
        }
    }

    /// Fraction of download complete (0.0 to 1.0).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            return 1.0;
        }
        self.downloaded_bytes as f64 / self.total_bytes as f64
    }
}

/// A failed file record for batch tracking.
///
/// Matches the agent.exe log format:
/// `"{n}/{total} loose files failed during batch download."`
#[derive(Debug, Clone)]
pub struct FailedFile {
    /// Encoding key of the failed file.
    pub ekey: [u8; 16],
    /// Error description.
    pub error: String,
}

/// Background download state.
#[derive(Debug, Clone)]
pub enum BgdlState {
    /// No download in progress.
    Idle,
    /// Scanning for files that need downloading.
    Scanning {
        /// Encoding keys of files pending download.
        pending: Vec<[u8; 16]>,
    },
    /// Downloading files from CDN.
    Downloading {
        /// Number of concurrent downloads in flight.
        in_flight: usize,
        /// Completed download count.
        completed: usize,
        /// Total files to download.
        total: usize,
    },
    /// Download paused by user or `containerless_do_real_cancel` config.
    Paused {
        /// Completed downloads before pause.
        completed: usize,
        /// Total files to download.
        total: usize,
    },
    /// Final processing after all downloads complete (residency updates, etc).
    Finalizing {
        /// Successfully downloaded files.
        downloaded: usize,
        /// Files that failed.
        failed: usize,
    },
    /// All downloads finished.
    Complete {
        /// Successfully downloaded files.
        downloaded: usize,
        /// Files that failed to download.
        failed: usize,
    },
    /// Download error.
    Error {
        /// Error description.
        message: String,
    },
}

impl BgdlState {
    /// Whether the state machine has finished (complete or error).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete { .. } | Self::Error { .. })
    }

    /// Whether downloads are active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Scanning { .. } | Self::Downloading { .. } | Self::Finalizing { .. }
        )
    }
}

/// Trait for providing file downloads.
///
/// The containerless crate does not depend on the protocol layer.
/// Callers inject a downloader implementation to connect BGDL to
/// actual CDN fetches.
pub trait BgdlDownloader: Send + Sync {
    /// Download a file by encoding key. Returns the BLTE-encoded bytes.
    fn download(
        &self,
        ekey: &[u8; 16],
    ) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send;
}

/// Token bucket rate limiter for speed control.
///
/// Limits throughput to a configured bytes/sec rate using a token
/// bucket algorithm with 1-second refill intervals.
struct TokenBucket {
    /// Maximum tokens (bytes) per refill period.
    capacity: u64,
    /// Currently available tokens.
    available: u64,
    /// Last refill timestamp.
    last_refill: Instant,
}

impl TokenBucket {
    fn new(bytes_per_sec: u64) -> Self {
        Self {
            capacity: bytes_per_sec,
            available: bytes_per_sec,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire tokens. Returns the delay needed before the
    /// tokens become available, or `Duration::ZERO` if available now.
    #[allow(clippy::cast_precision_loss)]
    fn acquire(&mut self, bytes: u64) -> Duration {
        self.refill();

        if bytes <= self.available {
            self.available -= bytes;
            return Duration::ZERO;
        }

        // Calculate how long to wait for enough tokens.
        let deficit = bytes - self.available;
        let wait_secs = deficit as f64 / self.capacity as f64;
        Duration::from_secs_f64(wait_secs)
    }

    /// Refill tokens based on elapsed time.
    #[allow(clippy::cast_precision_loss)]
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let new_tokens = (elapsed.as_secs_f64() * self.capacity as f64) as u64;
        if new_tokens > 0 {
            self.available = (self.available + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }
}

/// Background download manager.
///
/// Orchestrates scanning, downloading, and installing files into a
/// containerless storage instance. Provides configuration, progress
/// tracking, speed limiting, and batch failure tracking.
pub struct BgdlManager {
    state: BgdlState,
    config: BgdlConfig,
    progress: Option<BgdlProgress>,
    failed_files: Vec<FailedFile>,
    rate_limiter: Option<TokenBucket>,
    start_time: Option<Instant>,
    /// Smoothing factor for speed calculation (exponential moving average).
    speed_alpha: f64,
}

impl BgdlManager {
    /// Create a new manager in idle state with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: BgdlState::Idle,
            config: BgdlConfig::default(),
            progress: None,
            failed_files: Vec::new(),
            rate_limiter: None,
            start_time: None,
            speed_alpha: 0.3,
        }
    }

    /// Create a new manager with the given configuration.
    #[must_use]
    pub fn with_config(config: BgdlConfig) -> Self {
        let rate_limiter = config.speed_limit.map(TokenBucket::new);
        Self {
            state: BgdlState::Idle,
            config,
            progress: None,
            failed_files: Vec::new(),
            rate_limiter,
            start_time: None,
            speed_alpha: 0.3,
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> &BgdlState {
        &self.state
    }

    /// Current configuration.
    #[must_use]
    pub fn config(&self) -> &BgdlConfig {
        &self.config
    }

    /// Current download progress (available after scanning).
    #[must_use]
    pub fn progress(&self) -> Option<&BgdlProgress> {
        self.progress.as_ref()
    }

    /// Files that failed during batch download.
    #[must_use]
    pub fn failed_files(&self) -> &[FailedFile] {
        &self.failed_files
    }

    /// Update the configuration. Resets the rate limiter if speed changes.
    pub fn set_config(&mut self, config: BgdlConfig) {
        let new_limit = config.speed_limit;
        self.config = config;

        match new_limit {
            Some(limit) => {
                self.rate_limiter = Some(TokenBucket::new(limit));
            }
            None => {
                self.rate_limiter = None;
            }
        }
    }

    /// Start scanning for files that need downloading.
    pub fn start_scan(&mut self, pending: Vec<[u8; 16]>) {
        self.state = BgdlState::Scanning { pending };
    }

    /// Transition from scanning to downloading.
    ///
    /// `total_bytes` is the estimated total size for progress tracking.
    pub fn begin_download(&mut self, total_bytes: u64) {
        if let BgdlState::Scanning { ref pending } = self.state {
            let total = pending.len();
            self.progress = Some(BgdlProgress::new(total_bytes, total));
            self.start_time = Some(Instant::now());
            self.failed_files.clear();
            self.state = BgdlState::Downloading {
                in_flight: 0,
                completed: 0,
                total,
            };
        }
    }

    /// Record a completed download (success or failure).
    #[allow(clippy::cast_precision_loss)]
    pub fn record_completion(&mut self, success: bool, bytes: u64) {
        if let BgdlState::Downloading {
            ref mut completed,
            total,
            ..
        } = self.state
        {
            *completed += 1;

            // Update progress.
            if let Some(ref mut progress) = self.progress {
                progress.files_completed += 1;
                if success {
                    progress.downloaded_bytes += bytes;
                } else {
                    progress.files_failed += 1;
                }

                // Update elapsed time and speed.
                if let Some(start) = self.start_time {
                    progress.elapsed = start.elapsed();
                    let elapsed_secs = progress.elapsed.as_secs_f64();
                    if elapsed_secs > 0.0 {
                        let instant_speed = progress.downloaded_bytes as f64 / elapsed_secs;
                        // Exponential moving average.
                        progress.current_speed = self.speed_alpha.mul_add(
                            instant_speed,
                            (1.0 - self.speed_alpha) * progress.current_speed,
                        );
                    }
                }
            }

            if *completed >= total {
                let failed = self.failed_files.len();
                let downloaded = total - failed;
                self.state = BgdlState::Finalizing { downloaded, failed };
            }
        }
    }

    /// Record a file failure with its encoding key and error.
    pub fn record_failure(&mut self, ekey: [u8; 16], error: String) {
        self.failed_files.push(FailedFile { ekey, error });
        self.record_completion(false, 0);
    }

    /// Transition from finalizing to complete.
    pub fn finalize(&mut self) {
        if let BgdlState::Finalizing { downloaded, failed } = self.state {
            self.state = BgdlState::Complete { downloaded, failed };
        }
    }

    /// Pause the download.
    pub fn pause(&mut self) {
        if let BgdlState::Downloading {
            completed, total, ..
        } = self.state
        {
            self.state = BgdlState::Paused { completed, total };
        }
    }

    /// Resume a paused download.
    pub fn resume(&mut self) {
        if let BgdlState::Paused { completed, total } = self.state {
            self.state = BgdlState::Downloading {
                in_flight: 0,
                completed,
                total,
            };
        }
    }

    /// Transition to error state.
    pub fn set_error(&mut self, message: String) {
        self.state = BgdlState::Error { message };
    }

    /// Reset to idle, clearing all progress and failure tracking.
    pub fn reset(&mut self) {
        self.state = BgdlState::Idle;
        self.progress = None;
        self.failed_files.clear();
        self.start_time = None;
    }

    /// Acquire rate limiter tokens for a download of the given size.
    ///
    /// Returns the duration to wait before downloading. Returns
    /// `Duration::ZERO` if no rate limit is configured.
    pub fn acquire_rate_limit(&mut self, bytes: u64) -> Duration {
        match self.rate_limiter.as_mut() {
            Some(limiter) => limiter.acquire(bytes),
            None => Duration::ZERO,
        }
    }

    /// Process a tick of the event loop. Updates elapsed time and speed.
    ///
    /// Call this periodically (e.g. every 100ms) for smooth progress updates.
    pub fn tick(&mut self) {
        if let Some(ref mut progress) = self.progress
            && let Some(start) = self.start_time
        {
            progress.elapsed = start.elapsed();
        }
    }
}

impl Default for BgdlManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let mgr = BgdlManager::new();
        assert!(matches!(mgr.state(), BgdlState::Idle));
        assert!(!mgr.state().is_active());
        assert!(!mgr.state().is_terminal());
    }

    #[test]
    fn test_scan_to_download_to_complete() {
        let mut mgr = BgdlManager::new();

        let keys = vec![[0x01; 16], [0x02; 16]];
        mgr.start_scan(keys);
        assert!(mgr.state().is_active());

        mgr.begin_download(2048);
        assert!(matches!(
            mgr.state(),
            BgdlState::Downloading { total: 2, .. }
        ));

        mgr.record_completion(true, 1024);
        mgr.record_completion(true, 1024);

        // Should be in Finalizing now.
        assert!(matches!(
            mgr.state(),
            BgdlState::Finalizing {
                downloaded: 2,
                failed: 0
            }
        ));

        mgr.finalize();
        assert!(mgr.state().is_terminal());
        assert!(matches!(
            mgr.state(),
            BgdlState::Complete {
                downloaded: 2,
                failed: 0
            }
        ));
    }

    #[test]
    fn test_error_state() {
        let mut mgr = BgdlManager::new();
        mgr.set_error("cdn unavailable".to_string());
        assert!(mgr.state().is_terminal());
        assert!(matches!(mgr.state(), BgdlState::Error { .. }));
    }

    #[test]
    fn test_reset() {
        let mut mgr = BgdlManager::new();
        mgr.set_error("test".to_string());
        mgr.reset();
        assert!(matches!(mgr.state(), BgdlState::Idle));
    }

    #[test]
    fn test_pause_resume() {
        let mut mgr = BgdlManager::new();

        let keys = vec![[0x01; 16], [0x02; 16], [0x03; 16]];
        mgr.start_scan(keys);
        mgr.begin_download(3000);

        mgr.record_completion(true, 1000);

        mgr.pause();
        assert!(matches!(
            mgr.state(),
            BgdlState::Paused {
                completed: 1,
                total: 3
            }
        ));
        // Paused is not active.
        assert!(!mgr.state().is_active());
        assert!(!mgr.state().is_terminal());

        mgr.resume();
        assert!(matches!(
            mgr.state(),
            BgdlState::Downloading {
                completed: 1,
                total: 3,
                ..
            }
        ));
    }

    #[test]
    fn test_progress_tracking() {
        let mut mgr = BgdlManager::new();

        mgr.start_scan(vec![[0x01; 16], [0x02; 16]]);
        mgr.begin_download(2000);

        let progress = mgr.progress().unwrap();
        assert_eq!(progress.total_bytes, 2000);
        assert_eq!(progress.files_total, 2);
        assert_eq!(progress.downloaded_bytes, 0);

        mgr.record_completion(true, 1200);
        let progress = mgr.progress().unwrap();
        assert_eq!(progress.downloaded_bytes, 1200);
        assert_eq!(progress.files_completed, 1);
        assert_eq!(progress.files_failed, 0);
    }

    #[test]
    fn test_failure_tracking() {
        let mut mgr = BgdlManager::new();

        mgr.start_scan(vec![[0x01; 16], [0x02; 16]]);
        mgr.begin_download(2000);

        mgr.record_completion(true, 1000);
        mgr.record_failure([0x02; 16], "connection timeout".to_string());

        assert_eq!(mgr.failed_files().len(), 1);
        assert_eq!(mgr.failed_files()[0].ekey, [0x02; 16]);

        // Should transition to Finalizing.
        assert!(matches!(
            mgr.state(),
            BgdlState::Finalizing {
                downloaded: 1,
                failed: 1
            }
        ));

        let progress = mgr.progress().unwrap();
        assert_eq!(progress.files_failed, 1);
    }

    #[test]
    fn test_config_defaults() {
        let config = BgdlConfig::default();
        assert!(config.speed_limit.is_none());
        assert!(config.use_default_limit);
        assert!(config.active_key.is_none());
        assert!(config.completed_keys.is_empty());
    }

    #[test]
    fn test_with_config() {
        let config = BgdlConfig {
            speed_limit: Some(1_000_000),
            use_default_limit: false,
            active_key: Some("bgdl-wow-12345".to_string()),
            completed_keys: vec!["bgdl-wow-11111".to_string()],
        };
        let mgr = BgdlManager::with_config(config);
        assert_eq!(mgr.config().speed_limit, Some(1_000_000));
        assert!(!mgr.config().use_default_limit);
    }

    #[test]
    fn test_rate_limiter_no_limit() {
        let mut mgr = BgdlManager::new();
        let delay = mgr.acquire_rate_limit(1_000_000);
        assert_eq!(delay, Duration::ZERO);
    }

    #[test]
    fn test_rate_limiter_with_limit() {
        let config = BgdlConfig {
            speed_limit: Some(1000),
            ..BgdlConfig::default()
        };
        let mut mgr = BgdlManager::with_config(config);

        // First acquisition should succeed immediately (bucket starts full).
        let delay = mgr.acquire_rate_limit(500);
        assert_eq!(delay, Duration::ZERO);

        // Second acquisition of remaining tokens should succeed.
        let delay = mgr.acquire_rate_limit(500);
        assert_eq!(delay, Duration::ZERO);

        // Third acquisition should require waiting (bucket empty).
        let delay = mgr.acquire_rate_limit(500);
        assert!(delay > Duration::ZERO);
    }

    #[test]
    fn test_progress_fraction() {
        let mut progress = BgdlProgress::new(1000, 10);
        assert!((progress.fraction() - 0.0).abs() < f64::EPSILON);

        progress.downloaded_bytes = 500;
        assert!((progress.fraction() - 0.5).abs() < f64::EPSILON);

        progress.downloaded_bytes = 1000;
        assert!((progress.fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_progress_fraction_zero_total() {
        let progress = BgdlProgress::new(0, 0);
        assert!((progress.fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_reset_clears_progress() {
        let mut mgr = BgdlManager::new();
        mgr.start_scan(vec![[0x01; 16]]);
        mgr.begin_download(1000);
        mgr.record_failure([0x01; 16], "error".to_string());

        mgr.reset();
        assert!(mgr.progress().is_none());
        assert!(mgr.failed_files().is_empty());
    }

    #[test]
    fn test_finalizing_is_active() {
        let state = BgdlState::Finalizing {
            downloaded: 5,
            failed: 0,
        };
        assert!(state.is_active());
        assert!(!state.is_terminal());
    }

    #[test]
    fn test_set_config_updates_rate_limiter() {
        let mut mgr = BgdlManager::new();

        // No rate limiter initially.
        assert_eq!(mgr.acquire_rate_limit(1_000_000), Duration::ZERO);

        // Set a speed limit.
        mgr.set_config(BgdlConfig {
            speed_limit: Some(100),
            ..BgdlConfig::default()
        });

        // First 100 bytes are free (bucket full).
        let delay = mgr.acquire_rate_limit(100);
        assert_eq!(delay, Duration::ZERO);

        // Next bytes require waiting.
        let delay = mgr.acquire_rate_limit(100);
        assert!(delay > Duration::ZERO);
    }

    #[test]
    fn test_tick_updates_elapsed() {
        let mut mgr = BgdlManager::new();
        mgr.start_scan(vec![[0x01; 16]]);
        mgr.begin_download(1000);

        // Tick should update elapsed without panicking.
        mgr.tick();
        let progress = mgr.progress().unwrap();
        // Elapsed should be non-negative (it was just set).
        assert!(progress.elapsed >= Duration::ZERO);
    }
}
