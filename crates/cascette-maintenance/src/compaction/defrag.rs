//! Defragmentation algorithm — shifts live data backward to fill gaps.
//!
//! Matches `casc::CompactAlgorithm::Defrag` from the Agent.exe spec.
//! Walks entries in offset order within a segment and creates move operations
//! to close gaps left by deleted content.

use cascette_client_storage::IndexEntry;

/// Segment header size in bytes (0x1E0 = 480).
pub const SEGMENT_HEADER_SIZE: u64 = 0x1E0;

/// A planned move of data within a single segment.
#[derive(Debug, Clone)]
pub struct MoveOp {
    /// Source offset in the segment file.
    pub src_offset: u64,
    /// Destination offset (always <= src_offset).
    pub dst_offset: u64,
    /// Number of bytes to move.
    pub length: u64,
}

/// Result of planning a defrag pass on a single segment.
#[derive(Debug, Clone)]
pub struct DefragPlan {
    /// Segment being defragmented.
    pub segment_id: u16,
    /// Ordered list of move operations.
    pub moves: Vec<MoveOp>,
    /// Total bytes recoverable by executing this plan.
    pub bytes_recoverable: u64,
}

/// Plan defragmentation for a single segment.
///
/// `entries` must be pre-sorted by offset and filtered to the target segment.
/// The algorithm walks entries sequentially, tracking the expected write
/// position. Any gap between the expected position and the actual entry offset
/// generates a `MoveOp` to shift the entry backward.
pub fn plan_defrag(segment_id: u16, entries: &[IndexEntry]) -> DefragPlan {
    let mut moves = Vec::new();
    let mut expected_pos = SEGMENT_HEADER_SIZE;

    for entry in entries {
        let entry_offset = u64::from(entry.archive_location.archive_offset);
        let entry_size = u64::from(entry.size);

        if entry_offset > expected_pos {
            // Gap detected — move this entry backward
            moves.push(MoveOp {
                src_offset: entry_offset,
                dst_offset: expected_pos,
                length: entry_size,
            });
        }
        // Advance expected position regardless of whether we moved
        let actual_start = if entry_offset > expected_pos {
            expected_pos
        } else {
            entry_offset
        };
        expected_pos = actual_start + entry_size;
    }

    // Total bytes recoverable = last entry end position vs what would be
    // the end position after defrag
    let original_end = entries.last().map_or(SEGMENT_HEADER_SIZE, |e| {
        u64::from(e.archive_location.archive_offset) + u64::from(e.size)
    });

    let bytes_recoverable = original_end.saturating_sub(expected_pos);

    DefragPlan {
        segment_id,
        moves,
        bytes_recoverable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cascette_client_storage::IndexEntry;
    use cascette_client_storage::index::ArchiveLocation;

    fn make_entry(offset: u32, size: u32) -> IndexEntry {
        IndexEntry {
            key: [0; 9],
            archive_location: ArchiveLocation {
                archive_id: 0,
                archive_offset: offset,
            },
            size,
        }
    }

    #[test]
    fn no_gaps_produces_no_moves() {
        // Entries packed tightly after header
        let entries = vec![
            make_entry(SEGMENT_HEADER_SIZE as u32, 100),
            make_entry(SEGMENT_HEADER_SIZE as u32 + 100, 200),
        ];
        let plan = plan_defrag(0, &entries);
        assert!(plan.moves.is_empty());
        assert_eq!(plan.bytes_recoverable, 0);
    }

    #[test]
    fn single_gap_produces_one_move() {
        let entries = vec![
            make_entry(SEGMENT_HEADER_SIZE as u32, 100),
            // 50-byte gap
            make_entry(SEGMENT_HEADER_SIZE as u32 + 150, 200),
        ];
        let plan = plan_defrag(0, &entries);
        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.moves[0].src_offset, SEGMENT_HEADER_SIZE + 150);
        assert_eq!(plan.moves[0].dst_offset, SEGMENT_HEADER_SIZE + 100);
        assert_eq!(plan.moves[0].length, 200);
        assert_eq!(plan.bytes_recoverable, 50);
    }

    #[test]
    fn empty_entries_produces_no_op() {
        let plan = plan_defrag(0, &[]);
        assert!(plan.moves.is_empty());
        assert_eq!(plan.bytes_recoverable, 0);
    }

    #[test]
    fn multiple_gaps() {
        let entries = vec![
            make_entry(SEGMENT_HEADER_SIZE as u32, 100),
            // 30-byte gap
            make_entry(SEGMENT_HEADER_SIZE as u32 + 130, 50),
            // 20-byte gap
            make_entry(SEGMENT_HEADER_SIZE as u32 + 200, 80),
        ];
        let plan = plan_defrag(0, &entries);
        assert_eq!(plan.moves.len(), 2);
        assert_eq!(plan.bytes_recoverable, 50); // 30 + 20
    }
}
