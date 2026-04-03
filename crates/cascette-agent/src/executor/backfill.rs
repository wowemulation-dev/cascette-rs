//! Backfill executor: resumes a partial install with partial-priority classification.
//!
//! Delegates to the install pipeline with `backfill_mode = true`, which causes
//! `classify_backfill_artifacts` to be used instead of `classify_download_artifacts`.
//! All remaining download manifest entries are promoted to the highest priority
//! bucket, matching Agent.exe's behavior of prioritising partially-downloaded files
//! over not-yet-started ones.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::error::AgentResult;
use crate::models::operation::{Operation, OperationState};
use crate::models::product::{InstallationMode, ProductStatus};
use crate::server::router::AppState;

use super::helpers::{
    ProgressBridge, build_install_config, resolve_cdn_info, resolve_product_metadata,
    resolve_version_from_wago,
};

/// Execute a backfill operation.
///
/// Identical to install, except:
/// - `backfill_mode = true` is set on the `InstallConfig`, which causes the
///   download manifest classifier to promote all remaining files to the highest
///   priority bucket.
/// - `resume = true` is always set so the existing checkpoint is honoured and
///   already-downloaded files are skipped.
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    let params = operation.parameters.as_ref().ok_or_else(|| {
        crate::error::AgentError::InvalidConfig("backfill operation missing parameters".into())
    })?;

    let install_path = params
        .get("install_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            crate::error::AgentError::InvalidConfig("missing install_path parameter".into())
        })?
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

    let subfolder = if let Ok(mut product) = state.registry.get(&operation.product_code).await {
        let sub = product.subfolder.clone();
        product.transition_to(ProductStatus::Installing)?;
        state.registry.update(&product).await?;
        sub
    } else {
        None
    };

    let custom_build_config = params
        .get("build_config")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let custom_cdn_config = params
        .get("cdn_config")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    let (build_config_hash, cdn_config_hash, version_name, cdn_path, endpoints, keyring_hash) =
        if let (Some(bc), Some(cc)) = (&custom_build_config, &custom_cdn_config) {
            let version = resolve_version_from_wago(&state.wago, &operation.product_code, bc)
                .await
                .unwrap_or_else(|| "unknown".to_string());

            let (cdn_path, official_endpoints) =
                resolve_cdn_info(&state.ribbit_client, &operation.product_code, &region).await?;

            let mirror_config = cascette_installation::mirror::MirrorConfig {
                official: official_endpoints,
                use_community_mirrors: true,
                is_historic: true,
            };
            let endpoints = mirror_config.build_endpoint_list(&cdn_path);

            (bc.clone(), cc.clone(), version, cdn_path, endpoints, None)
        } else {
            let cdn_overrides = state.config.cdn_endpoint_overrides();
            let metadata = resolve_product_metadata(
                &state.ribbit_client,
                &state.cdn_client,
                &operation.product_code,
                &region,
                cdn_overrides.as_deref(),
            )
            .await?;

            (
                metadata.build_config,
                metadata.cdn_config,
                metadata.version_name,
                metadata.cdn_path,
                metadata.endpoints,
                metadata.keyring_hash,
            )
        };

    let keyring = if let Some(ref kh) = keyring_hash {
        super::helpers::fetch_keyring(&state.cdn_client, &endpoints, kh).await
    } else {
        None
    };
    let key_provider = super::helpers::build_key_provider(keyring.as_ref());

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
    // Enable backfill mode: remaining DL manifest entries are promoted to
    // highest priority, matching Agent.exe partial-file prioritisation.
    install_config.backfill_mode = true;
    install_config.resume = true;

    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();

    let pipeline_endpoints = install_config.endpoints.clone();
    let pipeline = cascette_installation::InstallPipeline::new(install_config);
    let cdn = Arc::clone(&state.cdn_client);

    let pipeline_result = tokio::select! {
        result = pipeline.run(cdn, pipeline_endpoints, callback) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            return Err(crate::error::AgentError::Cancelled(
                format!("backfill of {} cancelled", operation.product_code)
            ));
        }
    };

    flush_handle.abort();

    match pipeline_result {
        Ok(report) => {
            tracing::info!(
                product = %operation.product_code,
                downloaded = report.downloaded,
                failed = report.failed,
                skipped = report.skipped,
                bytes = report.bytes_downloaded,
                "backfill pipeline completed"
            );

            operation.transition_to(OperationState::Verifying)?;
            state.queue.update(operation).await?;
            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

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
                tracing::warn!(
                    product = %operation.product_code,
                    failed = report.failed,
                    "backfill completed with failed files"
                );
            }

            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                product = %operation.product_code,
                error = %e,
                "backfill pipeline failed"
            );

            if let Ok(mut product) = state.registry.get(&operation.product_code).await {
                product.install_path = Some(install_path);
                let _ = product.transition_to(ProductStatus::Available);
                let _ = state.registry.update(&product).await;
            }

            Err(e.into())
        }
    }
}
