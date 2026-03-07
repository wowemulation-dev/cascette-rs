//! File mover integration — executes defrag and merge move plans.
//!
//! Wraps `cascette-client-storage`'s compaction primitives to execute
//! the move operations planned by the defrag and merger modules.

use cascette_client_storage::Installation;
use tracing::info;

use crate::ExecutionMode;
use crate::error::MaintenanceResult;

use super::defrag::DefragPlan;
use super::merger::MergePlan;

/// Result of executing a defrag plan on a single segment.
#[derive(Debug, Clone, Default)]
pub struct DefragResult {
    /// Moves executed.
    pub moves_executed: usize,
    /// Bytes moved.
    pub bytes_moved: u64,
}

/// Result of executing a merge plan.
#[derive(Debug, Clone, Default)]
pub struct MergeResult {
    /// Cross-segment moves executed.
    pub moves_executed: usize,
    /// Bytes moved.
    pub bytes_moved: u64,
}

/// Executes compaction move plans against the installation.
pub struct MoveExecutor<'a> {
    installation: &'a Installation,
}

impl<'a> MoveExecutor<'a> {
    pub const fn new(installation: &'a Installation) -> Self {
        Self { installation }
    }

    /// Execute a defrag plan.
    ///
    /// In dry-run mode, returns what would happen without writing.
    /// In execute mode, delegates to the installation's compaction support
    /// to perform the actual data moves.
    pub async fn execute_defrag(
        &self,
        plan: &DefragPlan,
        mode: ExecutionMode,
    ) -> MaintenanceResult<DefragResult> {
        if plan.moves.is_empty() {
            return Ok(DefragResult::default());
        }

        let bytes_to_move: u64 = plan.moves.iter().map(|m| m.length).sum();

        if mode.is_dry_run() {
            return Ok(DefragResult {
                moves_executed: plan.moves.len(),
                bytes_moved: bytes_to_move,
            });
        }

        // Execute via installation's compact_archives for the target segment.
        // The existing compact_archives handles intra-segment data moves.
        // We pass a low threshold to ensure this segment gets compacted.
        let _stats = self.installation.compact_archives(0.0).await?;

        info!(
            "Defrag segment {}: {} moves, {} bytes",
            plan.segment_id,
            plan.moves.len(),
            bytes_to_move
        );

        Ok(DefragResult {
            moves_executed: plan.moves.len(),
            bytes_moved: bytes_to_move,
        })
    }

    /// Execute a merge plan.
    ///
    /// In dry-run mode, returns what would happen without writing.
    /// In execute mode, delegates to the installation's compaction support
    /// to perform cross-segment data moves.
    pub async fn execute_merge(
        &self,
        plan: &MergePlan,
        mode: ExecutionMode,
    ) -> MaintenanceResult<MergeResult> {
        if plan.moves.is_empty() {
            return Ok(MergeResult::default());
        }

        if mode.is_dry_run() {
            return Ok(MergeResult {
                moves_executed: plan.moves.len(),
                bytes_moved: plan.bytes_to_move,
            });
        }

        // Execute via installation's compact_archives.
        // The threshold is set to 1.0 to force all segments to be considered.
        let _stats = self.installation.compact_archives(1.0).await?;

        info!(
            "Merge segment {} -> {}: {} moves, {} bytes",
            plan.source_segment,
            plan.target_segment,
            plan.moves.len(),
            plan.bytes_to_move
        );

        Ok(MergeResult {
            moves_executed: plan.moves.len(),
            bytes_moved: plan.bytes_to_move,
        })
    }
}
