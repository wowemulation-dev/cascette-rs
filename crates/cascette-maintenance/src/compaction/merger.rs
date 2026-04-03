//! Archive merger — consolidates low-utilization segments.
//!
//! Matches `casc::ArchiveMerger` from the Agent.exe spec. Sorts segments by
//! utilization and merges the least-utilized into the most-utilized targets
//! that have enough free space.

use cascette_client_storage::IndexEntry;

use super::defrag::SEGMENT_HEADER_SIZE;

/// A planned cross-segment data move.
#[derive(Debug, Clone)]
pub struct CrossSegmentMove {
    /// Source segment ID.
    pub src_segment: u16,
    /// Source offset within the source segment.
    pub src_offset: u64,
    /// Destination segment ID.
    pub dst_segment: u16,
    /// Destination offset within the target segment.
    pub dst_offset: u64,
    /// Number of bytes to move.
    pub length: u64,
    /// Truncated encoding key for the moved entry.
    pub key: [u8; 9],
}

/// A plan to merge one source segment into a target segment.
#[derive(Debug, Clone)]
pub struct MergePlan {
    /// Source segment to empty.
    pub source_segment: u16,
    /// Target segment receiving data.
    pub target_segment: u16,
    /// Cross-segment move operations.
    pub moves: Vec<CrossSegmentMove>,
    /// Total bytes to move.
    pub bytes_to_move: u64,
}

/// Segment descriptor for merge planning.
#[derive(Debug, Clone)]
struct SegmentInfo {
    id: u16,
    entries: Vec<IndexEntry>,
    file_size: u64,
    used_bytes: u64,
}

impl SegmentInfo {
    fn utilization(&self) -> f64 {
        if self.file_size == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.used_bytes as f64 / self.file_size as f64
        }
    }

    fn free_bytes(&self) -> u64 {
        self.file_size.saturating_sub(self.used_bytes)
    }
}

/// Plan merges for a set of segments.
///
/// `segments` is a slice of `(segment_id, entries, file_size)` tuples.
/// `merge_threshold` controls which segments are eligible for merging —
/// segments with utilization below this threshold are merge candidates.
/// Must be in `[0.0, 0.4]`.
///
/// # Errors
///
/// Returns an empty vec if `merge_threshold` is outside `[0.0, 0.4]`.
pub fn plan_merge(
    segments: &[(u16, Vec<IndexEntry>, u64)],
    merge_threshold: f64,
) -> Vec<MergePlan> {
    if !(0.0..=0.4).contains(&merge_threshold) {
        return Vec::new();
    }

    if segments.is_empty() {
        return Vec::new();
    }

    // Build segment info with used-byte calculations
    let mut infos: Vec<SegmentInfo> = segments
        .iter()
        .map(|(id, entries, file_size)| {
            let used_bytes =
                entries.iter().map(|e| u64::from(e.size)).sum::<u64>() + SEGMENT_HEADER_SIZE;
            SegmentInfo {
                id: *id,
                entries: entries.clone(),
                file_size: *file_size,
                used_bytes,
            }
        })
        .collect();

    // Sort by utilization ascending — least utilized first as merge sources
    infos.sort_by(|a, b| {
        a.utilization()
            .partial_cmp(&b.utilization())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut plans = Vec::new();
    let mut merged_sources: Vec<bool> = vec![false; infos.len()];

    // For each low-utilization source, find the best target
    for src_idx in 0..infos.len() {
        if merged_sources[src_idx] {
            continue;
        }
        if infos[src_idx].utilization() >= merge_threshold {
            continue;
        }
        if infos[src_idx].entries.is_empty() {
            continue;
        }

        let src_data_size = infos[src_idx]
            .used_bytes
            .saturating_sub(SEGMENT_HEADER_SIZE);

        // Find best target: highest utilization that has enough free space
        let mut best_target: Option<usize> = None;
        for tgt_idx in (0..infos.len()).rev() {
            if tgt_idx == src_idx || merged_sources[tgt_idx] {
                continue;
            }
            if infos[tgt_idx].free_bytes() >= src_data_size {
                best_target = Some(tgt_idx);
                break;
            }
        }

        if let Some(tgt_idx) = best_target {
            let mut moves = Vec::new();
            let mut dst_offset = infos[tgt_idx].used_bytes;
            let mut bytes_to_move = 0u64;

            for entry in &infos[src_idx].entries {
                let length = u64::from(entry.size);
                moves.push(CrossSegmentMove {
                    src_segment: infos[src_idx].id,
                    src_offset: u64::from(entry.archive_location.archive_offset),
                    dst_segment: infos[tgt_idx].id,
                    dst_offset,
                    length,
                    key: entry.key,
                });
                dst_offset += length;
                bytes_to_move += length;
            }

            // Update target's used bytes for subsequent planning
            infos[tgt_idx].used_bytes += bytes_to_move;
            merged_sources[src_idx] = true;

            plans.push(MergePlan {
                source_segment: infos[src_idx].id,
                target_segment: infos[tgt_idx].id,
                moves,
                bytes_to_move,
            });
        }
    }

    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use cascette_client_storage::IndexEntry;
    use cascette_client_storage::index::ArchiveLocation;

    fn make_entry(segment: u16, offset: u32, size: u32, key: [u8; 9]) -> IndexEntry {
        IndexEntry {
            key,
            archive_location: ArchiveLocation {
                archive_id: segment,
                archive_offset: offset,
            },
            size,
        }
    }

    #[test]
    fn empty_segments_no_plan() {
        let plans = plan_merge(&[], 0.3);
        assert!(plans.is_empty());
    }

    #[test]
    fn threshold_out_of_range_no_plan() {
        let segments = vec![(0, vec![], 1000)];
        assert!(plan_merge(&segments, -0.1).is_empty());
        assert!(plan_merge(&segments, 0.5).is_empty());
        assert!(plan_merge(&segments, 1.0).is_empty());
    }

    #[test]
    fn two_low_utilization_segments_merge() {
        // used_bytes = entry_size + SEGMENT_HEADER_SIZE (480)
        // Segment 0: used = 100 + 480 = 580 / 10000 = 5.8% — below threshold
        // Segment 1: used = 2000 + 480 = 2480 / 10000 = 24.8% — below threshold
        // Segment 0 is least utilized, should merge into segment 1
        let src_entries = vec![make_entry(0, SEGMENT_HEADER_SIZE as u32, 100, [1; 9])];
        let tgt_entries = vec![make_entry(1, SEGMENT_HEADER_SIZE as u32, 2000, [2; 9])];

        let segments = vec![(0, src_entries, 10000), (1, tgt_entries, 10000)];

        let plans = plan_merge(&segments, 0.4);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].source_segment, 0);
        assert_eq!(plans[0].target_segment, 1);
        assert_eq!(plans[0].moves.len(), 1);
        assert_eq!(plans[0].bytes_to_move, 100);
    }

    #[test]
    fn no_merge_when_above_threshold() {
        let entries = vec![make_entry(0, SEGMENT_HEADER_SIZE as u32, 800, [1; 9])];
        let segments = vec![(0, entries, 1000)]; // 80% utilized
        let plans = plan_merge(&segments, 0.3);
        assert!(plans.is_empty());
    }
}
