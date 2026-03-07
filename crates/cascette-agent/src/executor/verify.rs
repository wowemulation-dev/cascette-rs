//! Verify executor: drives the cascette-installation VerifyPipeline.

use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use cascette_installation::{VerifyConfig, VerifyMode, VerifyPipeline};

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::ProductStatus;
use crate::server::router::AppState;

use super::helpers::ProgressBridge;

/// Execute a verify operation.
///
/// Runs the VerifyPipeline against the installed product files. If
/// invalid or missing files are detected, the product is marked Corrupted.
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    let product = state.registry.get(&operation.product_code).await?;

    let install_path =
        product
            .install_path
            .as_deref()
            .ok_or_else(|| AgentError::InvalidProductState {
                product: operation.product_code.clone(),
                status: product.status.to_string(),
                operation: "verify".to_string(),
            })?;

    // Transition product to Verifying
    {
        let mut product = state.registry.get(&operation.product_code).await?;
        product.transition_to(ProductStatus::Verifying)?;
        state.registry.update(&product).await?;
    }

    // Skip downloading phase -- verify is local-only
    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    operation.transition_to(OperationState::Verifying)?;
    state.queue.update(operation).await?;

    info!(
        product = %operation.product_code,
        install_path = %install_path,
        "starting verification pipeline"
    );

    let config = VerifyConfig {
        install_path: PathBuf::from(install_path),
        mode: VerifyMode::Full,
    };

    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();

    let pipeline = VerifyPipeline::new(config);

    let pipeline_result = tokio::select! {
        result = pipeline.run(callback) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            return Err(AgentError::Cancelled(
                format!("verify of {} cancelled", operation.product_code)
            ));
        }
    };

    flush_handle.abort();

    match pipeline_result {
        Ok(report) => {
            info!(
                product = %operation.product_code,
                total = report.total,
                valid = report.valid,
                invalid = report.invalid,
                missing = report.missing,
                "verification completed"
            );

            // Store summary in operation metadata
            operation.metadata = Some(serde_json::json!({
                "total": report.total,
                "valid": report.valid,
                "invalid": report.invalid,
                "missing": report.missing,
            }));

            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                if report.invalid > 0 || report.missing > 0 {
                    warn!(
                        product = %operation.product_code,
                        invalid = report.invalid,
                        missing = report.missing,
                        "verification found issues, marking product corrupted"
                    );
                    product.transition_to(ProductStatus::Corrupted)?;
                } else {
                    product.transition_to(ProductStatus::Installed)?;
                }
                state.registry.update(&product).await?;
            }

            Ok(())
        }
        Err(e) => {
            warn!(
                product = %operation.product_code,
                error = %e,
                "verification pipeline failed"
            );

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                let _ = product.transition_to(ProductStatus::Installed);
                let _ = state.registry.update(&product).await;
            }

            Err(e.into())
        }
    }
}
