//! File metadata, content categorization, and TACT key orchestration for NGDP/CASC.
//!
//! This crate provides:
//!
//! - **Bidirectional FileDataID lookup** via [`FileDataIdService`]
//! - **Content categorization** via [`ContentCategory`] and [`ContentInfo`]
//! - **Metadata orchestration** via [`MetadataOrchestrator`], which coordinates
//!   FDID resolution and TACT key access under a unified API
//!
//! # Usage
//!
//! The `import` feature (enabled by default) allows constructing services
//! directly from `cascette-import` providers. Without it, use [`FileDataIdService::from_map`]
//! and [`MetadataOrchestrator::from_raw`] with your own data.
//!
//! ## Example
//!
//! ```
//! use std::collections::HashMap;
//! use cascette_crypto::TactKeyStore;
//! use cascette_metadata::{
//!     MetadataOrchestrator, OrchestratorConfig, HealthStatus,
//! };
//!
//! let mut mappings = HashMap::new();
//! mappings.insert(100u32, "world/maps/azeroth/azeroth.wmo".to_string());
//! mappings.insert(200u32, "sound/music/zone.mp3".to_string());
//!
//! let keys = TactKeyStore::new(); // includes hardcoded WoW keys
//!
//! let orch = MetadataOrchestrator::from_raw(
//!     mappings,
//!     keys,
//!     OrchestratorConfig::default(),
//! );
//!
//! assert_eq!(orch.resolve_id(100).unwrap(), "world/maps/azeroth/azeroth.wmo");
//! assert_eq!(orch.resolve_path("sound/music/zone.mp3").unwrap(), 200);
//!
//! let stats = orch.stats();
//! assert_eq!(stats.fdid_count, 2);
//! assert!(stats.keys_ready);
//! assert_eq!(orch.health(), HealthStatus::Healthy);
//! ```

#![warn(missing_docs)]

pub mod content;
pub mod error;
pub mod fdid;
pub mod orchestrator;

pub use content::{ContentCategory, ContentInfo};
pub use error::{MetadataError, MetadataResult};
pub use fdid::FileDataIdService;
pub use orchestrator::{HealthStatus, MetadataOrchestrator, OrchestratorConfig, OrchestratorStats};
