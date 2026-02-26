//! Configuration file formats for NGDP/CASC system
//!
//! This module provides parsers and builders for Build Config and CDN Config files,
//! which are the entry points to accessing CASC content.

mod build_config;
mod cdn_config;
mod keyring_config;
mod patch_config;
mod product_config;

pub use build_config::{BuildConfig, BuildInfo, PartialPriority};
pub use cdn_config::{ArchiveInfo, CdnConfig};
pub use keyring_config::{KeyringConfig, KeyringEntry};
pub use patch_config::{PatchConfig, PatchEntry};
pub use product_config::{
    AddRemoveProgramsConfig, Config, DiscInfo, EulaConfig, FormConfig, GameDirConfig,
    InstallAction, LauncherInstallInfo, MediaConfig, PlatformConfigs, ProductConfig,
    ProductConfigError, RegionConfig, ShortcutConfig, TitleInfo,
};

/// Common functionality for config file parsing
pub(crate) fn parse_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.splitn(2, " = ");
    let key = parts.next()?.trim();
    let value = parts.next()?.trim();

    // Validate key format
    if is_valid_key(key) {
        Some((key.to_string(), value.to_string()))
    } else {
        None
    }
}

/// Validate config key format
pub(crate) fn is_valid_key(key: &str) -> bool {
    key.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Validate MD5 hash format (32 hex characters)
pub(crate) fn is_valid_md5_hex(hash: &str) -> bool {
    hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit())
}
