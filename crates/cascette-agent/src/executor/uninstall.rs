//! Uninstall executor: checks for running processes and removes installation files.
//!
//! Matches Agent.exe's `HasMultipleProducts` check: if another product shares
//! the same install path, only product-specific files are removed; the shared
//! `Data/` directory is preserved.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::ProductStatus;
use crate::process_detection;
use crate::server::router::AppState;

/// Product-specific files removed on uninstall (matching Agent.exe's task list).
///
/// These are relative to the product install path. `Data/` is only removed when
/// no other product shares the install path.
const PRODUCT_SPECIFIC_FILES: &[&str] = &[
    ".flavor.info",
    ".agent.db",
    ".build.info",
    ".patch.result",
    ".product.db",
    "Launcher.db",
];

/// Execute an uninstall operation.
///
/// Checks that no game process is running. If another product shares the same
/// install path (`HasMultipleProducts` check), only product-specific files are
/// removed. Otherwise the entire installation directory is deleted.
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    _cancellation: &CancellationToken,
) -> AgentResult<()> {
    // Check for running game process
    if process_detection::is_game_running(&operation.product_code)
        || state
            .session_tracker
            .is_active(&operation.product_code)
            .await
    {
        return Err(AgentError::GameProcessRunning {
            product: operation.product_code.clone(),
            operation: "uninstall".to_string(),
        });
    }

    let product = state.registry.get(&operation.product_code).await?;
    let install_path = product.install_path.clone();

    // Transition product to Uninstalling
    {
        let mut product = state.registry.get(&operation.product_code).await?;
        product.transition_to(ProductStatus::Uninstalling)?;
        state.registry.update(&product).await?;
    }

    // Use Downloading state as "processing" placeholder
    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    if let Some(ref path) = install_path {
        let path_buf = PathBuf::from(path);
        if path_buf.exists() {
            // HasMultipleProducts: check if any other installed product shares this path.
            let has_sibling = has_other_product_at_path(state, &operation.product_code, path).await;

            if has_sibling {
                // Shared path — remove only product-specific files, preserve Data/.
                info!(
                    product = %operation.product_code,
                    path = %path,
                    "shared install path detected; removing product-specific files only"
                );
                remove_product_specific_files(&path_buf).await;
            } else {
                // Sole occupant — remove the entire installation directory.
                info!(
                    product = %operation.product_code,
                    path = %path,
                    "removing installation directory"
                );
                if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                    warn!(
                        product = %operation.product_code,
                        path = %path,
                        error = %e,
                        "failed to remove installation directory"
                    );
                }
            }
        }
    }

    operation.transition_to(OperationState::Verifying)?;
    state.queue.update(operation).await?;

    operation.transition_to(OperationState::Complete)?;
    state.queue.update(operation).await?;

    // Return product to Available
    if let Ok(mut product) = state.registry.get(&operation.product_code).await {
        product.transition_to(ProductStatus::Available)?;
        product.version = None;
        product.install_path = None;
        product.size_bytes = None;
        product.installation_mode = None;
        product.is_update_available = false;
        product.available_version = None;
        state.registry.update(&product).await?;
    }

    info!(
        product = %operation.product_code,
        "uninstall completed"
    );

    Ok(())
}

/// Returns true if any product other than `this_product` has an install path
/// equal to `path` and is in an installed state.
async fn has_other_product_at_path(state: &Arc<AppState>, this_product: &str, path: &str) -> bool {
    let Ok(all) = state.registry.list().await else {
        return false;
    };
    all.iter().any(|p| {
        p.product_code != this_product
            && p.install_path.as_deref() == Some(path)
            && matches!(
                p.status,
                ProductStatus::Installed | ProductStatus::Corrupted
            )
    })
}

/// Remove product-specific files from an install directory, leaving `Data/` intact.
async fn remove_product_specific_files(base: &Path) {
    for name in PRODUCT_SPECIFIC_FILES {
        let target = base.join(name);
        if target.exists()
            && let Err(e) = tokio::fs::remove_file(&target).await
        {
            warn!(path = %target.display(), error = %e, "failed to remove product file");
        }
    }
}
