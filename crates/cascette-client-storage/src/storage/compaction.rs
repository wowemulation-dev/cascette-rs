//! Two-phase compaction for CASC archive segments.
//!
//! Implements the compaction pipeline from Agent.exe:
//! - Archive merge (flag=0): consolidates fragmented segments
//! - Extract-compact (flag=1): per-segment span validation and cleanup
//!
//! Buffer sizing: `count = min(total >> 17, 16)`, per_buf = total/count,
//! minimum 128 KiB total.
//!
//! The backup file (`<data_dir>.extract_bu`) tracks in-progress compaction
//! for crash recovery. Format: version(1) + max_entries(1023) + segment
//! indices as u32 LE, append-only.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::storage::segment::{MAX_SEGMENTS, SegmentInfo, SegmentState};
use crate::{Result, StorageError};

/// Minimum total buffer size for compaction (128 KiB).
const MIN_BUFFER_SIZE: usize = 128 * 1024;

/// Maximum number of I/O buffers.
const MAX_BUFFERS: usize = 16;

/// Buffer size shift (128 KiB = 2^17).
const BUFFER_SIZE_SHIFT: u32 = 17;

/// Backup file version.
const BACKUP_VERSION: u8 = 1;

/// Backup file suffix.
const BACKUP_SUFFIX: &str = ".extract_bu";

/// Compaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionMode {
    /// Archive merge: consolidate fragmented segments (async).
    ArchiveMerge,
    /// Extract-compact: per-segment cleanup (direct).
    ExtractCompact,
}

/// A span within a segment (offset + length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataSpan {
    /// Byte offset within the segment.
    pub offset: u64,
    /// Length in bytes.
    pub length: u64,
}

impl DataSpan {
    /// Check if two spans overlap.
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.offset < other.offset + other.length && other.offset < self.offset + self.length
    }

    /// End offset (exclusive).
    pub const fn end(&self) -> u64 {
        self.offset + self.length
    }
}

/// A move item in the compaction plan.
#[derive(Debug, Clone)]
pub struct MoveItem {
    /// Source segment index.
    pub source_segment: u16,
    /// Offset within the source segment.
    pub source_offset: u64,
    /// Destination segment index.
    pub dest_segment: u16,
    /// Offset within the destination segment.
    pub dest_offset: u64,
    /// Number of bytes to move.
    pub length: u64,
    /// Encoding key (9 bytes) for index updates.
    pub ekey: [u8; 9],
}

/// Compaction plan describing what data to move.
#[derive(Debug, Default)]
pub struct CompactionPlan {
    /// Source segments being compacted.
    pub source_segments: Vec<u16>,
    /// Target segments receiving data.
    pub target_segments: Vec<u16>,
    /// Ordered list of move operations.
    pub moves: Vec<MoveItem>,
    /// Total bytes to move.
    pub total_bytes: u64,
}

impl CompactionPlan {
    /// Check if the plan is empty (nothing to do).
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// Number of move operations.
    pub fn move_count(&self) -> usize {
        self.moves.len()
    }
}

/// Buffered file mover for compaction I/O.
///
/// Sizing follows Agent.exe: `count = min(total >> 17, 16)`,
/// `per_buf = total / count`. Minimum total is 128 KiB.
pub struct CompactionFileMover {
    /// Per-buffer size.
    buf_size: usize,
    /// Number of buffers.
    buf_count: usize,
    /// I/O buffer.
    buffer: Vec<u8>,
    /// Bytes moved so far.
    bytes_moved: u64,
}

impl CompactionFileMover {
    /// Create a new file mover with the given total buffer budget.
    ///
    /// The budget is clamped to at least 128 KiB. Buffer count is
    /// `min(total >> 17, 16)`, with per-buffer size = total / count.
    pub fn new(total_budget: usize) -> Self {
        let total = total_budget.max(MIN_BUFFER_SIZE);
        let count = (total >> BUFFER_SIZE_SHIFT).clamp(1, MAX_BUFFERS);
        let per_buf = total / count;

        Self {
            buf_size: per_buf,
            buf_count: count,
            buffer: vec![0u8; per_buf],
            bytes_moved: 0,
        }
    }

    /// Get the per-buffer size.
    pub const fn buffer_size(&self) -> usize {
        self.buf_size
    }

    /// Get the buffer count.
    pub const fn buffer_count(&self) -> usize {
        self.buf_count
    }

    /// Get total bytes moved.
    pub const fn bytes_moved(&self) -> u64 {
        self.bytes_moved
    }

    /// Move data between files.
    ///
    /// Reads from `source` at `src_offset` and writes to `dest` at
    /// `dest_offset`, moving `length` bytes in buffer-sized chunks.
    pub fn move_data(
        &mut self,
        source: &mut File,
        src_offset: u64,
        dest: &mut File,
        dest_offset: u64,
        length: u64,
    ) -> Result<()> {
        source.seek(SeekFrom::Start(src_offset)).map_err(|e| {
            StorageError::Archive(format!("compaction: source seek failed: {e}"))
        })?;
        dest.seek(SeekFrom::Start(dest_offset)).map_err(|e| {
            StorageError::Archive(format!("compaction: dest seek failed: {e}"))
        })?;

        let mut remaining = length;
        while remaining > 0 {
            let chunk = (remaining as usize).min(self.buf_size);
            let buf = &mut self.buffer[..chunk];

            source.read_exact(buf).map_err(|e| {
                StorageError::Archive(format!("compaction: read failed: {e}"))
            })?;
            dest.write_all(buf).map_err(|e| {
                StorageError::Archive(format!("compaction: write failed: {e}"))
            })?;

            remaining -= chunk as u64;
            self.bytes_moved += chunk as u64;
        }

        Ok(())
    }

    /// Copy data within the same file (for in-place compaction).
    pub fn compact_in_place(
        &mut self,
        file: &mut File,
        src_offset: u64,
        dest_offset: u64,
        length: u64,
    ) -> Result<()> {
        if src_offset == dest_offset {
            return Ok(());
        }

        let mut remaining = length;
        let mut src_pos = src_offset;
        let mut dest_pos = dest_offset;

        while remaining > 0 {
            let chunk = (remaining as usize).min(self.buf_size);
            let buf = &mut self.buffer[..chunk];

            file.seek(SeekFrom::Start(src_pos)).map_err(|e| {
                StorageError::Archive(format!("compaction: seek read failed: {e}"))
            })?;
            file.read_exact(buf).map_err(|e| {
                StorageError::Archive(format!("compaction: read failed: {e}"))
            })?;

            file.seek(SeekFrom::Start(dest_pos)).map_err(|e| {
                StorageError::Archive(format!("compaction: seek write failed: {e}"))
            })?;
            file.write_all(buf).map_err(|e| {
                StorageError::Archive(format!("compaction: write failed: {e}"))
            })?;

            remaining -= chunk as u64;
            src_pos += chunk as u64;
            dest_pos += chunk as u64;
            self.bytes_moved += chunk as u64;
        }

        Ok(())
    }
}

/// Backup file for crash recovery during extract-compact.
///
/// Format: version(1 byte) + max_entries(u32 LE) + segment indices
/// (u32 LE each), append-only.
///
/// The backup file has a fixed capacity of 4101 bytes:
/// `1 + 4 + 1023 * 4 = 4097`, rounded to 4101 for alignment.
pub struct ExtractorCompactorBackup {
    /// Path to the backup file.
    path: PathBuf,
    /// Segment indices recorded in the backup.
    segments: Vec<u16>,
}

/// Backup file header size: version(1) + max_entries(4) = 5 bytes.
const BACKUP_HEADER_SIZE: usize = 5;

/// Maximum entries in the backup file (same as MAX_SEGMENTS).
const BACKUP_MAX_ENTRIES: u32 = MAX_SEGMENTS as u32;

/// Total backup file size: header + max_entries * 4 bytes.
#[allow(dead_code)]
const BACKUP_FILE_SIZE: usize = BACKUP_HEADER_SIZE + MAX_SEGMENTS as usize * 4;

impl ExtractorCompactorBackup {
    /// Create a new backup file at the given data directory.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(BACKUP_SUFFIX.trim_start_matches('.')),
            segments: Vec::new(),
        }
    }

    /// Load an existing backup file for recovery.
    pub fn load(data_dir: &Path) -> Result<Option<Self>> {
        let path = data_dir.join(BACKUP_SUFFIX.trim_start_matches('.'));

        if !path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&path).map_err(|e| {
            StorageError::Archive(format!(
                "failed to open compaction backup {}: {e}",
                path.display()
            ))
        })?;

        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| {
            StorageError::Archive(format!("failed to read compaction backup: {e}"))
        })?;

        if data.len() < BACKUP_HEADER_SIZE {
            warn!("compaction backup too small, ignoring");
            return Ok(None);
        }

        let version = data[0];
        if version != BACKUP_VERSION {
            warn!("compaction backup version {version} != {BACKUP_VERSION}, ignoring");
            return Ok(None);
        }

        let max_entries = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
        let entry_count = (data.len() - BACKUP_HEADER_SIZE) / 4;
        let count = entry_count.min(max_entries as usize);

        let mut segments = Vec::with_capacity(count);
        for i in 0..count {
            let offset = BACKUP_HEADER_SIZE + i * 4;
            if offset + 4 <= data.len() {
                let idx = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                if let Ok(seg) = u16::try_from(idx) {
                    segments.push(seg);
                }
            }
        }

        debug!(
            "loaded compaction backup with {} segments from {}",
            segments.len(),
            path.display()
        );

        Ok(Some(Self { path, segments }))
    }

    /// Write the backup file to disk.
    pub fn save(&self) -> Result<()> {
        let mut file = File::create(&self.path).map_err(|e| {
            StorageError::Archive(format!(
                "failed to create compaction backup {}: {e}",
                self.path.display()
            ))
        })?;

        // Header
        file.write_all(&[BACKUP_VERSION]).map_err(|e| {
            StorageError::Archive(format!("failed to write backup version: {e}"))
        })?;
        file.write_all(&BACKUP_MAX_ENTRIES.to_le_bytes())
            .map_err(|e| {
                StorageError::Archive(format!("failed to write backup max entries: {e}"))
            })?;

        // Segment indices
        for &seg in &self.segments {
            file.write_all(&u32::from(seg).to_le_bytes())
                .map_err(|e| {
                    StorageError::Archive(format!("failed to write backup segment: {e}"))
                })?;
        }

        file.flush().map_err(|e| {
            StorageError::Archive(format!("failed to flush backup: {e}"))
        })?;

        Ok(())
    }

    /// Append a segment index to the backup (append-only).
    pub fn record_segment(&mut self, segment_index: u16) -> Result<()> {
        self.segments.push(segment_index);

        // Append to the file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| {
                StorageError::Archive(format!("failed to append to backup: {e}"))
            })?;

        // If file is empty, write header first
        let metadata = file.metadata().map_err(|e| {
            StorageError::Archive(format!("failed to stat backup: {e}"))
        })?;

        if metadata.len() == 0 {
            file.write_all(&[BACKUP_VERSION]).map_err(|e| {
                StorageError::Archive(format!("failed to write backup header: {e}"))
            })?;
            file.write_all(&BACKUP_MAX_ENTRIES.to_le_bytes())
                .map_err(|e| {
                    StorageError::Archive(format!("failed to write backup max entries: {e}"))
                })?;
        }

        file.write_all(&u32::from(segment_index).to_le_bytes())
            .map_err(|e| {
                StorageError::Archive(format!("failed to write segment to backup: {e}"))
            })?;

        Ok(())
    }

    /// Get the recorded segment indices.
    pub fn segments(&self) -> &[u16] {
        &self.segments
    }

    /// Remove the backup file (after successful compaction).
    pub fn remove(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|e| {
                StorageError::Archive(format!(
                    "failed to remove compaction backup {}: {e}",
                    self.path.display()
                ))
            })?;
        }
        Ok(())
    }
}

/// Validate that spans within a segment don't overlap.
///
/// Returns `Ok(())` if all spans are non-overlapping, or `Err` with
/// the first overlapping pair.
pub fn validate_spans(spans: &mut [DataSpan]) -> Result<()> {
    if spans.len() <= 1 {
        return Ok(());
    }

    // Sort by offset
    spans.sort_by_key(|s| s.offset);

    // Check adjacent pairs for overlap
    for i in 0..spans.len() - 1 {
        if spans[i].end() > spans[i + 1].offset {
            return Err(StorageError::Archive(format!(
                "overlapping spans: [{}, {}) and [{}, {})",
                spans[i].offset,
                spans[i].end(),
                spans[i + 1].offset,
                spans[i + 1].end(),
            )));
        }
    }

    Ok(())
}

/// Build a compaction plan for fragmented segments.
///
/// Identifies segments with low utilization and plans moves to
/// consolidate data into fewer segments.
pub fn plan_archive_merge(
    segments: &[SegmentInfo],
    utilization_threshold: f64,
    segment_size: u64,
) -> CompactionPlan {
    let mut plan = CompactionPlan::default();

    // Identify source segments (low utilization, frozen)
    let mut sources: Vec<(u16, u64)> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        if seg.state == SegmentState::Frozen {
            let used = seg.write_position;
            #[allow(clippy::cast_precision_loss)]
            let utilization = used as f64 / segment_size as f64;
            if utilization < utilization_threshold && used > 0 {
                let idx = u16::try_from(i).unwrap_or(u16::MAX);
                sources.push((idx, used));
            }
        }
    }

    if sources.len() < 2 {
        // Need at least 2 sources to merge
        return plan;
    }

    // Sort sources by utilization (smallest first)
    sources.sort_by_key(|&(_, used)| used);

    // Greedily merge small segments into larger ones
    let mut dest_idx = 0;
    let mut dest_used = 0u64;

    for &(source_seg, source_used) in &sources[1..] {
        let (dest_seg, _) = sources[dest_idx];

        if dest_used + source_used <= segment_size {
            plan.moves.push(MoveItem {
                source_segment: source_seg,
                source_offset: 0,
                dest_segment: dest_seg,
                dest_offset: dest_used,
                length: source_used,
                ekey: [0; 9], // Will be filled per-entry during execution
            });
            plan.total_bytes += source_used;
            dest_used += source_used;

            if !plan.source_segments.contains(&source_seg) {
                plan.source_segments.push(source_seg);
            }
            if !plan.target_segments.contains(&dest_seg) {
                plan.target_segments.push(dest_seg);
            }
        } else {
            // Move to next dest
            dest_idx += 1;
            if dest_idx >= sources.len() {
                break;
            }
            dest_used = sources[dest_idx].1;
        }
    }

    if !plan.is_empty() {
        info!(
            "compaction plan: {} moves, {} bytes, {} source -> {} target segments",
            plan.move_count(),
            plan.total_bytes,
            plan.source_segments.len(),
            plan.target_segments.len()
        );
    }

    plan
}

/// Execute an extract-compact on a single segment.
///
/// Validates spans (no overlaps), compacts in-place by moving
/// data forward to fill gaps, then truncates the file.
pub fn extract_compact_segment(
    file: &mut File,
    spans: &mut [DataSpan],
    mover: &mut CompactionFileMover,
) -> Result<u64> {
    if spans.is_empty() {
        return Ok(0);
    }

    // Validate no overlaps
    validate_spans(spans)?;

    // Get original file size for computing bytes saved
    let original_size = file.metadata().map_err(|e| {
        StorageError::Archive(format!("failed to stat file for compaction: {e}"))
    })?.len();

    // Sort by offset (validate_spans already sorts)
    let mut write_pos = 0u64;

    for span in spans.iter() {
        if span.offset > write_pos {
            // Gap detected: move data forward
            mover.compact_in_place(file, span.offset, write_pos, span.length)?;
        }
        write_pos += span.length;
    }

    // Truncate file to new size
    let bytes_saved = original_size.saturating_sub(write_pos);
    if bytes_saved > 0 {
        file.set_len(write_pos).map_err(|e| {
            StorageError::Archive(format!("failed to truncate after compaction: {e}"))
        })?;
        debug!("extract-compact: saved {} bytes, new size {}", bytes_saved, write_pos);
    }

    Ok(bytes_saved)
}

/// Result of a compaction operation.
#[derive(Debug, Default)]
pub struct CompactionResult {
    /// Number of segments compacted.
    pub segments_compacted: usize,
    /// Total bytes reclaimed.
    pub bytes_reclaimed: u64,
    /// Number of entries whose index was updated.
    pub entries_updated: usize,
    /// Segments that can be deleted (fully emptied by merge).
    pub segments_to_delete: Vec<u16>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::storage::SegmentHeader;
    use tempfile::tempdir;

    #[test]
    fn test_data_span_overlap() {
        let a = DataSpan {
            offset: 0,
            length: 100,
        };
        let b = DataSpan {
            offset: 50,
            length: 100,
        };
        let c = DataSpan {
            offset: 100,
            length: 50,
        };
        let d = DataSpan {
            offset: 200,
            length: 50,
        };

        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c)); // adjacent, not overlapping
        assert!(!a.overlaps(&d));
        assert!(!c.overlaps(&d));
    }

    #[test]
    fn test_validate_spans_ok() {
        let mut spans = vec![
            DataSpan {
                offset: 100,
                length: 50,
            },
            DataSpan {
                offset: 0,
                length: 50,
            },
            DataSpan {
                offset: 200,
                length: 30,
            },
        ];
        assert!(validate_spans(&mut spans).is_ok());
    }

    #[test]
    fn test_validate_spans_overlap() {
        let mut spans = vec![
            DataSpan {
                offset: 0,
                length: 100,
            },
            DataSpan {
                offset: 50,
                length: 100,
            },
        ];
        assert!(validate_spans(&mut spans).is_err());
    }

    #[test]
    fn test_validate_spans_empty() {
        let mut spans: Vec<DataSpan> = vec![];
        assert!(validate_spans(&mut spans).is_ok());
    }

    #[test]
    fn test_validate_spans_single() {
        let mut spans = vec![DataSpan {
            offset: 0,
            length: 100,
        }];
        assert!(validate_spans(&mut spans).is_ok());
    }

    #[test]
    fn test_buffer_sizing() {
        // Minimum budget
        let mover = CompactionFileMover::new(0);
        assert!(mover.buffer_size() >= MIN_BUFFER_SIZE);
        assert_eq!(mover.buffer_count(), 1);

        // Exactly 128 KiB
        let mover = CompactionFileMover::new(MIN_BUFFER_SIZE);
        assert_eq!(mover.buffer_count(), 1);
        assert_eq!(mover.buffer_size(), MIN_BUFFER_SIZE);

        // 2 MiB -> 2^21 >> 17 = 16 buffers
        let mover = CompactionFileMover::new(2 * 1024 * 1024);
        assert_eq!(mover.buffer_count(), 16);
        assert_eq!(mover.buffer_size(), 2 * 1024 * 1024 / 16);

        // 512 KiB -> 4 buffers
        let mover = CompactionFileMover::new(512 * 1024);
        assert_eq!(mover.buffer_count(), 4);
    }

    #[test]
    fn test_file_mover_move_data() {
        let dir = tempdir().expect("tempdir");
        let src_path = dir.path().join("source");
        let dst_path = dir.path().join("dest");

        // Create source file with known data
        let data: Vec<u8> = (0..1024u32).map(|i| (i % 256) as u8).collect();
        std::fs::write(&src_path, &data).expect("write source");
        std::fs::write(&dst_path, vec![0u8; 1024]).expect("write dest");

        let mut mover = CompactionFileMover::new(MIN_BUFFER_SIZE);
        let mut src = File::open(&src_path).expect("open src");
        let mut dst = OpenOptions::new()
            .write(true)
            .open(&dst_path)
            .expect("open dst");

        mover
            .move_data(&mut src, 100, &mut dst, 200, 500)
            .expect("move");

        assert_eq!(mover.bytes_moved(), 500);

        // Verify data was moved correctly
        let result = std::fs::read(&dst_path).expect("read dest");
        assert_eq!(&result[200..700], &data[100..600]);
    }

    #[test]
    fn test_compact_in_place() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("data");

        // Create file with gaps: [AAA][---][BBB][---][CCC]
        let mut data = vec![0u8; 500];
        data[0..100].fill(0xAA);
        data[200..300].fill(0xBB);
        data[400..500].fill(0xCC);
        std::fs::write(&path, &data).expect("write");

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open");

        let mut mover = CompactionFileMover::new(MIN_BUFFER_SIZE);

        let mut spans = vec![
            DataSpan {
                offset: 0,
                length: 100,
            },
            DataSpan {
                offset: 200,
                length: 100,
            },
            DataSpan {
                offset: 400,
                length: 100,
            },
        ];

        let saved = extract_compact_segment(&mut file, &mut spans, &mut mover).expect("compact");
        assert_eq!(saved, 200); // Two 100-byte gaps removed

        // Verify file was truncated
        let result = std::fs::read(&path).expect("read");
        assert_eq!(result.len(), 300);
        assert_eq!(&result[0..100], &[0xAA; 100]);
        assert_eq!(&result[100..200], &[0xBB; 100]);
        assert_eq!(&result[200..300], &[0xCC; 100]);
    }

    #[test]
    fn test_backup_write_and_load() {
        let dir = tempdir().expect("tempdir");
        let mut backup = ExtractorCompactorBackup::new(dir.path());

        backup.record_segment(5).expect("record 5");
        backup.record_segment(10).expect("record 10");
        backup.record_segment(200).expect("record 200");

        // Load and verify
        let loaded = ExtractorCompactorBackup::load(dir.path())
            .expect("load")
            .expect("should exist");

        assert_eq!(loaded.segments(), &[5, 10, 200]);
    }

    #[test]
    fn test_backup_recovery_empty_dir() {
        let dir = tempdir().expect("tempdir");
        let loaded = ExtractorCompactorBackup::load(dir.path()).expect("load");
        assert!(loaded.is_none());
    }

    #[test]
    fn test_backup_remove() {
        let dir = tempdir().expect("tempdir");
        let mut backup = ExtractorCompactorBackup::new(dir.path());
        backup.record_segment(1).expect("record");

        let path = backup.path.clone();
        assert!(path.exists());

        backup.remove().expect("remove");
        assert!(!path.exists());
    }

    #[test]
    fn test_plan_archive_merge_not_enough_sources() {
        // Need at least 2 sources to merge
        let segments = vec![SegmentInfo::new(0, SegmentHeader::default())];
        let plan = plan_archive_merge(&segments, 0.5, 1024 * 1024);
        assert!(plan.is_empty());
    }

    #[test]
    fn test_extract_compact_no_gaps() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("data");

        // Contiguous data, no gaps
        let data = vec![0xAA; 300];
        std::fs::write(&path, &data).expect("write");

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open");

        let mut mover = CompactionFileMover::new(MIN_BUFFER_SIZE);
        let mut spans = vec![
            DataSpan {
                offset: 0,
                length: 100,
            },
            DataSpan {
                offset: 100,
                length: 100,
            },
            DataSpan {
                offset: 200,
                length: 100,
            },
        ];

        let saved = extract_compact_segment(&mut file, &mut spans, &mut mover).expect("compact");
        assert_eq!(saved, 0); // No gaps to reclaim
    }
}
