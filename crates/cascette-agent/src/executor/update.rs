//! Update executor: drives the cascette-installation UpdatePipeline for delta updates.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use cascette_client_storage::Installation;

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::ProductStatus;
use crate::server::router::AppState;

use super::helpers::{
    ProgressBridge, build_install_config, build_update_config, resolve_cdn_info,
    resolve_product_metadata, resolve_version_from_wago,
};

/// Execute an update operation.
///
/// Queries Ribbit for the latest version, compares with stored version,
/// and runs UpdatePipeline for delta updates. Falls back to InstallPipeline
/// if the product has no stored config hashes (pre-migration installs).
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    // Get installed product metadata
    let product = state.registry.get(&operation.product_code).await?;

    let install_path =
        product
            .install_path
            .as_deref()
            .ok_or_else(|| AgentError::InvalidProductState {
                product: operation.product_code.clone(),
                status: product.status.to_string(),
                operation: "update".to_string(),
            })?;

    let region = product.region.as_deref().unwrap_or("us");
    let locale = product.locale.as_deref().unwrap_or(&state.config.locale);

    // Transition product to Updating
    {
        let mut product = state.registry.get(&operation.product_code).await?;
        product.transition_to(ProductStatus::Updating)?;
        state.registry.update(&product).await?;
    }

    // Extract optional custom target configs from operation params
    let custom_target = operation.parameters.as_ref().and_then(|p| {
        let bc = p.get("build_config")?.as_str()?.to_string();
        let cc = p.get("cdn_config")?.as_str()?.to_string();
        Some((bc, cc))
    });

    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    // Resolve target version: either custom configs or latest from Ribbit
    let (target_build_config, target_cdn_config, version_name, cdn_path, endpoints, keyring_hash) =
        if let Some((bc, cc)) = custom_target {
            // Custom target: skip Ribbit version query
            let version = resolve_version_from_wago(&state.wago, &operation.product_code, &bc)
                .await
                .unwrap_or_else(|| "unknown".to_string());

            let (cdn_path, official_endpoints) =
                resolve_cdn_info(&state.ribbit_client, &operation.product_code, region).await?;

            let mirror_config = cascette_installation::mirror::MirrorConfig {
                official: official_endpoints,
                use_community_mirrors: true,
                is_historic: true,
            };
            let endpoints = mirror_config.build_endpoint_list(&cdn_path);

            info!(
                product = %operation.product_code,
                version = %version,
                build_config = %bc,
                endpoints = endpoints.len(),
                "using custom configs for historical update target"
            );

            (bc, cc, version, cdn_path, endpoints, None)
        } else {
            // Standard flow: query Ribbit for latest version
            let cdn_overrides = state.config.cdn_endpoint_overrides();
            let metadata = resolve_product_metadata(
                &state.ribbit_client,
                &state.cdn_client,
                &operation.product_code,
                region,
                cdn_overrides.as_deref(),
            )
            .await?;

            // Check if update is needed
            if product.version.as_deref() == Some(&metadata.version_name) {
                info!(
                    product = %operation.product_code,
                    version = %metadata.version_name,
                    "product is already at latest version, no update needed"
                );

                operation.transition_to(OperationState::Verifying)?;
                state.queue.update(operation).await?;
                operation.transition_to(OperationState::Complete)?;
                state.queue.update(operation).await?;

                if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                    product.transition_to(ProductStatus::Installed)?;
                    product.is_update_available = false;
                    state.registry.update(&product).await?;
                }
                return Ok(());
            }

            (
                metadata.build_config,
                metadata.cdn_config,
                metadata.version_name,
                metadata.cdn_path,
                metadata.endpoints,
                metadata.keyring_hash,
            )
        };

    // Fetch keyring config and build key provider
    let keyring = if let Some(ref kh) = keyring_hash {
        super::helpers::fetch_keyring(&state.cdn_client, &endpoints, kh).await
    } else {
        None
    };
    let key_provider = super::helpers::build_key_provider(keyring.as_ref());

    info!(
        product = %operation.product_code,
        from = product.version.as_deref().unwrap_or("unknown"),
        to = %version_name,
        "updating product"
    );

    // Create progress bridge
    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();
    let cdn = Arc::clone(&state.cdn_client);

    // Choose pipeline based on whether base config hashes are available.
    // Products installed before schema v3 lack these hashes and need the
    // InstallPipeline fallback (which re-downloads via checkpoint/resume).
    let pipeline_result = if let (Some(base_build), Some(base_cdn)) =
        (&product.build_config, &product.cdn_config)
    {
        info!(
            base_build = %base_build,
            target_build = %target_build_config,
            "running update pipeline with delta classification"
        );

        let mut update_config = build_update_config(
            &operation.product_code,
            install_path,
            &cdn_path,
            endpoints.clone(),
            region,
            locale,
            base_build.clone(),
            base_cdn.clone(),
            target_build_config.clone(),
            target_cdn_config.clone(),
        );
        update_config.key_store = Some(key_provider.clone());
        update_config.game_subfolder.clone_from(&product.subfolder);

        let installation = Installation::open(update_config.install_path.join("Data"))
            .map_err(|e| AgentError::InvalidConfig(format!("failed to open installation: {e}")))?;
        installation.initialize().await.map_err(|e| {
            AgentError::InvalidConfig(format!("failed to initialize installation: {e}"))
        })?;
        let installation = Arc::new(installation);

        let update_endpoints = update_config.endpoints.clone();
        let pipeline = cascette_installation::UpdatePipeline::new(update_config);

        tokio::select! {
            result = pipeline.run(cdn, update_endpoints, installation, callback) => {
                result.map_err(AgentError::from)
            }
            () = cancellation.cancelled() => {
                flush_handle.abort();
                return Err(AgentError::Cancelled(
                    format!("update of {} cancelled", operation.product_code)
                ));
            }
        }
    } else {
        warn!(
            product = %operation.product_code,
            "no base config hashes stored, falling back to install pipeline"
        );

        let mut install_config = build_install_config(
            &operation.product_code,
            install_path,
            &cdn_path,
            endpoints.clone(),
            region,
            locale,
            Some(target_build_config.clone()),
            Some(target_cdn_config.clone()),
            None,
        );
        install_config.key_store = Some(key_provider.clone());

        let install_endpoints = install_config.endpoints.clone();
        let pipeline = cascette_installation::InstallPipeline::new(install_config);

        tokio::select! {
            result = pipeline.run(cdn, install_endpoints, callback) => {
                // Map InstallReport to a compatible shape
                match result {
                    Ok(report) => Ok(cascette_installation::pipeline::update::UpdateReport {
                        missing_files: report.downloaded + report.failed + report.skipped,
                        missing_bytes: report.bytes_downloaded,
                        written_bytes: report.bytes_downloaded,
                        leech_count: 0,
                        leech_bytes: 0,
                        leech_failed: 0,
                        downloaded_bytes: report.bytes_downloaded,
                        retried: 0,
                        patchable: 0,
                        patch_applied: 0,
                        patch_failed: 0,
                        indices_downloaded: 0,
                        obsolete_removed: 0,
                        failed_files: vec![],
                    }),
                    Err(e) => Err(AgentError::from(e)),
                }
            }
            () = cancellation.cancelled() => {
                flush_handle.abort();
                return Err(AgentError::Cancelled(
                    format!("update of {} cancelled", operation.product_code)
                ));
            }
        }
    };

    flush_handle.abort();

    match pipeline_result {
        Ok(report) => {
            info!(
                product = %operation.product_code,
                downloaded_bytes = report.downloaded_bytes,
                missing = report.missing_files,
                patchable = report.patchable,
                patch_applied = report.patch_applied,
                "update pipeline completed"
            );

            operation.transition_to(OperationState::Verifying)?;
            state.queue.update(operation).await?;
            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                product.transition_to(ProductStatus::Installed)?;
                product.version = Some(version_name.clone());
                product.is_update_available = false;
                product.available_version = None;
                product.build_config = Some(target_build_config);
                product.cdn_config = Some(target_cdn_config);
                state.registry.update(&product).await?;
            }

            if !report.failed_files.is_empty() {
                warn!(
                    product = %operation.product_code,
                    failed = report.failed_files.len(),
                    "update completed with failed files"
                );
            }

            Ok(())
        }
        Err(e) => {
            warn!(
                product = %operation.product_code,
                error = %e,
                "update pipeline failed"
            );

            // Revert to Installed (update failed but files should be intact)
            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                let _ = product.transition_to(ProductStatus::Installed);
                let _ = state.registry.update(&product).await;
            }

            Err(e)
        }
    }
}
