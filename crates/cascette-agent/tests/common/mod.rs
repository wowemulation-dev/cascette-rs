//! Shared test helpers for cascette-agent integration tests.
//!
//! Provides `AppState` construction with in-memory database and no-network
//! protocol clients, plus product seeding and router creation utilities.

use std::sync::Arc;

use axum::Router;
use cascette_agent::config::AgentConfig;
use cascette_agent::server::router::{AppState, create_router};
use cascette_agent::state::db::Database;
use cascette_protocol::{CacheConfig, CdnConfig, ClientConfig};

/// Build a default `AgentConfig` struct literal (same pattern as config.rs unit tests).
pub fn test_config() -> AgentConfig {
    AgentConfig {
        port: None,
        db_path: None,
        locale: "enUS".to_string(),
        show: false,
        allowcommands: false,
        skipupdate: false,
        loglevel: None,
        session: None,
        patchfreq: 300,
        version_server_url: None,
        bind_addr: "127.0.0.1".to_string(),
        max_concurrent_operations: 1,
        request_timeout_secs: 30,
        cdn_hosts: None,
        cdn_path_override: None,
        #[cfg(windows)]
        install_service: false,
        #[cfg(windows)]
        remove_service: false,
        #[cfg(windows)]
        service: false,
    }
}

/// Create an `AppState` with in-memory database and dummy protocol clients.
///
/// No network connections are opened. Protocol clients will fail if `query()`
/// or `download()` is called, which is expected for handler-level tests.
pub async fn test_app_state() -> Arc<AppState> {
    test_app_state_with_config(ClientConfig {
        tact_https_url: String::new(),
        tact_http_url: String::new(),
        ribbit_url: "tcp://127.0.0.1:1".to_string(),
        cache_config: CacheConfig::memory_optimized(),
        ..Default::default()
    })
    .await
}

/// Create an `AppState` whose Ribbit TACT HTTPS client points to `mock_url`.
///
/// Used with `wiremock::MockServer` to test endpoints that query Ribbit.
pub async fn test_app_state_with_mock(mock_url: &str) -> Arc<AppState> {
    test_app_state_with_config(ClientConfig {
        tact_https_url: mock_url.to_string(),
        tact_http_url: String::new(),
        ribbit_url: "tcp://127.0.0.1:1".to_string(),
        cache_config: CacheConfig::memory_optimized(),
        ..Default::default()
    })
    .await
}

async fn test_app_state_with_config(client_config: ClientConfig) -> Arc<AppState> {
    let db = Database::open_memory().await.expect("in-memory db");
    let ribbit_client =
        Arc::new(cascette_protocol::RibbitTactClient::new(client_config).expect("ribbit client"));
    let cdn_client = Arc::new(
        cascette_protocol::CdnClient::new(ribbit_client.cache().clone(), CdnConfig::default())
            .expect("cdn client"),
    );
    let config = Arc::new(test_config());
    let wago_dir = tempfile::tempdir().expect("temp dir for wago cache");
    let wago =
        cascette_import::WagoProvider::new(wago_dir.path().to_path_buf()).expect("wago provider");
    Arc::new(
        AppState::new(db, ribbit_client, cdn_client, config, wago)
            .await
            .expect("app state"),
    )
}

/// Insert a product into the registry with the given code and status.
///
/// For `Installed` status, also populates install_path, version, and region.
pub async fn seed_product(
    state: &AppState,
    code: &str,
    status: cascette_agent::models::product::ProductStatus,
) {
    use cascette_agent::models::product::{Product, ProductStatus};

    let mut product = Product::new(code.to_string(), code.to_string());

    if status == ProductStatus::Installed {
        product.install_path = Some("/tmp/test_install".to_string());
        product.version = Some("1.0.0.10000".to_string());
        product.region = Some("us".to_string());
        product.locale = Some("enUS".to_string());
    }

    state
        .registry
        .insert(&product)
        .await
        .expect("insert product");

    // If status is not Available (the default), transition to it.
    if status != ProductStatus::Available {
        let mut p = state.registry.get(code).await.expect("get product");
        p.status = status;
        state.registry.update(&p).await.expect("update product");
    }
}

/// Create the full router backed by the given state.
pub fn test_router(state: Arc<AppState>) -> Router {
    create_router(state)
}
