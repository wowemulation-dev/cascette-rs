//! CASC installation pipeline: install, extract, verify, and repair operations.
//!
//! This crate provides the core install/update/repair pipeline for populating
//! Battle.net-compatible CASC installations from CDN sources. It handles
//! manifest resolution, CDN archive downloading, and local directory layout
//! generation.
//!
//! # Operations
//!
//! - **Install** -- Populate `Data/data/`, `Data/indices/`, `Data/config/`,
//!   `.build.info` from CDN. Matches agent.exe behavior.
//! - **Extract** -- Write game files from CASC storage to product
//!   subdirectories (e.g., `_classic_era_/`).
//! - **Verify** -- Check installation integrity (existence, size, or full
//!   MD5 + BLTE verification).
//! - **Repair** -- Verify then re-download failures.
//!
//! # Example
//!
//! ```ignore
//! use cascette_installation::config::InstallConfig;
//! use cascette_installation::InstallPipeline;
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! // cdn: Arc<impl CdnSource>, endpoints: Vec<CdnEndpoint>
//! let config = InstallConfig::new(
//!     "wow_classic_era".to_string(),
//!     PathBuf::from("/opt/wow"),
//!     "tpr/wow".to_string(),
//! );
//! let pipeline = InstallPipeline::new(config);
//! let report = pipeline.run(cdn, endpoints, |event| {
//!     let _ = event;
//! }).await?;
//! ```

pub mod cdn_source;
pub mod checkpoint;
pub mod config;
pub mod endpoint_scorer;
pub mod error;
pub mod extract;
pub mod layout;
pub mod mirror;
/// Patch application pipeline (BsDiff, block-level diff, re-encode)
pub mod patch;
pub mod pipeline;
pub mod progress;
pub mod repair;
pub mod verify;

// Re-exports
pub use cdn_source::CdnSource;
pub use config::{
    ExtractConfig, InstallConfig, RepairConfig, UpdateConfig, VerifyConfig, VerifyMode,
};
pub use error::{InstallationError, InstallationResult};
pub use pipeline::install::InstallPipeline;
pub use pipeline::manifests::BuildManifests;
pub use pipeline::update::UpdatePipeline;
pub use progress::ProgressEvent;

pub use extract::ExtractPipeline;
pub use repair::RepairPipeline;
pub use verify::VerifyPipeline;
