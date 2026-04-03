//! Fill-holes analysis — estimates free space without moving data.
//!
//! Matches `casc::CompactAlgorithm::Fillholes` from the Agent.exe spec.
//! This is an analysis-only pass that reports gaps in a segment without
//! generating move operations.

use cascette_client_storage::IndexEntry;

use super::defrag::SEGMENT_HEADER_SIZE;

/// Result of analyzing gaps in a single segment.
#[derive(Debug, Clone)]
pub struct FillholesAnalysis {
    /// Segment analyzed.
    pub segment_id: u16,
    /// Total free bytes across all gaps.
    pub total_free: u64,
    /// Size of the largest single gap.
    pub max_gap: u64,
    /// Number of gaps found.
    pub gap_count: usize,
}

/// Analyze gaps in a segment without planning moves.
///
/// `entries` must be pre-sorted by offset and filtered to the target segment.
/// Walks entries sequentially and records each gap between the expected
/// position and the actual entry offset.
pub fn analyze_fillholes(segment_id: u16, entries: &[IndexEntry]) -> FillholesAnalysis {
    let mut total_free: u64 = 0;
    let mut max_gap: u64 = 0;
    let mut gap_count: usize = 0;
    let mut expected_pos = SEGMENT_HEADER_SIZE;

    for entry in entries {
        let entry_offset = u64::from(entry.archive_location.archive_offset);
        let entry_size = u64::from(entry.size);

        if entry_offset > expected_pos {
            let gap = entry_offset - expected_pos;
            total_free += gap;
            if gap > max_gap {
                max_gap = gap;
            }
            gap_count += 1;
        }

        expected_pos = entry_offset + entry_size;
    }

    FillholesAnalysis {
        segment_id,
        total_free,
        max_gap,
        gap_count,
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
    fn no_gaps() {
        let entries = vec![
            make_entry(SEGMENT_HEADER_SIZE as u32, 100),
            make_entry(SEGMENT_HEADER_SIZE as u32 + 100, 200),
        ];
        let analysis = analyze_fillholes(0, &entries);
        assert_eq!(analysis.total_free, 0);
        assert_eq!(analysis.max_gap, 0);
        assert_eq!(analysis.gap_count, 0);
    }

    #[test]
    fn sparse_segment() {
        let entries = vec![
            make_entry(SEGMENT_HEADER_SIZE as u32, 100),
            make_entry(SEGMENT_HEADER_SIZE as u32 + 200, 50), // 100-byte gap
            make_entry(SEGMENT_HEADER_SIZE as u32 + 500, 80), // 250-byte gap
        ];
        let analysis = analyze_fillholes(0, &entries);
        assert_eq!(analysis.total_free, 350);
        assert_eq!(analysis.max_gap, 250);
        assert_eq!(analysis.gap_count, 2);
    }

    #[test]
    fn empty_segment() {
        let analysis = analyze_fillholes(0, &[]);
        assert_eq!(analysis.total_free, 0);
        assert_eq!(analysis.gap_count, 0);
    }
}
