//! Maintenance operations for local CASC installations.
//!
//! Provides four coordinated operations matching agent.exe's maintenance
//! subsystem:
//!
//! 1. **Preservation set** — collects all encoding keys referenced by the
//!    current build. Protected keys are excluded from removal.
//! 2. **Garbage collection** — removes content not in the preservation set.
//! 3. **Compaction** — defragments segments and consolidates partially-full
//!    archives.
//! 4. **Build repair** — validates content integrity and rebuilds damaged
//!    indices.
//!
//! All operations support dry-run mode via [`ExecutionMode`].
//!
//! Two entry points:
//! - [`run_maintenance`] — index-only, no CDN access required
//! - [`run_maintenance_with_manifests`] — manifest-aware with CDN re-download

pub mod compaction;
pub mod error;
pub mod gc;
pub mod preservation;
pub mod repair;
pub mod report;

pub use error::{MaintenanceError, MaintenanceResult};
pub use report::MaintenanceReport;

/// Controls whether maintenance operations modify storage.
///
/// Every operation accepts this parameter and returns a report regardless of
/// mode. In [`DryRun`](ExecutionMode::DryRun) mode the report describes what
/// *would* happen. In [`Execute`](ExecutionMode::Execute) mode it describes
/// what *did* happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Analyze and report without modifying storage.
    DryRun,
    /// Execute the operation, modifying storage.
    Execute,
}

impl ExecutionMode {
    /// Returns `true` if this is a dry-run.
    pub const fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
    }
}

/// Run the full maintenance pipeline on an installation.
///
/// Sequences: preservation -> GC -> compaction -> repair.
/// Uses index-only preservation and basic repair (no CDN re-download).
///
/// # Errors
///
/// Returns the first error encountered. Partial results are not returned.
pub async fn run_maintenance(
    installation: &cascette_client_storage::Installation,
    mode: ExecutionMode,
) -> MaintenanceResult<MaintenanceReport> {
    use std::time::Instant;

    let start = Instant::now();
    let mut report = MaintenanceReport::default();

    // 1. Build preservation set (index-only)
    let (preservation_set, preservation_report) =
        preservation::PreservationSet::build(installation).await?;
    report.preservation = Some(preservation_report);

    // 2. Garbage collection (without manifest-aware stages)
    let gc = gc::GarbageCollector::new(installation, &preservation_set, gc::GcConfig::default());
    let gc_report = gc.run(mode).await?;
    report.gc = Some(gc_report);

    // 3. Compaction
    let compactor = compaction::CompactionOrchestrator::new(
        installation,
        compaction::CompactionConfig::default(),
    );
    let compaction_report = compactor.run(mode).await?;
    report.compaction = Some(compaction_report);

    // 4. Repair (verification + index rebuild only)
    let repairer = repair::RepairOrchestrator::new(installation, repair::RepairConfig::default());
    let repair_report = repairer.run(mode).await?;
    report.repair = Some(repair_report);

    report.total_duration = start.elapsed();
    Ok(report)
}

/// Run the full maintenance pipeline with manifest awareness and CDN support.
///
/// Sequences: preservation -> GC -> compaction -> repair.
///
/// Compared to [`run_maintenance`], this function:
/// - Builds a manifest-aware preservation set (encoding + install + download keys)
/// - Enables GC stages 2-3 (config file cleanup, CDN index cleanup)
/// - Uses the full repair state machine with CDN re-download for corrupted entries
/// - Repairs loose files (config files, `.build.info`)
///
/// # Errors
///
/// Returns the first error encountered. Partial results are not returned.
pub async fn run_maintenance_with_manifests<S: cascette_installation::CdnSource>(
    installation: &cascette_client_storage::Installation,
    manifests: &cascette_installation::BuildManifests,
    cdn: &S,
    endpoints: &[cascette_protocol::CdnEndpoint],
    mode: ExecutionMode,
    game_subfolder: Option<String>,
) -> MaintenanceResult<MaintenanceReport> {
    use std::time::Instant;

    let start = Instant::now();
    let mut report = MaintenanceReport::default();

    // 1. Build manifest-aware preservation set
    let (preservation_set, preservation_report) =
        preservation::PreservationSet::build_from_manifests(installation, manifests).await?;
    report.preservation = Some(preservation_report);

    // 2. Garbage collection with manifest-aware stages
    let gc = gc::GarbageCollector::with_manifests(
        installation,
        &preservation_set,
        manifests,
        gc::GcConfig::default(),
    );
    let gc_report = gc.run(mode).await?;
    report.gc = Some(gc_report);

    // 3. Compaction
    let compactor = compaction::CompactionOrchestrator::new(
        installation,
        compaction::CompactionConfig::default(),
    );
    let compaction_report = compactor.run(mode).await?;
    report.compaction = Some(compaction_report);

    // 4. Full repair with CDN re-download
    let repairer = repair::BuildRepairOrchestrator::new(
        installation,
        cdn,
        endpoints,
        manifests,
        repair::RepairConfig::default(),
        game_subfolder,
    );
    let repair_report = repairer.run(mode).await?;
    report.repair = Some(repair_report);

    report.total_duration = start.elapsed();
    Ok(report)
}
