//! Report types for maintenance operations.
//!
//! Every operation produces a report regardless of execution mode. In dry-run
//! mode the report describes what *would* happen. In execute mode it describes
//! what *did* happen. The `executed` field distinguishes the two.

use std::time::Duration;

/// Report from building the preservation set.
#[derive(Debug, Clone, Default)]
pub struct PreservationReport {
    /// Number of keys in the preservation set.
    pub key_count: usize,
    /// Number of index entries scanned to build the set.
    pub source_index_entries: usize,
    /// Keys added from manifests beyond what indices reference.
    pub manifest_keys: usize,
    /// Time taken to build the preservation set.
    pub duration: Duration,
}

/// Report from garbage collection.
#[derive(Debug, Clone, Default)]
pub struct GcReport {
    /// Whether the operation was executed (false = dry-run).
    pub executed: bool,
    /// Total index entries scanned.
    pub entries_scanned: usize,
    /// Entries removed (or that would be removed in dry-run).
    pub entries_removed: usize,
    /// Segments with no remaining index references.
    pub segments_orphaned: usize,
    /// Segments deleted (0 in dry-run).
    pub segments_deleted: usize,
    /// Bytes freed (or freeable in dry-run).
    pub bytes_freed: u64,
    /// Empty directories cleaned up.
    pub empty_dirs_cleaned: usize,
    /// Config files removed from `Data/config/`.
    pub config_files_removed: usize,
    /// CDN index files removed from `Data/indices/`.
    pub cdn_indices_removed: usize,
    /// Time taken.
    pub duration: Duration,
}

/// Report from compaction.
#[derive(Debug, Clone, Default)]
pub struct CompactionReport {
    /// Whether the operation was executed (false = dry-run).
    pub executed: bool,
    /// Number of segments analyzed.
    pub segments_analyzed: usize,
    /// Segments that were (or would be) compacted.
    pub segments_compacted: usize,
    /// Data moves planned.
    pub moves_planned: usize,
    /// Data moves executed (0 in dry-run).
    pub moves_executed: usize,
    /// Total bytes moved.
    pub bytes_moved: u64,
    /// Bytes reclaimed (or reclaimable in dry-run).
    pub bytes_reclaimed: u64,
    /// Segments freed after compaction.
    pub segments_freed: usize,
    /// Segments that were defragmented.
    pub defrag_segments: usize,
    /// Segments analyzed for fill-holes gaps.
    pub fillholes_segments: usize,
    /// Segments merged into other segments.
    pub merged_segments: usize,
    /// Time taken.
    pub duration: Duration,
}

/// Report from repair.
#[derive(Debug, Clone, Default)]
pub struct RepairReport {
    /// Whether the operation was executed (false = dry-run).
    pub executed: bool,
    /// Total entries verified.
    pub entries_verified: usize,
    /// Entries that passed verification.
    pub entries_valid: usize,
    /// Entries with corruption detected.
    pub entries_corrupted: usize,
    /// Indices rebuilt (0 in dry-run).
    pub indices_rebuilt: usize,
    /// Entries re-downloaded from CDN.
    pub entries_redownloaded: usize,
    /// Entries that failed CDN re-download.
    pub redownload_failed: usize,
    /// Loose files checked for integrity.
    pub loose_files_checked: usize,
    /// Loose files repaired (re-downloaded or regenerated).
    pub loose_files_repaired: usize,
    /// Repair marker files written.
    pub markers_written: usize,
    /// Time taken.
    pub duration: Duration,
}

/// Aggregate report from a full maintenance run.
#[derive(Debug, Clone, Default)]
pub struct MaintenanceReport {
    /// Preservation set report (always present).
    pub preservation: Option<PreservationReport>,
    /// Garbage collection report.
    pub gc: Option<GcReport>,
    /// Compaction report.
    pub compaction: Option<CompactionReport>,
    /// Repair report.
    pub repair: Option<RepairReport>,
    /// Total time for the full pipeline.
    pub total_duration: Duration,
}
