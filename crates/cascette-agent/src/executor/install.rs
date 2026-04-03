//! Install executor: drives the cascette-installation InstallPipeline.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::{InstallationMode, ProductStatus};
use crate::server::router::AppState;

use super::helpers::{
    ProgressBridge, build_install_config, resolve_cdn_info, resolve_product_metadata,
    resolve_version_from_wago,
};

/// Execute an install operation.
///
/// 1. Parse install_path/region/locale from operation parameters
/// 2. Query Ribbit for version and CDN metadata
/// 3. Build InstallConfig and create progress bridge
/// 4. Run InstallPipeline, racing against cancellation
/// 5. Update product state on success or failure
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    // Extract parameters from the operation
    let params = operation
        .parameters
        .as_ref()
        .ok_or_else(|| AgentError::InvalidConfig("install operation missing parameters".into()))?;

    let install_path = params
        .get("install_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AgentError::InvalidConfig("missing install_path parameter".into()))?
        .to_string();

    let region = params
        .get("region")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("us")
        .to_string();

    let locale = params
        .get("locale")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&state.config.locale)
        .to_string();

    // Transition product to Installing and read subfolder
    let subfolder = if let Ok(mut product) = state.registry.get(&operation.product_code).await {
        let sub = product.subfolder.clone();
        product.transition_to(ProductStatus::Installing)?;
        state.registry.update(&product).await?;
        sub
    } else {
        None
    };

    // Extract optional custom config hashes for historical builds
    let custom_build_config = params
        .get("build_config")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let custom_cdn_config = params
        .get("cdn_config")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    // Transition to Downloading (metadata resolution + download phase)
    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    // Resolve metadata: either from Ribbit (latest version) or custom configs
    let (build_config_hash, cdn_config_hash, version_name, cdn_path, endpoints, keyring_hash) =
        if let (Some(bc), Some(cc)) = (&custom_build_config, &custom_cdn_config) {
            // Custom historical build: skip Ribbit version query
            let version = resolve_version_from_wago(&state.wago, &operation.product_code, bc)
                .await
                .unwrap_or_else(|| "unknown".to_string());

            let (cdn_path, official_endpoints) =
                resolve_cdn_info(&state.ribbit_client, &operation.product_code, &region).await?;

            // Use MirrorConfig to prioritize community mirrors for historical builds
            let mirror_config = cascette_installation::mirror::MirrorConfig {
                official: official_endpoints,
                use_community_mirrors: true,
                is_historic: true,
            };
            let mut endpoints = mirror_config.build_endpoint_list(&cdn_path);

            // Prepend any operator-configured CDN overrides (e.g. a local mirror)
            // so they are tried first before community and official endpoints.
            if let Some(overrides) = state.config.cdn_endpoint_overrides() {
                endpoints = overrides.into_iter().chain(endpoints).collect();
            }

            info!(
                product = %operation.product_code,
                version = %version,
                build_config = %bc,
                endpoints = endpoints.len(),
                "using custom configs for historical install"
            );

            (bc.clone(), cc.clone(), version, cdn_path, endpoints, None)
        } else {
            // Standard flow: query Ribbit for latest version
            let cdn_overrides = state.config.cdn_endpoint_overrides();
            let metadata = resolve_product_metadata(
                &state.ribbit_client,
                &state.cdn_client,
                &operation.product_code,
                &region,
                cdn_overrides.as_deref(),
            )
            .await?;

            info!(
                product = %operation.product_code,
                version = %metadata.version_name,
                build_config = %metadata.build_config,
                endpoints = metadata.endpoints.len(),
                "resolved metadata, starting install pipeline"
            );

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

    // Build install configuration
    let mut install_config = build_install_config(
        &operation.product_code,
        &install_path,
        &cdn_path,
        endpoints,
        &region,
        &locale,
        Some(build_config_hash.clone()),
        Some(cdn_config_hash.clone()),
        subfolder,
    );
    install_config.key_store = Some(key_provider);

    // Create progress bridge
    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);

    let callback = bridge.callback();

    // Run the install pipeline, racing against cancellation
    let pipeline_endpoints = install_config.endpoints.clone();
    let pipeline = cascette_installation::InstallPipeline::new(install_config);
    let cdn = Arc::clone(&state.cdn_client);

    let pipeline_result = tokio::select! {
        result = pipeline.run(cdn, pipeline_endpoints, callback) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            return Err(AgentError::Cancelled(
                format!("install of {} cancelled", operation.product_code)
            ));
        }
    };

    // Stop the progress flush task
    flush_handle.abort();

    match pipeline_result {
        Ok(report) => {
            info!(
                product = %operation.product_code,
                downloaded = report.downloaded,
                failed = report.failed,
                skipped = report.skipped,
                bytes = report.bytes_downloaded,
                "install pipeline completed"
            );

            // Transition through Verifying -> Complete
            operation.transition_to(OperationState::Verifying)?;
            state.queue.update(operation).await?;

            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

            // Update product to Installed with metadata
            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                product.transition_to(ProductStatus::Installed)?;
                product.version = Some(version_name.clone());
                product.install_path = Some(install_path);
                product.region = Some(region);
                product.locale = Some(locale);
                product.installation_mode = Some(InstallationMode::Casc);
                product.build_config = Some(build_config_hash.clone());
                product.cdn_config = Some(cdn_config_hash.clone());
                state.registry.update(&product).await?;
            }

            if report.failed > 0 {
                warn!(
                    product = %operation.product_code,
                    failed = report.failed,
                    "install completed with failed files"
                );
            }

            Ok(())
        }
        Err(e) => {
            warn!(
                product = %operation.product_code,
                error = %e,
                "install pipeline failed"
            );

            // Partial install: mark as corrupted if files were written
            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                product.install_path = Some(install_path);
                // Try to go back to Available; if that fails (because Installing
                // can also go to Installed or stay), just leave it
                let _ = product.transition_to(ProductStatus::Available);
                let _ = state.registry.update(&product).await;
            }

            Err(e.into())
        }
    }
}
