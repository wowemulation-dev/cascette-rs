//! Configuration types for installation operations.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use cascette_crypto::TactKeyProvider;
use cascette_protocol::CdnEndpoint;

/// Configuration for the install pipeline.
///
/// Specifies what product to install, where to install it, and
/// how to connect to CDN servers.
#[derive(Clone)]
pub struct InstallConfig {
    /// Product code (e.g., "wow_classic_era").
    pub product: String,

    /// Target installation directory. The pipeline creates `Data/` subdirectories here.
    pub install_path: PathBuf,

    /// CDN endpoints to use, in priority order.
    pub endpoints: Vec<CdnEndpoint>,

    /// CDN path prefix (e.g., "tpr/wow").
    pub cdn_path: String,

    /// Region code (e.g., "us", "eu").
    pub region: String,

    /// Platform tags for install manifest filtering (e.g., "Windows", "x86_64").
    pub platform_tags: Vec<String>,

    /// Locale tag (e.g., "enUS").
    pub locale: String,

    /// Build config hash (hex). If None, resolved via Ribbit.
    pub build_config: Option<String>,

    /// CDN config hash (hex). If None, resolved via Ribbit.
    pub cdn_config: Option<String>,

    /// Maximum concurrent downloads per host.
    pub max_connections_per_host: usize,

    /// Maximum global concurrent downloads.
    pub max_connections_global: usize,

    /// Archive index batch size for parallel downloads.
    pub index_batch_size: usize,

    /// Number of files between checkpoint saves.
    pub checkpoint_interval: usize,

    /// Whether to resume from a previous checkpoint.
    pub resume: bool,

    /// Whether this is a backfill operation.
    ///
    /// When `true`, remaining download manifest entries are classified at the
    /// highest priority regardless of their manifest-assigned priority value,
    /// matching Agent.exe's backfill behavior of promoting unfinished files.
    pub backfill_mode: bool,

    /// Product subfolder for loose files (e.g., "_classic_").
    /// When set, install manifest entries are extracted to
    /// `install_path/game_subfolder/` as each file completes download.
    pub game_subfolder: Option<String>,

    /// Optional encryption key provider for BLTE decryption.
    pub key_store: Option<Arc<dyn TactKeyProvider + Send + Sync>>,
}

impl std::fmt::Debug for InstallConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallConfig")
            .field("product", &self.product)
            .field("install_path", &self.install_path)
            .field("cdn_path", &self.cdn_path)
            .field("region", &self.region)
            .field("build_config", &self.build_config)
            .field("cdn_config", &self.cdn_config)
            .field("key_store", &self.key_store.as_ref().map(|_| "..."))
            .field("game_subfolder", &self.game_subfolder)
            .finish_non_exhaustive()
    }
}

impl InstallConfig {
    /// Create a new install config with required fields and defaults.
    #[must_use]
    pub fn new(product: String, install_path: PathBuf, cdn_path: String) -> Self {
        Self {
            product,
            install_path,
            endpoints: Vec::new(),
            cdn_path,
            region: "us".to_string(),
            platform_tags: vec!["Windows".to_string(), "x86_64".to_string()],
            locale: "enUS".to_string(),
            build_config: None,
            cdn_config: None,
            max_connections_per_host: 3,
            max_connections_global: 12,
            index_batch_size: 20,
            checkpoint_interval: 100,
            resume: true,
            backfill_mode: false,
            game_subfolder: None,
            key_store: None,
        }
    }
}

/// Configuration for the extract pipeline.
#[derive(Debug, Clone)]
pub struct ExtractConfig {
    /// Path to the CASC installation (containing `Data/`).
    pub install_path: PathBuf,

    /// Target directory for extracted files.
    pub output_path: PathBuf,

    /// Platform tags for install manifest filtering.
    pub platform_tags: Vec<String>,

    /// Locale tag.
    pub locale: String,

    /// Optional file pattern filter (supports `*` wildcards).
    pub pattern: Option<String>,

    /// Maximum concurrent extract operations.
    pub max_concurrent: usize,
}

impl ExtractConfig {
    /// Create a new extract config with required fields and defaults.
    #[must_use]
    pub fn new(install_path: PathBuf, output_path: PathBuf) -> Self {
        Self {
            install_path,
            output_path,
            platform_tags: vec!["Windows".to_string(), "x86_64".to_string()],
            locale: "enUS".to_string(),
            pattern: None,
            max_concurrent: 8,
        }
    }
}

/// Configuration for the verify pipeline.
#[derive(Debug, Clone)]
pub struct VerifyConfig {
    /// Path to the CASC installation (containing `Data/`).
    pub install_path: PathBuf,

    /// Verification mode.
    pub mode: VerifyMode,
}

/// Verification depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyMode {
    /// Check file existence only.
    Existence,
    /// Check existence and file size.
    Size,
    /// Full MD5 + BLTE integrity check.
    Full,
}

impl VerifyConfig {
    /// Create a new verify config.
    #[must_use]
    pub fn new(install_path: PathBuf) -> Self {
        Self {
            install_path,
            mode: VerifyMode::Existence,
        }
    }
}

/// Configuration for the update pipeline.
///
/// Specifies the base (current) and target (new) builds for transitioning
/// an existing CASC installation between versions.
#[derive(Clone)]
pub struct UpdateConfig {
    /// Product code (e.g., "wow_classic_era").
    pub product: String,

    /// Path to the existing CASC installation.
    pub install_path: PathBuf,

    /// CDN path prefix (e.g., "tpr/wow").
    pub cdn_path: String,

    /// Region code (e.g., "us", "eu").
    pub region: String,

    /// Platform tags for install manifest filtering.
    pub platform_tags: Vec<String>,

    /// Locale tag (e.g., "enUS").
    pub locale: String,

    /// Build config hash (hex) of the version being updated FROM.
    pub base_build_config: String,

    /// CDN config hash (hex) of the version being updated FROM.
    pub base_cdn_config: String,

    /// Build config hash (hex) of the version being updated TO.
    pub target_build_config: String,

    /// CDN config hash (hex) of the version being updated TO.
    pub target_cdn_config: String,

    /// Path to an alternate CASC installation for leeching files.
    pub alternate_install_path: Option<PathBuf>,

    /// Whether to apply patches (ZBSDIFF1) when patch chains exist.
    pub enable_patching: bool,

    /// Background download mode (BGDL).
    pub bgdl: bool,

    /// CDN endpoints to use, in priority order.
    pub endpoints: Vec<CdnEndpoint>,

    /// Maximum concurrent downloads per host.
    pub max_connections_per_host: usize,

    /// Maximum global concurrent downloads.
    pub max_connections_global: usize,

    /// Archive index batch size for parallel downloads.
    pub index_batch_size: usize,

    /// Number of files between checkpoint saves.
    pub checkpoint_interval: usize,

    /// Whether to resume from a previous checkpoint.
    pub resume: bool,

    /// Game subfolder for loose file placement (e.g., "_classic_era_").
    /// When set, install manifest entries are extracted to
    /// `install_path/game_subfolder/` as each file completes download.
    pub game_subfolder: Option<String>,

    /// Optional encryption key provider for BLTE decryption.
    pub key_store: Option<Arc<dyn TactKeyProvider + Send + Sync>>,
}

impl std::fmt::Debug for UpdateConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateConfig")
            .field("product", &self.product)
            .field("install_path", &self.install_path)
            .field("cdn_path", &self.cdn_path)
            .field("region", &self.region)
            .field("base_build_config", &self.base_build_config)
            .field("target_build_config", &self.target_build_config)
            .field("game_subfolder", &self.game_subfolder)
            .field("key_store", &self.key_store.as_ref().map(|_| "..."))
            .finish_non_exhaustive()
    }
}

impl UpdateConfig {
    /// Create a new update config with required fields and defaults.
    #[must_use]
    pub fn new(
        product: String,
        install_path: PathBuf,
        cdn_path: String,
        base_build_config: String,
        base_cdn_config: String,
        target_build_config: String,
        target_cdn_config: String,
    ) -> Self {
        Self {
            product,
            install_path,
            cdn_path,
            region: "us".to_string(),
            platform_tags: vec!["Windows".to_string(), "x86_64".to_string()],
            locale: "enUS".to_string(),
            base_build_config,
            base_cdn_config,
            target_build_config,
            target_cdn_config,
            alternate_install_path: None,
            enable_patching: true,
            bgdl: false,
            endpoints: Vec::new(),
            max_connections_per_host: 3,
            max_connections_global: 12,
            index_batch_size: 20,
            checkpoint_interval: 100,
            resume: true,
            game_subfolder: None,
            key_store: None,
        }
    }
}

/// Configuration for the repair pipeline.
#[derive(Debug, Clone)]
pub struct RepairConfig {
    /// Path to the CASC installation.
    pub install_path: PathBuf,

    /// CDN endpoints for re-downloading.
    pub endpoints: Vec<CdnEndpoint>,

    /// CDN path prefix.
    pub cdn_path: String,

    /// Verification mode to detect failures.
    pub verify_mode: VerifyMode,

    /// Maximum concurrent re-downloads.
    pub max_connections_global: usize,
}

impl RepairConfig {
    /// Create a new repair config.
    #[must_use]
    pub fn new(install_path: PathBuf, cdn_path: String) -> Self {
        Self {
            install_path,
            endpoints: Vec::new(),
            cdn_path,
            verify_mode: VerifyMode::Full,
            max_connections_global: 12,
        }
    }
}
