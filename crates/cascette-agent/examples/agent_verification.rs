#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Agent integration verification.
//!
//! Exercises `cascette-agent` AppState initialization and router creation.
//! Optionally runs a maintenance dry-run on a local installation.
//!
//! Environment variables:
//!   CASCETTE_WOW_PATH   Path to a local WoW installation root (optional)
//!   CASCETTE_CDN_HOSTS  Comma-separated CDN hostnames (optional,
//!                       default: casc.wago.tools,cdn.arctium.tools,archive.wow.tools)
//!
//! Usage:
//!   cargo run -p cascette-agent --example agent_verification

use cascette_agent::config::AgentConfig;
use cascette_agent::server::router::{AppState, create_router};
use cascette_agent::state::db::Database;
use cascette_protocol::{CacheConfig, CdnClient, CdnConfig, ClientConfig, RibbitTactClient};
use clap::Parser;
use std::sync::Arc;

async fn community_agent_state() -> Arc<AppState> {
    let client_config = ClientConfig {
        tact_https_url: "https://casc.wago.tools".to_string(),
        tact_http_url: String::new(),
        ribbit_url: "tcp://127.0.0.1:1".to_string(),
        cache_config: CacheConfig::memory_optimized(),
        ..Default::default()
    };
    let ribbit = Arc::new(RibbitTactClient::new(client_config).expect("ribbit client"));
    let cdn =
        Arc::new(CdnClient::new(ribbit.cache().clone(), CdnConfig::default()).expect("cdn client"));

    let db = Database::open_memory().await.expect("in-memory db");
    let agent_config = AgentConfig::parse_from::<[&str; 0], &str>([]);

    let wago_dir = tempfile::tempdir().expect("temp dir for wago cache");
    let wago =
        cascette_import::WagoProvider::new(wago_dir.path().to_path_buf()).expect("wago provider");
    let state = AppState::new(db, ribbit, cdn, Arc::new(agent_config), wago)
        .await
        .expect("app state creation");

    Arc::new(state)
}

#[tokio::main]
async fn main() {
    // F1: Initialize agent state and verify router creation
    println!("=== F1: Agent state initialization ===");
    let state = community_agent_state().await;
    let _app = create_router(Arc::clone(&state));

    assert!(
        !state.agent_version.is_empty(),
        "agent should have a version string"
    );

    println!("  version={}", state.agent_version);

    // F2: Optionally verify a local installation
    if let Ok(wow_path) = std::env::var("CASCETTE_WOW_PATH") {
        let install_path = std::path::PathBuf::from(&wow_path);
        if install_path.exists() {
            println!("\n=== F2: Agent verify local installation ===");
            let data_path = install_path.join("Data");

            let install =
                cascette_client_storage::Installation::open(data_path).expect("installation open");
            install.initialize().await.expect("initialization");

            let report = cascette_maintenance::run_maintenance(
                &install,
                cascette_maintenance::ExecutionMode::DryRun,
            )
            .await
            .expect("verification should succeed");

            println!("  completed in {:?}", report.total_duration);

            if let Some(ref repair) = report.repair {
                println!(
                    "  {} entries verified, {} corrupted",
                    repair.entries_verified, repair.entries_corrupted,
                );
            }
        } else {
            println!("\n  CASCETTE_WOW_PATH set but path does not exist: {wow_path}");
        }
    } else {
        println!("\n  CASCETTE_WOW_PATH not set, skipping local verification");
    }

    println!("\nAll agent checks passed.");
}
