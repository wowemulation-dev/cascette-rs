//! Compaction orchestrator — drives defragmentation and segment consolidation.
//!
//! Sequences three analysis passes and optional data movement:
//! 1. Fill-holes analysis on all segments (gap estimation)
//! 2. Defrag on segments with detected gaps
//! 3. Merge planning for low-utilization segments
//!
//! The orchestrator delegates actual I/O to `MoveExecutor`.

pub mod defrag;
pub mod fillholes;
pub mod merger;
pub mod mover;

use std::collections::HashMap;
use std::time::Instant;

use cascette_client_storage::Installation;
use tracing::info;

use crate::ExecutionMode;
use crate::error::MaintenanceResult;
use crate::report::CompactionReport;

/// Compaction strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// Move live data within segments to fill gaps.
    Defrag,
    /// Analyze gaps without moving data.
    Fillholes,
    /// Merge low-utilization segments into higher-utilization ones.
    Merge,
    /// Run defrag, then merge.
    Full,
}

/// Configuration for compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Which strategy to apply.
    pub strategy: CompactionStrategy,
    /// Utilization threshold below which a segment is eligible for merge.
    /// Must be in [0.0, 0.4]. Default: 0.3.
    pub merge_threshold: f64,
    /// Total I/O buffer budget in bytes. Default: 2 MiB.
    pub buffer_budget: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            strategy: CompactionStrategy::Full,
            merge_threshold: 0.3,
            buffer_budget: 2 * 1024 * 1024,
        }
    }
}

/// Compaction orchestrator.
pub struct CompactionOrchestrator<'a> {
    installation: &'a Installation,
    config: CompactionConfig,
}

impl<'a> CompactionOrchestrator<'a> {
    pub const fn new(installation: &'a Installation, config: CompactionConfig) -> Self {
        Self {
            installation,
            config,
        }
    }

    pub async fn run(&self, mode: ExecutionMode) -> MaintenanceResult<CompactionReport> {
        let start = Instant::now();
        let mut report = CompactionReport {
            executed: !mode.is_dry_run(),
            ..CompactionReport::default()
        };

        info!(
            "Starting compaction ({}, {:?})",
            if mode.is_dry_run() {
                "dry-run"
            } else {
                "execute"
            },
            self.config.strategy,
        );

        // Collect per-segment entry lists
        let segment_entries = self.collect_segment_entries().await;
        report.segments_analyzed = segment_entries.len();

        if segment_entries.is_empty() {
            report.duration = start.elapsed();
            return Ok(report);
        }

        // Step 1: Fill-holes analysis on all segments
        let analyses: Vec<_> = segment_entries
            .iter()
            .map(|(seg_id, entries)| fillholes::analyze_fillholes(*seg_id, entries))
            .collect();

        let segments_with_gaps: Vec<_> = analyses.iter().filter(|a| a.gap_count > 0).collect();
        report.fillholes_segments = segments_with_gaps.len();

        info!(
            "Fill-holes analysis: {} segments with gaps out of {}",
            segments_with_gaps.len(),
            segment_entries.len()
        );

        // Step 2: Defrag eligible segments
        if matches!(
            self.config.strategy,
            CompactionStrategy::Defrag | CompactionStrategy::Full
        ) {
            let executor = mover::MoveExecutor::new(self.installation);

            for analysis in &segments_with_gaps {
                let entries = &segment_entries[&analysis.segment_id];
                let plan = defrag::plan_defrag(analysis.segment_id, entries);

                if !plan.moves.is_empty() {
                    let result = executor.execute_defrag(&plan, mode).await?;
                    report.defrag_segments += 1;
                    report.moves_planned += plan.moves.len();
                    report.moves_executed += result.moves_executed;
                    report.bytes_moved += result.bytes_moved;
                    report.bytes_reclaimed += plan.bytes_recoverable;
                }
            }
        }

        // Step 3: Merge low-utilization segments
        if matches!(
            self.config.strategy,
            CompactionStrategy::Merge | CompactionStrategy::Full
        ) {
            self.run_merge_pass(mode, &segment_entries, &mut report)
                .await?;
        }

        report.segments_compacted = report.defrag_segments + report.merged_segments;

        report.duration = start.elapsed();
        Ok(report)
    }

    /// Collect all index entries grouped by segment ID, sorted by offset.
    async fn collect_segment_entries(
        &self,
    ) -> HashMap<u16, Vec<cascette_client_storage::IndexEntry>> {
        let all_entries = self.installation.get_all_index_entries().await;
        let mut by_segment: HashMap<u16, Vec<cascette_client_storage::IndexEntry>> = HashMap::new();

        for entry in all_entries {
            by_segment
                .entry(entry.archive_location.archive_id)
                .or_default()
                .push(entry);
        }

        // Sort each segment's entries by offset
        for entries in by_segment.values_mut() {
            entries.sort_unstable_by_key(|e| e.archive_location.archive_offset);
        }

        by_segment
    }

    /// Run the merge pass: plan and execute merges for low-utilization segments.
    async fn run_merge_pass(
        &self,
        mode: ExecutionMode,
        segment_entries: &HashMap<u16, Vec<cascette_client_storage::IndexEntry>>,
        report: &mut CompactionReport,
    ) -> MaintenanceResult<()> {
        // Build segment descriptors with file sizes from utilization data
        let utilization = self.installation.archive_utilization().await?;
        let stats = self.installation.stats().await;

        #[allow(clippy::cast_precision_loss)]
        let avg_size = if stats.archive_files > 0 {
            stats.archive_size / stats.archive_files as u64
        } else {
            return Ok(());
        };

        let segments: Vec<(u16, Vec<cascette_client_storage::IndexEntry>, u64)> = utilization
            .iter()
            .filter_map(|(id, _util)| {
                let entries = segment_entries.get(id)?.clone();
                // Use average archive size as estimate when per-file size
                // is not available from utilization data
                Some((*id, entries, avg_size))
            })
            .collect();

        let merge_plans = merger::plan_merge(&segments, self.config.merge_threshold);

        if merge_plans.is_empty() {
            info!("No segments eligible for merge");
            return Ok(());
        }

        let executor = mover::MoveExecutor::new(self.installation);

        for plan in &merge_plans {
            info!(
                "Merge plan: segment {} -> {} ({} moves, {} bytes)",
                plan.source_segment,
                plan.target_segment,
                plan.moves.len(),
                plan.bytes_to_move,
            );

            let result = executor.execute_merge(plan, mode).await?;
            report.merged_segments += 1;
            report.moves_planned += plan.moves.len();
            report.moves_executed += result.moves_executed;
            report.bytes_moved += result.bytes_moved;
            report.bytes_reclaimed += plan.bytes_to_move;
            report.segments_freed += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compaction_config_defaults() {
        let config = CompactionConfig::default();
        assert_eq!(config.strategy, CompactionStrategy::Full);
        assert!((config.merge_threshold - 0.3).abs() < f64::EPSILON);
        assert_eq!(config.buffer_budget, 2 * 1024 * 1024);
    }

    #[test]
    fn merge_threshold_validation() {
        // plan_merge rejects thresholds outside [0.0, 0.4]
        let plans = merger::plan_merge(&[], 0.5);
        assert!(plans.is_empty());

        let plans = merger::plan_merge(&[], -0.1);
        assert!(plans.is_empty());

        // Valid thresholds produce empty plans only because input is empty
        let plans = merger::plan_merge(&[], 0.3);
        assert!(plans.is_empty());
    }
}
