//! Install executor: drives the cascette-installation InstallPipeline.
//!
//! Creates up to three concurrent TACT update operations matching Blizzard
//! Agent behavior:
//!
//! 1. **Game content operation**: downloads game data to
//!    `install_path/subfolder/` (e.g., `_classic_era_/`)
//! 2. **Bootstrapper operation**: downloads the setup/provisioner binary
//!    from the `bts` product to `install_path/` (no subfolder), using the
//!    `product_tag` (e.g., `"wow"`) to find the `Region` row in `bts/versions`.
//! 3. **Launcher binary operation**: downloads the launcher binary from the
//!    `bts` product `"launcher"` branch to `install_path/` (no subfolder),
//!    using the `bootstrapper_branch` (e.g., `"launcher"`) to find the
//!    `Region` row in `bts/versions`. The launcher binary is extracted
//!    to the install root (no subfolder).

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use cascette_protocol::product_config;

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{Operation, OperationState};
use crate::models::product::{InstallationMode, ProductStatus};
use crate::server::router::AppState;

use super::helpers::{
    ProgressBridge, build_install_config, build_launcher_install_config, resolve_cdn_info,
    resolve_product_metadata, resolve_version_from_wago,
};

/// Execute an install operation.
///
/// 1. Parse install_path/region/locale from operation parameters
/// 2. Query Ribbit for version and CDN metadata
/// 3. Resolve launcher/bootstrapper metadata (product config or fallback)
/// 4. Build InstallConfig and create progress bridge
/// 5. Run game content + launcher pipelines concurrently, racing against cancellation
/// 6. Update product state on success or failure
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

    // Transition product to Installing and read subfolder, country, and platform fields.
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
            metadata.product_config_hash,
            metadata.config_path,
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

    // --- Bootstrapper + launcher binary resolution ---
    // Resolve both bts operations: bootstrapper (setup binary, Region==product_tag)
    // and launcher binary (Region==bootstrapper_branch=="launcher").
    // Skip for historical/pinned builds: BTS resolution fetches current live
    // versions which would overwrite the game's .build.info with BTS metadata.
    let is_historical = custom_build_config.is_some() && custom_cdn_config.is_some();
    let (bootstrapper_config, launcher_binary_config) = if is_historical {
        info!(
            product = %operation.product_code,
            "skipping BTS/launcher resolution for historical build"
        );
        (None, None)
    } else {
        resolve_bts_configs(
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
        .await
    };

    // Create progress bridge
    let (bridge, flush_handle) = ProgressBridge::new(operation.operation_id, state);
    let callback = bridge.callback();

    // Spawn bootstrapper install as a concurrent task if resolved.
    let bootstrapper_handle = if let Some(bs_cfg) = bootstrapper_config {
        let cdn = Arc::clone(&state.cdn_client);
        let bs_endpoints = bs_cfg.endpoints.clone();
        Some(tokio::spawn(async move {
            info!("starting concurrent bootstrapper install");
            let pipeline = cascette_installation::InstallPipeline::new(bs_cfg);
            let result = pipeline.run(cdn, bs_endpoints, |_event| {}).await;
            match &result {
                Ok(report) => {
                    info!(
                        downloaded = report.downloaded,
                        failed = report.failed,
                        "bootstrapper install completed"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "bootstrapper install failed (non-fatal)");
                }
            }
            result
        }))
    } else {
        debug!(
            product = %operation.product_code,
            "no bootstrapper config resolved, skipping bootstrapper install"
        );
        None
    };

    // Spawn launcher binary install as a concurrent task if resolved.
    let launcher_binary_handle = if let Some(lb_cfg) = launcher_binary_config {
        let cdn = Arc::clone(&state.cdn_client);
        let lb_endpoints = lb_cfg.endpoints.clone();
        Some(tokio::spawn(async move {
            info!("starting concurrent launcher binary install");
            let pipeline = cascette_installation::InstallPipeline::new(lb_cfg);
            let result = pipeline.run(cdn, lb_endpoints, |_event| {}).await;
            match &result {
                Ok(report) => {
                    info!(
                        downloaded = report.downloaded,
                        failed = report.failed,
                        "launcher binary install completed"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "launcher binary install failed (non-fatal)");
                }
            }
            result
        }))
    } else {
        debug!(
            product = %operation.product_code,
            "no launcher binary config resolved, skipping launcher binary install"
        );
        None
    };

    // Run the game content install pipeline, racing against cancellation
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
            return Err(AgentError::Cancelled(
                format!("install of {} cancelled", operation.product_code)
            ));
        }
    };

    // Stop the progress flush task
    flush_handle.abort();

    // Wait for bootstrapper install to finish (non-blocking for main result).
    if let Some(handle) = bootstrapper_handle {
        match handle.await {
            Ok(Ok(report)) => {
                info!(
                    downloaded = report.downloaded,
                    "bootstrapper install completed successfully"
                );
            }
            Ok(Err(e)) => {
                warn!(error = %e, "bootstrapper install failed (non-fatal)");
            }
            Err(e) => {
                warn!(error = %e, "bootstrapper install task panicked (non-fatal)");
            }
        }
    }

    // Wait for launcher binary install to finish (non-blocking for main result).
    if let Some(handle) = launcher_binary_handle {
        match handle.await {
            Ok(Ok(report)) => {
                info!(
                    downloaded = report.downloaded,
                    "launcher binary install completed successfully"
                );
            }
            Ok(Err(e)) => {
                warn!(error = %e, "launcher binary install failed (non-fatal)");
            }
            Err(e) => {
                warn!(error = %e, "launcher binary install task panicked (non-fatal)");
            }
        }
    }

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
                product.product_config_hash.clone_from(&product_config_hash);
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

/// Resolve both bts install configurations: bootstrapper and launcher binary.
///
/// Returns a tuple of `(bootstrapper_config, launcher_binary_config)`. Either
/// or both may be `None` if resolution fails.
///
/// The bootstrapper (operation 2) uses `product_tag` (e.g., `"wow"`) to find
/// the setup binary in `bts/versions`. The launcher binary (operation 3) uses
/// `bootstrapper_branch` (e.g., `"launcher"`) to find the shared launcher
/// build in `bts/versions`.
///
/// Tries two approaches for product tag / branch resolution:
/// 1. Fetch product config JSON from CDN and extract `launcher_install_info`
/// 2. Use hardcoded product tag + default bootstrapper branch fallback
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
    // Try to get product tag and bootstrapper branch from product config JSON.
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
                            bootstrapper_product = %info.bootstrapper_product,
                            product_tag = %info.product_tag,
                            bootstrapper_branch = %info.bootstrapper_branch,
                            "resolved launcher info from product config"
                        );
                        (
                            Some(info.product_tag.clone()),
                            Some(info.bootstrapper_branch.clone()),
                        )
                    } else {
                        debug!(
                            product = %product_code,
                            "product config has no launcher_install_info"
                        );
                        (None, None)
                    }
                }
                Err(e) => {
                    warn!(
                        product = %product_code,
                        error = %e,
                        "failed to fetch product config, trying fallback"
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

    // Fallback: use hardcoded product tag mapping and default branch.
    let product_tag = product_tag.or_else(|| {
        let tag = product_config::fallback_product_tag(product_code)?;
        info!(
            product = %product_code,
            product_tag = %tag,
            "using fallback product tag for bts resolution"
        );
        Some(tag.to_string())
    });

    let bootstrapper_branch = bootstrapper_branch
        .unwrap_or_else(|| product_config::DEFAULT_BOOTSTRAPPER_BRANCH.to_string());

    let cdn_overrides = state.config.cdn_endpoint_overrides();

    // --- Operation 2: Bootstrapper (setup binary) ---
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
                    "resolved bootstrapper metadata"
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
                    "failed to resolve bootstrapper metadata"
                );
                None
            }
        }
    } else {
        debug!(
            product = %product_code,
            "no product tag available, skipping bootstrapper install"
        );
        None
    };

    // --- Operation 3: Launcher binary ---
    // Uses bootstrapper_branch (typically "launcher") as the Region lookup
    // in bts/versions, finding the shared launcher build.
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
                "resolved launcher binary metadata"
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
                "failed to resolve launcher binary metadata (non-fatal)"
            );
            None
        }
    };

    (bootstrapper_config, launcher_binary_config)
}
