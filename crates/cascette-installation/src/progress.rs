//! Progress reporting types for installation pipelines.

/// Events emitted during pipeline execution.
///
/// Pass a callback `impl Fn(ProgressEvent) + Send` to pipeline `run()` methods
/// to receive progress updates.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Metadata resolution has started.
    MetadataResolving {
        /// Product code being resolved.
        product: String,
    },

    /// Metadata resolution completed.
    MetadataResolved {
        /// Number of artifacts identified.
        artifacts: usize,
        /// Total bytes to download.
        total_bytes: u64,
    },

    /// An archive index download has started.
    ArchiveIndexDownloading {
        /// Current index number (0-based).
        index: usize,
        /// Total number of indices.
        total: usize,
    },

    /// An archive index download completed.
    ArchiveIndexComplete {
        /// Archive key (hex).
        archive_key: String,
    },

    /// A file download has started.
    FileDownloading {
        /// File path or identifier.
        path: String,
        /// File size in bytes.
        size: u64,
    },

    /// A file download completed.
    FileComplete {
        /// File path or identifier.
        path: String,
    },

    /// A file download failed.
    FileFailed {
        /// File path or identifier.
        path: String,
        /// Error description.
        error: String,
    },

    /// A checkpoint was saved.
    CheckpointSaved {
        /// Number of completed files.
        completed: usize,
        /// Number of remaining files.
        remaining: usize,
    },

    /// A verification result for a single file.
    VerifyResult {
        /// File path or identifier.
        path: String,
        /// Whether the file passed verification.
        valid: bool,
    },

    /// Extraction of a file started.
    ExtractStarted {
        /// File path being extracted.
        path: String,
    },

    /// Extraction of a file completed.
    ExtractComplete {
        /// File path extracted.
        path: String,
    },

    /// A repair download started.
    RepairDownloading {
        /// File path being repaired.
        path: String,
    },

    /// A repair download completed.
    RepairComplete {
        /// File path repaired.
        path: String,
    },

    /// Update artifact classification completed.
    UpdateClassified {
        /// Files that must be fully downloaded.
        required: usize,
        /// Partially-downloaded files to resume.
        partial: usize,
        /// Files with in-progress patch chains.
        inflight: usize,
        /// Files available from an alternate installation.
        leechable: usize,
        /// Total bytes across all categories.
        total_bytes: u64,
    },

    /// A file was copied from an alternate installation.
    FileLeeched {
        /// File path or identifier.
        path: String,
        /// Bytes copied.
        bytes: u64,
    },

    /// Leeching a file from an alternate installation failed.
    LeechFailed {
        /// File path or identifier.
        path: String,
        /// Error description.
        error: String,
    },

    /// A patch is being applied.
    PatchApplying {
        /// File path or identifier.
        path: String,
    },

    /// A patch was applied.
    PatchApplied {
        /// File path or identifier.
        path: String,
    },

    /// A patch application failed.
    PatchFailed {
        /// File path or identifier.
        path: String,
        /// Error description.
        error: String,
    },
}
