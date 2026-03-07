//! Configuration types for installation operations.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use cascette_crypto::TactKeyProvider;
use cascette_protocol::CdnEndpoint;

/// Structured tag query for manifest filtering and `.build.info` generation.
///
/// Blizzard Agent groups tags by type (Platform, Architecture, Locale, etc.)
/// and applies OR-within-group, AND-between-groups logic via
/// `TagTable::ApplyTagQuery`. This struct captures the full set of tag
/// components so that both the manifest filter query and the `.build.info`
/// Tags column can be generated correctly.
///
/// The `.build.info` Tags format uses `?` as a group delimiter within a
/// configuration and `:` to separate alternative configurations (typically
/// speech vs text variants):
///
/// ```text
/// Windows x86_64 EU? enUS speech?:Windows x86_64 EU? enUS text?
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagQuery {
    /// Operating system (e.g., "Windows", "OSX").
    pub platform: String,
    /// CPU architecture (e.g., "x86_64", "x86_32", "arm64").
    pub architecture: String,
    /// Locale code (e.g., "enUS", "deDE").
    pub locale: String,
    /// Region code, uppercase in tag queries (e.g., "US", "EU").
    pub region: String,
    /// Install speech audio content.
    pub has_speech: bool,
    /// Install text/UI content.
    pub has_text: bool,
    /// ISO 3166-1 alpha-3 account country (e.g., "BGR", "DEU").
    /// Produces `acct-{code}` tag in `.build.info` Tags column.
    /// Injected by the launcher via `POST /agent/override/productstate`.
    pub account_country: Option<String>,
    /// ISO 3166-1 alpha-2 GeoIP country (e.g., "BG", "DE").
    /// Produces `geoip-{code}` tag in `.build.info` Tags column.
    /// Injected by the launcher via `POST /agent/override/productstate`.
    pub geo_ip_country: Option<String>,

    /// Additional tags to include in the manifest filter query.
    ///
    /// Used by the launcher installer to add the `"launcher"` tag, which
    /// selects only launcher-tagged entries from the bootstrapper's install
    /// manifest.
    pub extra_tags: Vec<String>,
}

impl Default for TagQuery {
    fn default() -> Self {
        Self {
            platform: "Windows".to_string(),
            architecture: "x86_64".to_string(),
            locale: "enUS".to_string(),
            region: "US".to_string(),
            has_speech: true,
            has_text: true,
            account_country: None,
            geo_ip_country: None,
            extra_tags: Vec::new(),
        }
    }
}

impl TagQuery {
    /// Create a tag query with the given parameters.
    #[must_use]
    pub fn new(
        platform: impl Into<String>,
        architecture: impl Into<String>,
        locale: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            platform: platform.into(),
            architecture: architecture.into(),
            locale: locale.into(),
            region: region.into(),
            has_speech: true,
            has_text: true,
            account_country: None,
            geo_ip_country: None,
            extra_tags: Vec::new(),
        }
    }

    /// Produce the flat list of tag names for `get_files_for_tag_query`.
    ///
    /// Includes platform, architecture, locale, and content type tags.
    /// Region is excluded because install/download manifests don't use
    /// region tags in their bitmask filtering -- region filtering happens
    /// at the Ribbit level when selecting which version row to use.
    #[must_use]
    pub fn tag_names(&self) -> Vec<String> {
        let mut tags = vec![
            self.platform.clone(),
            self.architecture.clone(),
            self.locale.clone(),
        ];

        if self.has_speech {
            tags.push("speech".to_string());
        }
        if self.has_text {
            tags.push("text".to_string());
        }

        for tag in &self.extra_tags {
            tags.push(tag.clone());
        }

        tags
    }

    /// Format the `.build.info` Tags column value.
    ///
    /// Generates the multi-configuration format matching Blizzard Agent output.
    /// Each configuration is:
    /// `{platform} {arch} {region}? [acct-{X}?] [geoip-{X}?] {locale} {content}?`
    /// Multiple configurations are joined with `:`.
    ///
    /// The `acct-` and `geoip-` segments are included only when
    /// `account_country` / `geo_ip_country` are set. These values come
    /// from the launcher via the override endpoint and do not affect
    /// manifest file selection.
    #[must_use]
    pub fn build_info_tags(&self) -> String {
        use std::fmt::Write;

        let region_upper = self.region.to_uppercase();

        // Build the common prefix: platform, arch, region, optional country tags.
        let mut prefix = format!("{} {} {region_upper}?", self.platform, self.architecture);
        if let Some(ref acct) = self.account_country {
            let _ = write!(prefix, " acct-{acct}?");
        }
        if let Some(ref geo) = self.geo_ip_country {
            let _ = write!(prefix, " geoip-{geo}?");
        }

        let mut configs = Vec::new();

        if self.has_speech {
            configs.push(format!("{prefix} {} speech?", self.locale));
        }
        if self.has_text {
            configs.push(format!("{prefix} {} text?", self.locale));
        }

        // Fallback: if neither speech nor text, produce a basic tag string.
        if configs.is_empty() {
            return format!("{prefix} {}?", self.locale);
        }

        configs.join(":")
    }

    /// Convert a region code from Ribbit format (lowercase "us", "eu")
    /// to tag format (uppercase "US", "EU").
    #[must_use]
    pub fn region_from_ribbit(region: &str) -> String {
        region.to_uppercase()
    }

    /// Fill missing `account_country` and `geo_ip_country` from OS timezone.
    ///
    /// Uses [`crate::geolocation::detect_geo_defaults`] to derive country codes
    /// from the system's IANA timezone. Only applies when both country fields
    /// are `None` — launcher overrides always take precedence.
    pub fn apply_geo_defaults(&mut self) {
        if self.account_country.is_some() || self.geo_ip_country.is_some() {
            return;
        }
        if let Some(geo) = crate::geolocation::detect_geo_defaults() {
            self.account_country = Some(geo.alpha3);
            self.geo_ip_country = Some(geo.alpha2);
        }
    }
}

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

    /// Region code as received from Ribbit (lowercase, e.g., "us", "eu").
    pub region: String,

    /// Structured tag query for manifest filtering.
    pub tag_query: TagQuery,

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
    /// promoting unfinished files to highest priority.
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
            .field("tag_query", &self.tag_query)
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
            tag_query: TagQuery::default(),
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

    /// Structured tag query for manifest filtering.
    pub tag_query: TagQuery,

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
            tag_query: TagQuery::default(),
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

    /// Region code as received from Ribbit (lowercase, e.g., "us", "eu").
    pub region: String,

    /// Structured tag query for manifest filtering.
    pub tag_query: TagQuery,

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
            .field("tag_query", &self.tag_query)
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
            tag_query: TagQuery::default(),
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
