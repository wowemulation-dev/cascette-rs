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

    /// A manifest file is being downloaded from CDN.
    ManifestDownloading {
        /// Manifest type (e.g., "build config", "encoding", "install", "download").
        manifest_type: String,
    },

    /// A manifest file has been downloaded and parsed.
    ManifestComplete {
        /// Manifest type.
        manifest_type: String,
        /// Number of entries in the manifest (0 for configs).
        entries: usize,
    },

    /// Metadata resolution completed — all manifests downloaded and classified.
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

    /// Loose file download phase started (install manifest files).
    LooseFilePhaseStarted {
        /// Number of loose files to download.
        files: usize,
        /// Total bytes for loose files.
        total_bytes: u64,
    },

    /// Loose file download phase completed.
    LooseFilePhaseComplete {
        /// Files placed in product directory.
        files_placed: usize,
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
        /// Bytes downloaded for this file.
        bytes: u64,
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

    /// Layout files written (.build.info, configs).
    LayoutWritten,

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
