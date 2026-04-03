//! Axum router configuration matching real Blizzard Agent.exe endpoints.
//!
//! The real agent exposes 22 static endpoints on port 1120. Dynamic per-product
//! endpoints are registered for install, update, repair, uninstall, backfill,
//! game, and gamesession operations.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::time::Duration;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use super::handlers;
use crate::config::AgentConfig;
use crate::observability::Metrics;
use crate::session::SessionTracker;
use crate::state::db::Database;
use crate::state::queue::OperationQueue;
use crate::state::registry::ProductRegistry;
use crate::state::size_cache::SizeEstimateCache;

/// Global download speed/pause state shared across handlers and executors.
#[derive(Default)]
pub struct DownloadState {
    /// Maximum download speed in bytes per second (0 = unlimited).
    pub max_speed_bps: u64,
    /// Whether downloads are paused.
    pub paused: bool,
    /// Current aggregate download speed in bytes per second.
    pub current_speed_bps: u64,
}

/// Per-product download configuration (keyed by product UID).
#[derive(Debug, Clone)]
pub struct ProductDownloadConfig {
    /// Enable background downloads for this product.
    pub background_download: bool,
    /// Download priority (default 700).
    pub priority: u32,
    /// Speed limit in bytes per second (0 = unlimited).
    pub download_limit: u64,
    /// Whether downloads for this product are paused.
    pub paused: bool,
}

impl Default for ProductDownloadConfig {
    fn default() -> Self {
        Self {
            background_download: false,
            priority: 700,
            download_limit: 0,
            paused: false,
        }
    }
}

/// Shared application state accessible by all handlers.
pub struct AppState {
    /// Product registry.
    pub registry: ProductRegistry,
    /// Operation queue.
    pub queue: OperationQueue,
    /// Prometheus metrics.
    pub metrics: Metrics,
    /// Agent version string.
    pub agent_version: String,
    /// Server start time.
    pub started_at: std::time::SystemTime,
    /// Ribbit/TACT protocol client for version queries.
    pub ribbit_client: Arc<cascette_protocol::RibbitTactClient>,
    /// CDN content download client (implements CdnSource).
    pub cdn_client: Arc<cascette_protocol::CdnClient>,
    /// Agent configuration from CLI/env.
    pub config: Arc<AgentConfig>,
    /// In-memory game session tracker.
    pub session_tracker: SessionTracker,
    /// In-memory size estimation cache.
    pub size_cache: SizeEstimateCache,
    /// Global download speed/pause configuration.
    pub download_state: Arc<tokio::sync::RwLock<DownloadState>>,
    /// Per-product download configuration (keyed by product UID).
    pub product_download_config: Arc<tokio::sync::RwLock<HashMap<String, ProductDownloadConfig>>>,
    /// Actual port the HTTP server is bound to.
    /// Set after successful TCP bind; read by `/agent` handler.
    pub bound_port: AtomicU16,
    /// wago.tools build database for historical version lookup.
    pub wago: Arc<tokio::sync::RwLock<cascette_import::WagoProvider>>,
    /// Wakeup signal for the operation runner.
    ///
    /// Handlers call `notify_one()` after inserting into the queue so the
    /// runner picks up work immediately instead of waiting for its poll tick.
    pub queue_notify: tokio::sync::Notify,
    /// CDN / version-service / config override state.
    /// GET /agent/override reads; POST /agent/override writes.
    pub override_config: Arc<tokio::sync::RwLock<handlers::override_config::OverrideConfig>>,
}

impl AppState {
    /// Create new application state from a database and protocol clients.
    ///
    /// Loads persisted per-product download configurations from the database
    /// into the in-memory cache so they survive agent restarts.
    ///
    /// # Errors
    ///
    /// Returns an error if metrics initialization fails.
    pub async fn new(
        db: Database,
        ribbit_client: Arc<cascette_protocol::RibbitTactClient>,
        cdn_client: Arc<cascette_protocol::CdnClient>,
        config: Arc<AgentConfig>,
        wago: cascette_import::WagoProvider,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let metrics = Metrics::new()?;
        let default_port = config.port();
        let registry = ProductRegistry::new(db.clone());

        // Hydrate per-product download configs from database.
        let configs = registry.list_download_configs().await.unwrap_or_default();
        if !configs.is_empty() {
            tracing::info!(
                count = configs.len(),
                "loaded per-product download configs from database"
            );
        }

        Ok(Self {
            registry,
            queue: OperationQueue::new(db),
            metrics,
            agent_version: format!("cascette-agent/{}", env!("CARGO_PKG_VERSION")),
            started_at: std::time::SystemTime::now(),
            ribbit_client,
            cdn_client,
            config,
            session_tracker: SessionTracker::new(),
            size_cache: SizeEstimateCache::new(),
            download_state: Arc::new(tokio::sync::RwLock::new(DownloadState::default())),
            product_download_config: Arc::new(tokio::sync::RwLock::new(configs)),
            bound_port: AtomicU16::new(default_port),
            wago: Arc::new(tokio::sync::RwLock::new(wago)),
            queue_notify: tokio::sync::Notify::new(),
            override_config: Arc::new(tokio::sync::RwLock::new(
                handlers::override_config::OverrideConfig::default(),
            )),
        })
    }
}

/// Create the full Axum router with all agent endpoints.
pub fn create_router(state: Arc<AppState>) -> Router {
    let timeout = Duration::from_secs(30);

    // CORS restricted to localhost (agent is a local service)
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(|origin, _req| {
            origin
                .to_str()
                .map(|s| s.contains("localhost") || s.contains("127.0.0.1"))
                .unwrap_or(false)
        }))
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        // Real agent endpoints
        .route(
            "/agent",
            axum::routing::get(handlers::agent::get_agent_info)
                .post(handlers::agent::post_agent_config),
        )
        .route(
            "/game",
            axum::routing::get(handlers::game::list_games).post(handlers::game::post_game_config),
        )
        .route(
            "/game/{product}",
            axum::routing::get(handlers::game::get_game),
        )
        .route(
            "/install",
            axum::routing::post(handlers::install::post_install_bare),
        )
        .route(
            "/install/{product}",
            axum::routing::post(handlers::install::post_install)
                .get(handlers::progress::get_progress),
        )
        .route(
            "/update",
            axum::routing::post(handlers::update::post_update_bare),
        )
        .route(
            "/update/{product}",
            axum::routing::post(handlers::update::post_update)
                .get(handlers::progress::get_progress),
        )
        .route(
            "/repair",
            axum::routing::post(handlers::repair::post_repair_bare),
        )
        .route(
            "/repair/{product}",
            axum::routing::post(handlers::repair::post_repair)
                .get(handlers::progress::get_progress),
        )
        .route(
            "/uninstall",
            axum::routing::post(handlers::uninstall::post_uninstall_bare),
        )
        .route(
            "/uninstall/{product}",
            axum::routing::post(handlers::uninstall::post_uninstall)
                .get(handlers::progress::get_progress),
        )
        .route(
            "/backfill",
            axum::routing::post(handlers::backfill::post_backfill_bare),
        )
        .route(
            "/backfill/{product}",
            axum::routing::post(handlers::backfill::post_backfill)
                .get(handlers::progress::get_progress),
        )
        .route(
            "/version",
            axum::routing::get(handlers::version::get_version),
        )
        .route(
            "/hardware",
            axum::routing::get(handlers::hardware::get_hardware),
        )
        .route(
            "/gamesession",
            axum::routing::get(handlers::gamesession::get_sessions),
        )
        .route(
            "/gamesession/{product}",
            axum::routing::get(handlers::gamesession::get_session)
                .post(handlers::gamesession::post_session),
        )
        .route(
            "/download",
            axum::routing::get(handlers::download::get_download)
                .post(handlers::download::post_download),
        )
        .route(
            "/option",
            axum::routing::get(handlers::option::get_option).post(handlers::option::post_option),
        )
        .route(
            "/size_estimate",
            axum::routing::post(handlers::size_estimate::post_size_estimate),
        )
        .route(
            "/size_estimate/{uid}",
            axum::routing::get(handlers::size_estimate::get_size_estimate_result),
        )
        .route(
            "/agent/download",
            axum::routing::get(handlers::agent_download::get_agent_download),
        )
        .route(
            "/agent/override",
            axum::routing::get(handlers::override_config::get_override_config)
                .post(handlers::override_config::post_override_config),
        )
        .route(
            "/agent/{product}",
            axum::routing::get(handlers::override_config::get_product_override_state)
                .post(handlers::override_config::post_product_override_state),
        )
        .route(
            "/spawned",
            axum::routing::get(handlers::spawned::get_spawned)
                .post(handlers::spawned::post_spawned),
        )
        .route(
            "/spawned/{product}",
            axum::routing::get(handlers::spawned::get_spawned_product)
                .post(handlers::spawned::post_spawned_product),
        )
        .route(
            "/gce_state",
            axum::routing::get(handlers::admin::get_gce_state)
                .post(handlers::admin::post_gce_state),
        )
        .route(
            "/createshortcut",
            axum::routing::post(handlers::admin::post_createshortcut),
        )
        .route(
            "/admin_command",
            axum::routing::post(handlers::admin::post_admin_command),
        )
        .route("/admin", axum::routing::post(handlers::admin::post_admin))
        .route(
            "/register",
            axum::routing::post(handlers::register::post_register),
        )
        .route(
            "/priorities",
            axum::routing::get(handlers::priorities::get_priorities)
                .post(handlers::priorities::post_priorities),
        )
        .route(
            "/content/{hash}",
            axum::routing::get(handlers::content::get_content),
        )
        // Cascette extensions
        .route("/health", axum::routing::get(handlers::health::get_health))
        .route(
            "/metrics",
            axum::routing::get(handlers::metrics::get_metrics),
        )
        .route(
            "/extract/{product}",
            axum::routing::post(handlers::extract::post_extract)
                .get(handlers::progress::get_progress),
        )
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            timeout,
        ))
        .with_state(state)
}
