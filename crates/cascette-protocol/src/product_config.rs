//! Product configuration JSON fetching and parsing.
//!
//! Blizzard Agent fetches per-product JSON configuration from CDN using the
//! `ConfigPath` from the CDNs BPSV response. This path (`tpr/configs/data`)
//! is separate from the standard TACT `Path` (`tpr/wow`) used for build
//! configs, data, and patches.
//!
//! The product config contains `launcher_install_info` which describes how
//! to resolve the bootstrapper (setup/provisioner binary) for a game product.
//! The `bts` product provides setup binaries (e.g., `World-of-Warcraft-Setup.exe`),
//! not game launchers. The `product_tag` field (e.g., `"wow"`) is a product
//! family identifier used to filter `bts/versions`.
//!
//! URL format: `{scheme}://{host}/{config_path}/{hash[0:2]}/{hash[2:4]}/{hash}`
//!
//! The hash comes from the `ProductConfig` column in the product's versions
//! BPSV response.

use serde::Deserialize;
use tracing::{debug, warn};

use crate::cdn::CdnEndpoint;
use crate::error::{ProtocolError, Result};
use crate::transport::HttpClient;

/// Default `bootstrapper_branch` value for the launcher binary operation.
///
/// In `bts/versions`, the Region column `"launcher"` identifies the shared
/// launcher binary build (e.g., build 3140_launcher_merged). This branch is
/// used when the product config JSON's `launcher_install_info.bootstrapper_branch`
/// is not available (older builds without a ProductConfig column).
///
/// The Blizzard Agent uses this value as the default branch when the
/// product config does not specify `bootstrapper_branch`.
pub const DEFAULT_BOOTSTRAPPER_BRANCH: &str = "launcher";

/// Top-level product configuration JSON.
///
/// The JSON has a nested structure with `all` containing defaults and
/// `platform` containing per-platform overrides. We only parse `all.config`
/// since that's where `launcher_install_info` lives.
#[derive(Debug, Clone, Deserialize)]
pub struct ProductConfig {
    /// Global configuration (applies to all platforms).
    pub all: Option<ProductConfigSection>,
    /// Per-platform overrides (win, mac, etc.).
    pub platform: Option<serde_json::Value>,
}

/// A section within the product config (e.g., `all` or a platform).
#[derive(Debug, Clone, Deserialize)]
pub struct ProductConfigSection {
    /// The config block containing launcher info and other settings.
    pub config: Option<ProductConfigFields>,
}

/// Fields within a product config section.
#[derive(Debug, Clone, Deserialize)]
pub struct ProductConfigFields {
    /// Product code (e.g., "wow_classic_era").
    pub product: Option<String>,

    /// Launcher install info describing the bootstrapper product.
    pub launcher_install_info: Option<LauncherInstallInfo>,

    /// Default subfolder for the shared container (e.g., "_classic_era_").
    pub shared_container_default_subfolder: Option<String>,

    /// Update method (typically "ngdp").
    pub update_method: Option<String>,
}

/// Launcher install info from the product config JSON.
///
/// Despite the name, this describes the **bootstrapper** (setup/provisioner
/// binary), not the game launcher. The `product_tag` is a product family
/// identifier for filtering `bts/versions`.
#[derive(Debug, Clone, Deserialize)]
pub struct LauncherInstallInfo {
    /// The bootstrapper product code (typically "bts").
    pub bootstrapper_product: String,

    /// The branch to use (typically "launcher").
    pub bootstrapper_branch: String,

    /// Migration version string for launcher binary migration.
    pub bootstrapper_migration_version: Option<String>,

    /// Product tag for filtering `bts/versions`.
    ///
    /// The `bts/versions` BPSV uses the Region column to store product codes
    /// (not geographic regions). This tag is used to find the correct row.
    /// For example, both `wow_classic` and `wow_classic_era` use `"wow"`.
    pub product_tag: String,
}

/// Build a product config URL from a config path and hash.
///
/// Uses the `ConfigPath` from CDNs BPSV (e.g., `tpr/configs/data`) and the
/// `ProductConfig` hash from versions BPSV. The URL format does NOT include
/// a content type segment — `ConfigPath` is its own namespace.
fn build_product_config_url(host: &str, scheme: &str, config_path: &str, hash: &str) -> String {
    let config_path = config_path.trim_end_matches('/');
    format!(
        "{scheme}://{host}/{config_path}/{}/{}/{}",
        &hash[..2],
        &hash[2..4],
        hash,
    )
}

/// Fetch a product config JSON from CDN.
///
/// Tries each endpoint in order until one succeeds. The `config_path` is
/// the `ConfigPath` value from the CDNs BPSV (e.g., `tpr/configs/data`).
/// The `hash` is the `ProductConfig` value from the versions BPSV.
///
/// # Errors
///
/// Returns `ProtocolError::AllHostsFailed` if no endpoint serves the config.
pub async fn fetch_product_config(
    http_client: &HttpClient,
    endpoints: &[CdnEndpoint],
    config_path: &str,
    hash: &str,
) -> Result<ProductConfig> {
    if hash.len() < 4 {
        return Err(ProtocolError::Parse(format!(
            "product config hash too short: {hash}"
        )));
    }

    let mut last_error = None;

    for endpoint in endpoints {
        let scheme = endpoint.scheme.as_deref().unwrap_or("https");
        let url = build_product_config_url(&endpoint.host, scheme, config_path, hash);

        debug!(url = %url, "fetching product config");

        match http_client.inner().get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let bytes = response.bytes().await.map_err(ProtocolError::Http)?;

                let config: ProductConfig = serde_json::from_slice(&bytes).map_err(|e| {
                    ProtocolError::Parse(format!("failed to parse product config JSON: {e}"))
                })?;

                debug!(hash = %hash, "product config fetched");
                return Ok(config);
            }
            Ok(response) => {
                let status = response.status();
                warn!(url = %url, status = %status, "product config fetch failed");
                last_error = Some(ProtocolError::HttpStatus(status));
            }
            Err(e) => {
                warn!(url = %url, error = %e, "product config fetch failed");
                last_error = Some(ProtocolError::Http(e));
            }
        }
    }

    Err(last_error.unwrap_or(ProtocolError::AllHostsFailed))
}

impl ProductConfig {
    /// Get the launcher install info from the global config section.
    #[must_use]
    pub fn launcher_install_info(&self) -> Option<&LauncherInstallInfo> {
        self.all
            .as_ref()?
            .config
            .as_ref()?
            .launcher_install_info
            .as_ref()
    }

    /// Get the shared container default subfolder.
    #[must_use]
    pub fn shared_container_default_subfolder(&self) -> Option<&str> {
        self.all
            .as_ref()?
            .config
            .as_ref()?
            .shared_container_default_subfolder
            .as_deref()
    }
}

/// Known product tag mappings for builds without a `ProductConfig` column.
///
/// Builds before approximately version 1.14.0 don't have a ProductConfig
/// column in the versions BPSV. This table provides the fallback mapping
/// from game product code to `bts/versions` product tag.
///
/// These mappings are derived from live CDN product config JSON files.
pub fn fallback_product_tag(product_code: &str) -> Option<&'static str> {
    match product_code {
        "wowt" => Some("wowt"),
        "wowb" => Some("wowb"),
        "wow" | "wow_classic" | "wow_classic_era" | "wow_classic_titan" | "wow_anniversary" => {
            Some("wow")
        }
        "d3" => Some("d3"),
        "ow" => Some("ow"),
        "hero" => Some("hero"),
        "s1" => Some("s1"),
        "s2" => Some("s2"),
        "w3" => Some("w3"),
        "bna" => Some("bna"),
        "launcher" => Some("launcher"),
        _ => None,
    }
}

/// Extract the `ConfigPath` value from a CDNs BPSV row.
///
/// Returns `None` if the field is missing (older BPSV responses may not
/// include `ConfigPath`).
pub fn config_path_from_bpsv_row(
    row: &cascette_formats::bpsv::BpsvRow,
    schema: &cascette_formats::bpsv::BpsvSchema,
) -> Option<String> {
    row.get_raw_by_name("ConfigPath", schema)
        .map(ToString::to_string)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_product_config_url() {
        let url = build_product_config_url(
            "level3.blizzard.com",
            "https",
            "tpr/configs/data",
            "c9934edfc8f217a2e01c47e4deae8454",
        );
        assert_eq!(
            url,
            "https://level3.blizzard.com/tpr/configs/data/c9/93/c9934edfc8f217a2e01c47e4deae8454"
        );
    }

    #[test]
    fn test_build_product_config_url_trailing_slash() {
        let url = build_product_config_url(
            "level3.blizzard.com",
            "https",
            "tpr/configs/data/",
            "c9934edfc8f217a2e01c47e4deae8454",
        );
        assert_eq!(
            url,
            "https://level3.blizzard.com/tpr/configs/data/c9/93/c9934edfc8f217a2e01c47e4deae8454"
        );
    }

    #[test]
    fn test_parse_product_config_json() {
        let json = r#"{
            "all": {
                "config": {
                    "product": "wow_classic_era",
                    "launcher_install_info": {
                        "bootstrapper_product": "bts",
                        "bootstrapper_branch": "launcher",
                        "product_tag": "wow"
                    },
                    "shared_container_default_subfolder": "_classic_era_",
                    "update_method": "ngdp"
                }
            },
            "platform": {
                "win": {
                    "config": {
                        "tags": ["Windows"],
                        "tags_64bit": ["x86_64"]
                    }
                }
            }
        }"#;

        let config: ProductConfig = serde_json::from_str(json).unwrap();
        let info = config.launcher_install_info().unwrap();
        assert_eq!(info.bootstrapper_product, "bts");
        assert_eq!(info.bootstrapper_branch, "launcher");
        assert_eq!(info.product_tag, "wow");
        assert_eq!(
            config.shared_container_default_subfolder(),
            Some("_classic_era_")
        );
    }

    #[test]
    fn test_parse_product_config_missing_launcher_info() {
        let json = r#"{
            "all": {
                "config": {
                    "product": "some_product",
                    "update_method": "ngdp"
                }
            }
        }"#;

        let config: ProductConfig = serde_json::from_str(json).unwrap();
        assert!(config.launcher_install_info().is_none());
    }

    #[test]
    fn test_fallback_product_tag() {
        assert_eq!(fallback_product_tag("wow"), Some("wow"));
        assert_eq!(fallback_product_tag("wow_classic"), Some("wow"));
        assert_eq!(fallback_product_tag("wow_classic_era"), Some("wow"));
        assert_eq!(fallback_product_tag("d3"), Some("d3"));
        assert_eq!(fallback_product_tag("unknown_product"), None);
    }

    #[test]
    fn test_default_bootstrapper_branch_matches_product_config() {
        // The constant must match the bootstrapper_branch value in live product
        // config JSON, which maps to a Region value in bts/versions.
        assert_eq!(DEFAULT_BOOTSTRAPPER_BRANCH, "launcher");
    }
}
