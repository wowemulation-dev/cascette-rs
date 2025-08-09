//! TACT manifest integration for CASC storage
//!
//! This module provides the integration between TACT manifests (root, encoding)
//! and CASC storage to enable FileDataID-based lookups.

mod tact_integration;

pub use tact_integration::{FileMapping, ManifestConfig, TactManifests};
