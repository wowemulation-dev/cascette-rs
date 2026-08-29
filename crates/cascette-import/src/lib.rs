//! Community data source integration for NGDP/CASC.
//!
//! This crate provides integration with community-maintained data sources:
//!
//! - **wago.tools**: Historical build information via `/api/builds`
//! - **WoWDev Listfile**: File ID to path mappings from `github.com/wowdev/wow-listfile`
//! - **WoWDev TACT Keys**: Encryption keys from `github.com/wowdev/TACTKeys`
//! - **BlizzTrack**: Per-region TACT build information for all Blizzard products
//!
//! Each data source is implemented as a provider behind the [`ImportProvider`] trait.
//! The [`ImportManager`] coordinates multiple providers, aggregating results and
//! managing per-provider health and caching.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cascette_import::{ImportManager, WagoProvider};
//! use std::path::PathBuf;
//!
//! # async fn example() -> cascette_import::ImportResult<()> {
//! let cache_dir = PathBuf::from("/tmp/cascette-cache/wago");
//! let wago = WagoProvider::new(cache_dir)?;
//!
//! let mut manager = ImportManager::new();
//! manager.add_provider("wago", Box::new(wago)).await?;
//!
//! let builds = manager.get_builds("wow").await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod error;
pub mod manager;
pub mod providers;

#[cfg(feature = "wago")]
pub mod wago;

#[cfg(feature = "listfile")]
pub mod listfile;

#[cfg(feature = "tactkeys")]
pub mod tactkeys;

#[cfg(feature = "blizztrack")]
pub mod blizztrack;

/// Ensure a rustls crypto provider is installed before creating TLS clients.
///
/// Uses ring as the crypto provider. If another provider was already installed
/// (e.g. by the application), this is a no-op.
#[cfg(any(
    feature = "wago",
    feature = "listfile",
    feature = "tactkeys",
    feature = "blizztrack"
))]
pub(crate) fn ensure_crypto_provider() {
    use std::sync::OnceLock;
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

// Re-export main types.
pub use error::{ImportError, ImportResult};
pub use manager::ImportManager;
pub use providers::{
    BuildSearchCriteria, CacheStats, DataSource, ImportProvider, ImportProviderInfo,
};

#[cfg(feature = "wago")]
pub use wago::WagoProvider;

#[cfg(feature = "listfile")]
pub use listfile::ListfileProvider;

#[cfg(feature = "tactkeys")]
pub use tactkeys::{TactKeysProvider, fetch_github_tactkeys};

#[cfg(feature = "blizztrack")]
pub use blizztrack::BlizzTrackProvider;

/// Common data types used across providers.
pub mod types {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// Build information from community sources.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BuildInfo {
        /// Product identifier (e.g., "wow", "wow_classic").
        pub product: String,
        /// Version string (e.g., "11.0.7.57212").
        pub version: String,
        /// Build number extracted from the version string.
        pub build: u32,
        /// Version type (e.g., "live", "beta", "ptr").
        pub version_type: String,
        /// Region if applicable.
        pub region: Option<String>,
        /// Unix timestamp seconds, if known.
        pub timestamp: Option<u64>,
        /// Raw `created_at` string from the API (e.g., wago.tools format).
        pub created_at: Option<String>,
        /// Additional metadata (build config hashes, CDN config, etc.).
        pub metadata: HashMap<String, String>,
    }

    /// File ID to path mapping.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FileMapping {
        /// File data ID.
        pub file_id: u32,
        /// Full file path.
        pub path: String,
        /// File name only.
        pub filename: String,
        /// Directory path.
        pub directory: String,
    }

    /// Import provider capabilities.
    #[derive(Debug, Clone)]
    #[allow(clippy::struct_excessive_bools)]
    pub struct ProviderCapabilities {
        /// Can provide build information.
        pub builds: bool,
        /// Can provide file mappings.
        pub file_mappings: bool,
        /// Supports real-time updates.
        pub real_time: bool,
        /// Requires authentication.
        pub requires_auth: bool,
    }
}
