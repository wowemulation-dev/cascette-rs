//! Repair executor: drives the full cascette-maintenance BuildRepairOrchestrator.
//!
//! Resolves CDN endpoints and build manifests, opens the local CASC installation,
//! then runs the 11-state repair machine which covers container repair, loose-file
//! hash-and-compare, CDN re-download, and crash-recovery marker files — matching
//! Blizzard Agent's BuildRepairState flow.

use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use cascette_client_storage::Installation;
use cascette_installation::InstallPipeline;
use cascette_maintenance::ExecutionMode;
use cascette_maintenance::repair::{BuildRepairOrchestrator, RepairConfig};

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::ProductStatus;
use crate::server::router::AppState;

use super::helpers::{ProgressBridge, build_install_config, resolve_product_metadata};

/// Execute a repair operation.
///
/// Resolves CDN endpoints, fetches build manifests, then runs the full
/// BuildRepairOrchestrator state machine:
///
/// 1. Container verification (data, ecache, hardlink)
/// 2. Per-file hash-and-compare via LooseFileRepairEngine
/// 3. CDN re-download of corrupted entries via DataRepairEngine
/// 4. Crash-recovery markers (CASCRepair.mrk / RepairMarker.psv)
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    let product = state.registry.get(&operation.product_code).await?;

    let install_path_str = product
        .install_path
        .as_deref()
        .ok_or_else(|| AgentError::InvalidProductState {
            product: operation.product_code.clone(),
            status: product.status.to_string(),
            operation: "repair".to_string(),
        })?
        .to_string();

    let region = product.region.as_deref().unwrap_or("us");

    // Transition product to Repairing
    {
        let mut product = state.registry.get(&operation.product_code).await?;
        product.transition_to(ProductStatus::Repairing)?;
        state.registry.update(&product).await?;
    }

    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    // Resolve CDN endpoints and build/CDN config hashes via Ribbit
    let cdn_overrides = state.config.cdn_endpoint_overrides();
    let metadata = resolve_product_metadata(
        &state.ribbit_client,
        &state.cdn_client,
        &operation.product_code,
        region,
        cdn_overrides.as_deref(),
    )
    .await?;

    info!(
        product = %operation.product_code,
        install_path = %install_path_str,
        endpoints = metadata.endpoints.len(),
        "fetching build manifests for repair"
    );

    // Fetch all build manifests needed by the repair orchestrator
    let install_config = build_install_config(
        &operation.product_code,
        &install_path_str,
        &metadata.cdn_path,
        metadata.endpoints.clone(),
        region,
        "enUS",
        Some(metadata.build_config.clone()),
        Some(metadata.cdn_config.clone()),
        None,
    );

    let cdn = &*state.cdn_client;
    let pipeline = InstallPipeline::new(install_config);
    let manifests = pipeline.resolve_manifests(cdn, &metadata.endpoints).await?;

    // Open the local CASC installation
    let data_path = PathBuf::from(&install_path_str).join("Data");
    let installation = Installation::open(data_path)?;
    installation.initialize().await?;

    info!(
        product = %operation.product_code,
        "starting full repair orchestrator"
    );

    let (_bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);

    let orchestrator = BuildRepairOrchestrator::new(
        &installation,
        cdn,
        &metadata.endpoints,
        &manifests,
        RepairConfig::default(),
        product.subfolder.clone(),
    );

    let repair_result = tokio::select! {
        result = orchestrator.run(ExecutionMode::Execute) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            return Err(AgentError::Cancelled(
                format!("repair of {} cancelled", operation.product_code)
            ));
        }
    };

    flush_handle.abort();

    match repair_result {
        Ok(report) => {
            info!(
                product = %operation.product_code,
                verified = report.entries_verified,
                corrupted = report.entries_corrupted,
                redownloaded = report.entries_redownloaded,
                loose_repaired = report.loose_files_repaired,
                "repair completed"
            );

            // Store summary in operation metadata
            operation.metadata = Some(serde_json::json!({
                "entries_verified": report.entries_verified,
                "entries_valid": report.entries_valid,
                "entries_corrupted": report.entries_corrupted,
                "entries_redownloaded": report.entries_redownloaded,
                "redownload_failed": report.redownload_failed,
                "loose_files_checked": report.loose_files_checked,
                "loose_files_repaired": report.loose_files_repaired,
                "indices_rebuilt": report.indices_rebuilt,
                "markers_written": report.markers_written,
            }));

            operation.transition_to(OperationState::Verifying)?;
            state.queue.update(operation).await?;
            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                if report.entries_corrupted == 0 || report.redownload_failed == 0 {
                    product.transition_to(ProductStatus::Installed)?;
                } else {
                    warn!(
                        product = %operation.product_code,
                        redownload_failed = report.redownload_failed,
                        "repair could not fix all corrupted entries"
                    );
                    product.transition_to(ProductStatus::Corrupted)?;
                }
                state.registry.update(&product).await?;
            }

            Ok(())
        }
        Err(e) => {
            warn!(
                product = %operation.product_code,
                error = %e,
                "repair orchestrator failed"
            );

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                let _ = product.transition_to(ProductStatus::Corrupted);
                let _ = state.registry.update(&product).await;
            }

            Err(AgentError::InvalidConfig(format!("repair failed: {e}")))
        }
    }
}
