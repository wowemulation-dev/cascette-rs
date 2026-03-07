//! Backfill executor: resumes a partial install with partial-priority classification.
//!
//! Delegates to the install pipeline with `backfill_mode = true`, which causes
//! `classify_backfill_artifacts` to be used instead of `classify_download_artifacts`.
//! All remaining download manifest entries are promoted to the highest priority
//! bucket, prioritising partially-downloaded files
//! over not-yet-started ones.
//!
//! Also spawns concurrent bootstrapper and launcher binary installs matching the
//! three-operation architecture in the install executor.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use cascette_protocol::product_config;

use crate::error::AgentResult;
use crate::models::operation::{Operation, OperationState};
use crate::models::product::{InstallationMode, ProductStatus};
use crate::server::router::AppState;

use super::helpers::{
    ProgressBridge, build_install_config, build_launcher_install_config, resolve_cdn_info,
    resolve_product_metadata, resolve_version_from_wago,
};

/// Execute a backfill operation.
///
/// Identical to install, except:
/// - `backfill_mode = true` is set on the `InstallConfig`, which causes the
///   download manifest classifier to promote all remaining files to the highest
///   priority bucket.
/// - `resume = true` is always set so the existing checkpoint is honoured and
///   already-downloaded files are skipped.
/// - A concurrent launcher install is also spawned (same as install executor).
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

    let (subfolder, account_country, geo_ip_country, platform, architecture) =
        if let Ok(mut product) = state.registry.get(&operation.product_code).await {
            let sub = product.subfolder.clone();
            let acct = product.account_country.clone();
            let geo = product.geo_ip_country.clone();
            let plat = product.platform.clone();
            let arch = product.architecture.clone();
            product.transition_to(ProductStatus::Installing)?;
            state.registry.update(&product).await?;
            (sub, acct, geo, plat, arch)
        } else {
            (None, None, None, None, None)
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

    let (
        build_config_hash,
        cdn_config_hash,
        version_name,
        cdn_path,
        endpoints,
        keyring_hash,
        product_config_hash,
        config_path,
    ) = if let (Some(bc), Some(cc)) = (&custom_build_config, &custom_cdn_config) {
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

        (
            bc.clone(),
            cc.clone(),
            version,
            cdn_path,
            endpoints,
            None,
            None,
            None,
        )
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
            metadata.product_config_hash,
            metadata.config_path,
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
        endpoints.clone(),
        &region,
        &locale,
        Some(build_config_hash.clone()),
        Some(cdn_config_hash.clone()),
        subfolder,
        account_country,
        geo_ip_country,
        platform.as_deref(),
        architecture.as_deref(),
    );
    install_config.key_store = Some(key_provider);
    // Enable backfill mode: remaining DL manifest entries are promoted to
    // highest priority to prioritise partially-downloaded files.
    install_config.backfill_mode = true;
    install_config.resume = true;

    // --- Bootstrapper + launcher binary resolution ---
    let (bootstrapper_config, launcher_binary_config) = resolve_bts_configs(
        state,
        &operation.product_code,
        &endpoints,
        product_config_hash.as_deref(),
        config_path.as_deref(),
        &region,
        &locale,
        &install_path,
        platform.as_deref(),
        architecture.as_deref(),
    )
    .await;

    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();

    // Spawn bootstrapper install as a concurrent task if resolved.
    let bootstrapper_handle = if let Some(bs_cfg) = bootstrapper_config {
        let cdn = Arc::clone(&state.cdn_client);
        let bs_endpoints = bs_cfg.endpoints.clone();
        Some(tokio::spawn(async move {
            info!("starting concurrent bootstrapper install (backfill)");
            let pipeline = cascette_installation::InstallPipeline::new(bs_cfg);
            let result = pipeline.run(cdn, bs_endpoints, |_event| {}).await;
            match &result {
                Ok(report) => {
                    info!(
                        downloaded = report.downloaded,
                        failed = report.failed,
                        "bootstrapper install completed (backfill)"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "bootstrapper install failed (non-fatal, backfill)");
                }
            }
            result
        }))
    } else {
        debug!(
            product = %operation.product_code,
            "no bootstrapper config resolved, skipping bootstrapper install (backfill)"
        );
        None
    };

    // Spawn launcher binary install as a concurrent task if resolved.
    let launcher_binary_handle = if let Some(lb_cfg) = launcher_binary_config {
        let cdn = Arc::clone(&state.cdn_client);
        let lb_endpoints = lb_cfg.endpoints.clone();
        Some(tokio::spawn(async move {
            info!("starting concurrent launcher binary install (backfill)");
            let pipeline = cascette_installation::InstallPipeline::new(lb_cfg);
            let result = pipeline.run(cdn, lb_endpoints, |_event| {}).await;
            match &result {
                Ok(report) => {
                    info!(
                        downloaded = report.downloaded,
                        failed = report.failed,
                        "launcher binary install completed (backfill)"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "launcher binary install failed (non-fatal, backfill)");
                }
            }
            result
        }))
    } else {
        debug!(
            product = %operation.product_code,
            "no launcher binary config resolved, skipping launcher binary install (backfill)"
        );
        None
    };

    let pipeline_endpoints = install_config.endpoints.clone();
    let pipeline = cascette_installation::InstallPipeline::new(install_config);
    let cdn = Arc::clone(&state.cdn_client);

    let pipeline_result = tokio::select! {
        result = pipeline.run(cdn, pipeline_endpoints, callback) => result,
        () = cancellation.cancelled() => {
            flush_handle.abort();
            if let Some(handle) = bootstrapper_handle {
                handle.abort();
            }
            if let Some(handle) = launcher_binary_handle {
                handle.abort();
            }
            return Err(crate::error::AgentError::Cancelled(
                format!("backfill of {} cancelled", operation.product_code)
            ));
        }
    };

    flush_handle.abort();

    // Wait for bootstrapper install to finish (non-blocking for main result).
    if let Some(handle) = bootstrapper_handle {
        match handle.await {
            Ok(Ok(report)) => {
                info!(
                    downloaded = report.downloaded,
                    "bootstrapper install completed successfully (backfill)"
                );
            }
            Ok(Err(e)) => {
                warn!(error = %e, "bootstrapper install failed (non-fatal, backfill)");
            }
            Err(e) => {
                warn!(error = %e, "bootstrapper install task panicked (non-fatal, backfill)");
            }
        }
    }

    // Wait for launcher binary install to finish (non-blocking for main result).
    if let Some(handle) = launcher_binary_handle {
        match handle.await {
            Ok(Ok(report)) => {
                info!(
                    downloaded = report.downloaded,
                    "launcher binary install completed successfully (backfill)"
                );
            }
            Ok(Err(e)) => {
                warn!(error = %e, "launcher binary install failed (non-fatal, backfill)");
            }
            Err(e) => {
                warn!(error = %e, "launcher binary install task panicked (non-fatal, backfill)");
            }
        }
    }

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

/// Resolve both bts install configurations: bootstrapper and launcher binary.
///
/// Same logic as `install::resolve_bts_configs`. Returns a tuple of
/// `(bootstrapper_config, launcher_binary_config)`.
#[allow(clippy::too_many_arguments)]
async fn resolve_bts_configs(
    state: &Arc<AppState>,
    product_code: &str,
    endpoints: &[cascette_protocol::CdnEndpoint],
    product_config_hash: Option<&str>,
    config_path: Option<&str>,
    region: &str,
    locale: &str,
    install_path: &str,
    platform: Option<&str>,
    architecture: Option<&str>,
) -> (
    Option<cascette_installation::config::InstallConfig>,
    Option<cascette_installation::config::InstallConfig>,
) {
    let (product_tag, bootstrapper_branch) =
        if let (Some(hash), Some(cfg_path)) = (product_config_hash, config_path) {
            match product_config::fetch_product_config(
                state.cdn_client.http_client(),
                endpoints,
                cfg_path,
                hash,
            )
            .await
            {
                Ok(config) => {
                    if let Some(info) = config.launcher_install_info() {
                        debug!(
                            product = %product_code,
                            product_tag = %info.product_tag,
                            bootstrapper_branch = %info.bootstrapper_branch,
                            "resolved launcher info from product config (backfill)"
                        );
                        (
                            Some(info.product_tag.clone()),
                            Some(info.bootstrapper_branch.clone()),
                        )
                    } else {
                        (None, None)
                    }
                }
                Err(e) => {
                    warn!(
                        product = %product_code,
                        error = %e,
                        "failed to fetch product config (backfill), trying fallback"
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

    let product_tag = product_tag.or_else(|| {
        let tag = product_config::fallback_product_tag(product_code)?;
        info!(
            product = %product_code,
            product_tag = %tag,
            "using fallback product tag for bts resolution (backfill)"
        );
        Some(tag.to_string())
    });

    let bootstrapper_branch = bootstrapper_branch
        .unwrap_or_else(|| product_config::DEFAULT_BOOTSTRAPPER_BRANCH.to_string());

    let cdn_overrides = state.config.cdn_endpoint_overrides();

    // Operation 2: Bootstrapper (setup binary)
    let bootstrapper_config = if let Some(ref tag) = product_tag {
        match super::helpers::resolve_bootstrapper_metadata(
            &state.ribbit_client,
            tag,
            region,
            cdn_overrides.as_deref(),
        )
        .await
        {
            Ok(bts_metadata) => {
                info!(
                    product_tag = %tag,
                    bts_version = %bts_metadata.version_name,
                    bts_build_config = %bts_metadata.build_config,
                    bts_cdn_config = %bts_metadata.cdn_config,
                    "resolved bootstrapper metadata (backfill)"
                );

                let mut config = build_launcher_install_config(
                    install_path,
                    &bts_metadata.cdn_path,
                    bts_metadata.endpoints,
                    region,
                    locale,
                    bts_metadata.build_config,
                    bts_metadata.cdn_config,
                    platform,
                    architecture,
                    None, // Bootstrapper manifest has only platform tags
                );

                let keyring = if let Some(ref kh) = bts_metadata.keyring_hash {
                    super::helpers::fetch_keyring(&state.cdn_client, &config.endpoints, kh).await
                } else {
                    None
                };
                config.key_store = Some(super::helpers::build_key_provider(keyring.as_ref()));

                Some(config)
            }
            Err(e) => {
                warn!(
                    product_tag = %tag,
                    error = %e,
                    "failed to resolve bootstrapper metadata (backfill)"
                );
                None
            }
        }
    } else {
        None
    };

    // Operation 3: Launcher binary
    let launcher_binary_config = match super::helpers::resolve_bootstrapper_metadata(
        &state.ribbit_client,
        &bootstrapper_branch,
        region,
        cdn_overrides.as_deref(),
    )
    .await
    {
        Ok(lb_metadata) => {
            info!(
                bootstrapper_branch = %bootstrapper_branch,
                version = %lb_metadata.version_name,
                build_config = %lb_metadata.build_config,
                cdn_config = %lb_metadata.cdn_config,
                "resolved launcher binary metadata (backfill)"
            );

            let mut config = build_launcher_install_config(
                install_path,
                &lb_metadata.cdn_path,
                lb_metadata.endpoints,
                region,
                locale,
                lb_metadata.build_config,
                lb_metadata.cdn_config,
                platform,
                architecture,
                product_tag.as_deref(), // Filter shared launcher manifest to this product
            );

            let keyring = if let Some(ref kh) = lb_metadata.keyring_hash {
                super::helpers::fetch_keyring(&state.cdn_client, &config.endpoints, kh).await
            } else {
                None
            };
            config.key_store = Some(super::helpers::build_key_provider(keyring.as_ref()));

            Some(config)
        }
        Err(e) => {
            warn!(
                bootstrapper_branch = %bootstrapper_branch,
                error = %e,
                "failed to resolve launcher binary metadata (non-fatal, backfill)"
            );
            None
        }
    };

    (bootstrapper_config, launcher_binary_config)
}
