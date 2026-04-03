//! Cascette Agent binary entry point.
//!
//! Thin wrapper around the cascette-agent library that:
//! 1. Parses CLI arguments
//! 2. Initializes logging
//! 3. Constructs protocol clients (Ribbit/CDN)
//! 4. Opens the database
//! 5. Starts the HTTP server and operation runner
//! 6. Handles graceful shutdown

use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use tracing::info;

use cascette_agent::config::AgentConfig;
use cascette_agent::executor::OperationRunner;
use cascette_agent::observability;
use cascette_agent::server::router::{AppState, create_router};
use cascette_agent::state::db::Database;
use cascette_import::ImportProvider;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AgentConfig::from_args();

    // Initialize tracing
    observability::init_tracing(&config.log_filter())
        .map_err(|e| anyhow::anyhow!("tracing init: {e}"))?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "cascette-agent starting"
    );

    // Construct protocol clients.
    // ClientConfig supports env-var overrides (CASCETTE_TACT_HTTPS_URL, etc.)
    // and the agent's --version-server-url flag overrides the TACT HTTPS endpoint.
    let mut client_config = cascette_protocol::ClientConfig::from_env().unwrap_or_default();

    if let Some(ref url) = config.version_server_url {
        client_config.tact_https_url.clone_from(url);
    }

    let ribbit_client = Arc::new(cascette_protocol::RibbitTactClient::new(client_config)?);

    let cdn_client = Arc::new(cascette_protocol::CdnClient::new(
        ribbit_client.cache().clone(),
        cascette_protocol::CdnConfig::default(),
    )?);

    info!("protocol clients initialized");

    // Initialize wago.tools build database
    let wago_cache_dir = AgentConfig::default_data_dir().join("wago");
    let mut wago = cascette_import::WagoProvider::new(wago_cache_dir)?;
    wago.initialize().await?;
    info!("wago.tools build database initialized");

    // Open database
    let db_path = config.db_path();
    info!(path = %db_path.display(), "opening database");
    let db = Database::open(&db_path).await?;

    // Create shared state
    let config = Arc::new(config);
    let state = Arc::new(
        AppState::new(db, ribbit_client, cdn_client, config.clone(), wago)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?,
    );

    // Create cancellation token for graceful shutdown
    let cancellation = CancellationToken::new();

    // Start operation runner
    let runner = OperationRunner::new(
        state.clone(),
        config.max_concurrent_operations,
        cancellation.child_token(),
    );
    let runner_handle = tokio::spawn(async move {
        if let Err(e) = runner.run().await {
            tracing::error!(error = %e, "operation runner failed");
        }
    });

    // Spawn periodic update checker
    let update_state = state.clone();
    let update_cancel = cancellation.child_token();
    let patchfreq = config.patchfreq;
    let update_handle = tokio::spawn(async move {
        update_checker(update_state, update_cancel, patchfreq).await;
    });

    // Bind HTTP server with port fallback
    let state_ref = state.clone();
    let router = create_router(state);
    let bind_addr = config.bind_addr.clone();
    let mut bound = false;

    for port in config.port_candidates() {
        let addr = format!("{bind_addr}:{port}");
        info!(addr = %addr, "attempting to bind HTTP server");

        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                state_ref.bound_port.store(port, Ordering::Relaxed);
                info!(addr = %addr, "HTTP server listening");
                bound = true;

                let shutdown = cancellation.clone();
                tokio::spawn(async move {
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        tracing::error!(error = %e, "failed to listen for Ctrl+C");
                        return;
                    }
                    info!("received shutdown signal");
                    shutdown.cancel();
                });

                axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        cancellation.cancelled().await;
                    })
                    .await?;

                break;
            }
            Err(e) => {
                tracing::warn!(addr = %addr, error = %e, "failed to bind, trying next port");
            }
        }
    }

    if !bound {
        anyhow::bail!(
            "failed to bind to any port: tried {:?}",
            config.port_candidates()
        );
    }

    // Wait for runner to finish
    let _ = runner_handle.await;
    update_handle.abort();

    info!("cascette-agent stopped");
    Ok(())
}

/// Periodically checks for product updates by querying Ribbit.
///
/// Runs every `patchfreq` seconds. For each installed product, queries the
/// latest version and sets `is_update_available` if a newer version exists.
/// Does NOT auto-start updates -- that is the launcher's decision.
async fn update_checker(state: Arc<AppState>, cancellation: CancellationToken, patchfreq: u32) {
    let interval = std::time::Duration::from_secs(u64::from(patchfreq));

    // Wait one interval before first check
    tokio::select! {
        () = cancellation.cancelled() => return,
        () = tokio::time::sleep(interval) => {}
    }

    loop {
        tracing::debug!("running periodic update check");

        if let Ok(products) = state.registry.list().await {
            for product in &products {
                // Only check installed products
                if !product.status.is_installed() {
                    continue;
                }

                let region = product.region.as_deref().unwrap_or("us");
                let endpoint = format!("v1/products/{}/versions", product.product_code);

                let versions = match state.ribbit_client.query(&endpoint).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(
                            product = %product.product_code,
                            error = %e,
                            "update check query failed"
                        );
                        continue;
                    }
                };

                // Find the matching region row
                let version_row = versions
                    .rows()
                    .iter()
                    .find(|row| {
                        row.get_by_name("Region", versions.schema())
                            .and_then(|v| v.as_string())
                            .is_some_and(|r| r.eq_ignore_ascii_case(region))
                    })
                    .or_else(|| versions.rows().first());

                if let Some(row) = version_row {
                    let latest_version = row
                        .get_by_name("VersionsName", versions.schema())
                        .and_then(|v| v.as_string())
                        .unwrap_or("unknown");

                    let current = product.version.as_deref().unwrap_or("");

                    if !current.is_empty() && current != latest_version {
                        info!(
                            product = %product.product_code,
                            current = %current,
                            latest = %latest_version,
                            "update available"
                        );

                        if let Ok(mut p) = state.registry.get(&product.product_code).await {
                            p.is_update_available = true;
                            p.available_version = Some(latest_version.to_string());
                            let _ = state.registry.update(&p).await;
                        }
                    }
                }
            }
        }

        tokio::select! {
            () = cancellation.cancelled() => return,
            () = tokio::time::sleep(interval) => {}
        }
    }
}
