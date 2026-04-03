//! Extract executor: drives the cascette-installation ExtractPipeline.
//!
//! Resolves manifests from CDN (using the stored build/cdn config hashes from
//! the product registry), then extracts CASC content to the requested output
//! directory.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::server::router::AppState;

use super::helpers::{ProgressBridge, build_install_config, resolve_product_metadata};

/// Execute an extract operation.
///
/// Parameters (from operation.parameters JSON):
/// - `install_path`: path to the CASC installation (must contain `Data/`)
/// - `output_path`: target directory for extracted files
/// - `region`: CDN region (default "us")
/// - `pattern`: optional glob-style filter (e.g., "Interface/*")
pub async fn execute(
    operation: &mut Operation,
    state: &Arc<AppState>,
    cancellation: &CancellationToken,
) -> AgentResult<()> {
    let params = operation
        .parameters
        .as_ref()
        .ok_or_else(|| AgentError::InvalidConfig("extract operation missing parameters".into()))?;

    let install_path = params
        .get("install_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AgentError::InvalidConfig("missing install_path parameter".into()))?
        .to_string();

    let output_path = params
        .get("output_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AgentError::InvalidConfig("missing output_path parameter".into()))?
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

    let pattern = params
        .get("pattern")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    // Retrieve stored build/cdn config hashes from the product registry so we
    // can fetch manifests without hitting Ribbit again.
    let (build_config_hash, cdn_config_hash) =
        match state.registry.get(&operation.product_code).await {
            Ok(product) => {
                let bc = product.build_config.ok_or_else(|| {
                    AgentError::InvalidConfig(format!(
                        "product {} has no stored build_config; install first",
                        operation.product_code
                    ))
                })?;
                let cc = product.cdn_config.ok_or_else(|| {
                    AgentError::InvalidConfig(format!(
                        "product {} has no stored cdn_config; install first",
                        operation.product_code
                    ))
                })?;
                (bc, cc)
            }
            Err(_) => {
                return Err(AgentError::InvalidConfig(format!(
                    "product {} not found in registry",
                    operation.product_code
                )));
            }
        };

    // Transition to Downloading (manifest resolution phase)
    operation.transition_to(OperationState::Downloading)?;
    state.queue.update(operation).await?;

    // Resolve CDN metadata (endpoints only; build/cdn config already known)
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
        build_config = %build_config_hash,
        endpoints = metadata.endpoints.len(),
        "resolved CDN endpoints, fetching manifests for extract"
    );

    // Build InstallConfig with the known hashes to drive manifest resolution
    let install_config = build_install_config(
        &operation.product_code,
        &install_path,
        &metadata.cdn_path,
        metadata.endpoints.clone(),
        &region,
        &locale,
        Some(build_config_hash),
        Some(cdn_config_hash),
        None,
    );

    // Fetch manifests (build config -> CDN config -> encoding -> install manifest)
    let manifests = cascette_installation::pipeline::metadata::resolve_manifests(
        &install_config,
        state.cdn_client.as_ref(),
        &metadata.endpoints,
        &|_| {},
    )
    .await?;

    // Build ExtractConfig
    let extract_config = cascette_installation::config::ExtractConfig {
        install_path: std::path::PathBuf::from(&install_path),
        output_path: std::path::PathBuf::from(&output_path),
        platform_tags: install_config.platform_tags.clone(),
        locale: locale.clone(),
        pattern,
        max_concurrent: 8,
    };

    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();

    let pipeline = cascette_installation::ExtractPipeline::new(extract_config);

    let pipeline_result = tokio::select! {
        result = pipeline.run(manifests.install, callback) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            return Err(AgentError::Cancelled(
                format!("extract of {} cancelled", operation.product_code)
            ));
        }
    };

    flush_handle.abort();

    match pipeline_result {
        Ok(report) => {
            info!(
                product = %operation.product_code,
                extracted = report.extracted,
                failed = report.failed,
                skipped = report.skipped,
                "extract pipeline completed"
            );

            operation.transition_to(OperationState::Verifying)?;
            state.queue.update(operation).await?;
            operation.transition_to(OperationState::Complete)?;
            state.queue.update(operation).await?;

            if report.failed > 0 {
                warn!(
                    product = %operation.product_code,
                    failed = report.failed,
                    "extract completed with failures"
                );
            }

            Ok(())
        }
        Err(e) => {
            warn!(
                product = %operation.product_code,
                error = %e,
                "extract pipeline failed"
            );
            Err(e.into())
        }
    }
}
