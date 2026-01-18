//! Product Config file format implementation
//!
//! Product Config files contain region-based product information in JSON format.
//! They define localized configuration for `WoW` products across different regions,
//! platforms, and locales.

use std::collections::HashMap;
use std::io::Read;

use serde::{Deserialize, Serialize};

/// Product Configuration containing region and locale-specific information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductConfig {
    /// Global configuration section (applies to all regions)
    pub all: RegionConfig,
    /// China-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cn: Option<RegionConfig>,
    /// Platform-specific configurations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<PlatformConfigs>,
    /// German locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dede: Option<RegionConfig>,
    /// US English locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enus: Option<RegionConfig>,
    /// Spain Spanish locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eses: Option<RegionConfig>,
    /// Mexico Spanish locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub esmx: Option<RegionConfig>,
    /// French locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frfr: Option<RegionConfig>,
    /// Italian locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itit: Option<RegionConfig>,
    /// Korean locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kokr: Option<RegionConfig>,
    /// Brazil Portuguese locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ptbr: Option<RegionConfig>,
    /// Russian locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruru: Option<RegionConfig>,
    /// Chinese (CN) locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zhcn: Option<RegionConfig>,
    /// Chinese (TW) locale configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zhtw: Option<RegionConfig>,
}

/// Region-specific configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionConfig {
    /// Core configuration options
    pub config: Config,
}

/// Core product configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Data directory path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// Locales displayed to the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_locales: Option<Vec<String>>,
    /// Whether block copy patching is enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_block_copy_patch: Option<bool>,
    /// Form configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<FormConfig>,
    /// Launch arguments for the game
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_arguments: Option<Vec<String>>,
    /// Launcher installation information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launcher_install_info: Option<LauncherInstallInfo>,
    /// Migration configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_leeching_products: Option<Vec<String>>,
    /// Required build for migration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_required_build: Option<u32>,
    /// Required version for migration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_required_version: Option<String>,
    /// Product-specific opaque configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opaque_product_specific: Option<HashMap<String, String>>,
    /// Complex opaque configuration data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opaque_complex_data: Option<serde_json::Value>,
    /// Product name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Locale replacement mappings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_locales: Option<HashMap<String, String>>,
    /// Default shared container subfolder
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_container_default_subfolder: Option<String>,
    /// Supported locales
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_locales: Option<Vec<String>>,
    /// Whether multibox is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_multibox: Option<bool>,
    /// Whether offline mode is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_offline: Option<bool>,
    /// Title information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_info: Option<TitleInfo>,
    /// Update method (typically "ngdp")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_method: Option<String>,
    /// Install actions for this locale/region
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install: Option<Vec<InstallAction>>,
    /// Extra tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_tags: Option<Vec<String>>,
    /// Install media configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_media: Option<HashMap<String, MediaConfig>>,
}

/// Form configuration options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormConfig {
    /// EULA configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eula: Option<EulaConfig>,
    /// Game directory configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_dir: Option<GameDirConfig>,
}

/// EULA configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EulaConfig {
    /// Whether EULA acceptance is required
    pub eula: bool,
}

/// Game directory configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDirConfig {
    /// Directory name for the game
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirname: Option<String>,
    /// Default installation location (platform-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Required disk space in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_space: Option<u64>,
    /// Additional space per extra language in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_per_extra_language: Option<u64>,
}

/// Launcher installation information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherInstallInfo {
    /// Bootstrapper branch
    pub bootstrapper_branch: String,
    /// Bootstrapper product
    pub bootstrapper_product: String,
    /// Product tag
    pub product_tag: String,
}

/// Title information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleInfo {
    /// Title ID
    pub title_id: String,
}

/// Install action (shortcuts, registry keys, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InstallAction {
    /// Start menu shortcut
    StartMenuShortcut {
        /// Shortcut configuration
        start_menu_shortcut: ShortcutConfig,
    },
    /// Desktop shortcut
    DesktopShortcut {
        /// Shortcut configuration
        desktop_shortcut: ShortcutConfig,
    },
    /// Add/Remove Programs registry entry
    AddRemovePrograms {
        /// Registry configuration
        add_remove_programs_key: AddRemoveProgramsConfig,
    },
}

/// Shortcut configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutConfig {
    /// Arguments to pass to the executable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Description text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Link path
    pub link: String,
    /// Target executable path
    pub target: String,
    /// Working directory
    pub working_dir: String,
}

/// Add/Remove Programs configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddRemoveProgramsConfig {
    /// Display name in Add/Remove Programs
    pub display_name: String,
    /// Icon path
    pub icon_path: String,
    /// Install path
    pub install_path: String,
    /// Locale
    pub locale: String,
    /// Root identifier
    pub root: String,
    /// Unique identifier
    pub uid: String,
    /// Uninstall path
    pub uninstall_path: String,
}

/// Platform-specific configurations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformConfigs {
    /// macOS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<RegionConfig>,
    /// Windows configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub win: Option<RegionConfig>,
}

/// Media configuration for install discs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaConfig {
    /// Disc information
    pub discs: Vec<DiscInfo>,
}

/// Disc information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscInfo {
    /// Disc number
    pub disc_number: u32,
    /// Display name
    pub display_name: String,
    /// macOS volume label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub osx_volume_label: Option<String>,
    /// Windows volume label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_volume_label: Option<String>,
}

impl ProductConfig {
    /// Parse `ProductConfig` from a reader containing JSON data
    pub fn parse<R: Read>(mut reader: R) -> Result<Self, ProductConfigError> {
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(ProductConfigError::IoError)?;

        serde_json::from_str(&content).map_err(ProductConfigError::JsonError)
    }

    /// Build the config file content as JSON bytes
    pub fn build(&self) -> Vec<u8> {
        // Use pretty printing for readability
        serde_json::to_vec_pretty(self).unwrap_or_else(|_| Vec::new())
    }

    /// Build the config file content as a compact JSON string
    pub fn build_compact(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| Vec::new())
    }

    /// Get the product name from the global configuration
    pub fn product_name(&self) -> Option<&str> {
        self.all.config.product.as_deref()
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> Option<&str> {
        self.all.config.data_dir.as_deref()
    }

    /// Get supported locales
    pub fn supported_locales(&self) -> Option<&[String]> {
        self.all.config.supported_locales.as_deref()
    }

    /// Get display locales
    pub fn display_locales(&self) -> Option<&[String]> {
        self.all.config.display_locales.as_deref()
    }

    /// Check if block copy patching is enabled
    pub fn enable_block_copy_patch(&self) -> bool {
        self.all.config.enable_block_copy_patch.unwrap_or(false)
    }

    /// Check if multibox is supported
    pub fn supports_multibox(&self) -> bool {
        self.all.config.supports_multibox.unwrap_or(false)
    }

    /// Check if offline mode is supported
    pub fn supports_offline(&self) -> bool {
        self.all.config.supports_offline.unwrap_or(false)
    }

    /// Get the shared container default subfolder
    pub fn shared_container_default_subfolder(&self) -> Option<&str> {
        self.all
            .config
            .shared_container_default_subfolder
            .as_deref()
    }

    /// Get launch arguments
    pub fn launch_arguments(&self) -> Option<&[String]> {
        self.all.config.launch_arguments.as_deref()
    }

    /// Get launcher install info
    pub fn launcher_install_info(&self) -> Option<&LauncherInstallInfo> {
        self.all.config.launcher_install_info.as_ref()
    }

    /// Get game directory name
    pub fn game_dir_name(&self) -> Option<&str> {
        self.all
            .config
            .form
            .as_ref()?
            .game_dir
            .as_ref()?
            .dirname
            .as_deref()
    }

    /// Get configuration for a specific locale
    pub fn get_locale_config(&self, locale: &str) -> Option<&RegionConfig> {
        match locale.to_lowercase().as_str() {
            "dede" => self.dede.as_ref(),
            "enus" => self.enus.as_ref(),
            "eses" => self.eses.as_ref(),
            "esmx" => self.esmx.as_ref(),
            "frfr" => self.frfr.as_ref(),
            "itit" => self.itit.as_ref(),
            "kokr" => self.kokr.as_ref(),
            "ptbr" => self.ptbr.as_ref(),
            "ruru" => self.ruru.as_ref(),
            "zhcn" => self.zhcn.as_ref(),
            "zhtw" => self.zhtw.as_ref(),
            _ => None,
        }
    }

    /// Get platform configuration
    pub fn get_platform_config(&self, platform: &str) -> Option<&RegionConfig> {
        let platforms = self.platform.as_ref()?;
        match platform.to_lowercase().as_str() {
            "mac" | "macos" | "osx" => platforms.mac.as_ref(),
            "win" | "windows" => platforms.win.as_ref(),
            _ => None,
        }
    }

    /// Get all available locale codes
    pub fn available_locales(&self) -> Vec<&'static str> {
        let mut locales = Vec::new();
        if self.dede.is_some() {
            locales.push("dede");
        }
        if self.enus.is_some() {
            locales.push("enus");
        }
        if self.eses.is_some() {
            locales.push("eses");
        }
        if self.esmx.is_some() {
            locales.push("esmx");
        }
        if self.frfr.is_some() {
            locales.push("frfr");
        }
        if self.itit.is_some() {
            locales.push("itit");
        }
        if self.kokr.is_some() {
            locales.push("kokr");
        }
        if self.ptbr.is_some() {
            locales.push("ptbr");
        }
        if self.ruru.is_some() {
            locales.push("ruru");
        }
        if self.zhcn.is_some() {
            locales.push("zhcn");
        }
        if self.zhtw.is_some() {
            locales.push("zhtw");
        }
        locales
    }

    /// Validate the configuration structure
    pub fn validate(&self) -> Result<(), ProductConfigError> {
        // Basic validation - ensure we have the minimal required structure
        if self.all.config.product.is_none() {
            return Err(ProductConfigError::MissingRequiredField(
                "product".to_string(),
            ));
        }

        // Validate launcher install info if present
        if let Some(ref launcher_info) = self.all.config.launcher_install_info {
            if launcher_info.bootstrapper_branch.is_empty() {
                return Err(ProductConfigError::InvalidField(
                    "bootstrapper_branch cannot be empty".to_string(),
                ));
            }
            if launcher_info.bootstrapper_product.is_empty() {
                return Err(ProductConfigError::InvalidField(
                    "bootstrapper_product cannot be empty".to_string(),
                ));
            }
            if launcher_info.product_tag.is_empty() {
                return Err(ProductConfigError::InvalidField(
                    "product_tag cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Product config parsing and validation errors
#[derive(Debug, thiserror::Error)]
pub enum ProductConfigError {
    /// I/O operation failed
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    /// JSON parsing failed
    #[error("JSON parsing error: {0}")]
    JsonError(serde_json::Error),
    /// Required configuration field is missing
    #[error("missing required field: {0}")]
    MissingRequiredField(String),
    /// Configuration field has invalid value
    #[error("invalid field: {0}")]
    InvalidField(String),
}

impl crate::CascFormat for ProductConfig {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(self.build())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "product": "wow_classic"
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");
        assert_eq!(config.product_name(), Some("wow_classic"));
    }

    #[test]
    fn test_parse_full_config() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "data_dir": "Data/",
                    "display_locales": ["enUS", "deDE", "frFR"],
                    "enable_block_copy_patch": true,
                    "form": {
                        "game_dir": {
                            "dirname": "World of Warcraft"
                        }
                    },
                    "launch_arguments": ["-launch"],
                    "launcher_install_info": {
                        "bootstrapper_branch": "launcher",
                        "bootstrapper_product": "bts",
                        "product_tag": "wow"
                    },
                    "product": "wow_classic",
                    "shared_container_default_subfolder": "_classic_",
                    "supported_locales": ["enUS", "deDE", "frFR"],
                    "supports_multibox": true,
                    "supports_offline": false,
                    "title_info": {
                        "title_id": "wow"
                    },
                    "update_method": "ngdp"
                }
            },
            "enus": {
                "config": {
                    "install": [
                        {
                            "start_menu_shortcut": {
                                "args": "--productcode=wow_classic",
                                "link": "%commonstartmenu%World of Warcraft/World of Warcraft.lnk",
                                "target": "%shortcutpath%",
                                "working_dir": "%installpath%"
                            }
                        }
                    ]
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");

        // Test global config
        assert_eq!(config.product_name(), Some("wow_classic"));
        assert_eq!(config.data_dir(), Some("Data/"));
        assert_eq!(config.game_dir_name(), Some("World of Warcraft"));
        assert_eq!(
            config.shared_container_default_subfolder(),
            Some("_classic_")
        );
        assert!(config.enable_block_copy_patch());
        assert!(config.supports_multibox());
        assert!(!config.supports_offline());

        // Test locale config
        let enus_config = config
            .get_locale_config("enus")
            .expect("Missing enus config");
        assert!(enus_config.config.install.is_some());

        // Test launcher install info
        let launcher_info = config
            .launcher_install_info()
            .expect("Missing launcher info");
        assert_eq!(launcher_info.bootstrapper_branch, "launcher");
        assert_eq!(launcher_info.bootstrapper_product, "bts");
        assert_eq!(launcher_info.product_tag, "wow");
    }

    #[test]
    fn test_round_trip() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "product": "wow_classic",
                    "data_dir": "Data/",
                    "supported_locales": ["enUS", "deDE"]
                }
            }
        }
        "#;

        let original = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse");
        let rebuilt = original.build();
        let reparsed = ProductConfig::parse(&rebuilt[..]).expect("Failed to reparse");

        assert_eq!(original.product_name(), reparsed.product_name());
        assert_eq!(original.data_dir(), reparsed.data_dir());
    }

    #[test]
    fn test_validation() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "product": "wow_classic",
                    "launcher_install_info": {
                        "bootstrapper_branch": "launcher",
                        "bootstrapper_product": "bts",
                        "product_tag": "wow"
                    }
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");
        config.validate().expect("Config should be valid");
    }

    #[test]
    fn test_validation_missing_product() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "data_dir": "Data/"
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProductConfigError::MissingRequiredField(_)
        ));
    }

    #[test]
    fn test_locale_operations() {
        let json_data = r#"
        {
            "all": {
                "config": {
                    "product": "wow_classic"
                }
            },
            "enus": {
                "config": {
                    "extra_tags": ["test"]
                }
            },
            "dede": {
                "config": {
                    "extra_tags": ["test_de"]
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");

        // Test available locales
        let locales = config.available_locales();
        assert!(locales.contains(&"enus"));
        assert!(locales.contains(&"dede"));
        assert!(!locales.contains(&"frfr"));

        // Test getting specific locale config
        assert!(config.get_locale_config("enus").is_some());
        assert!(config.get_locale_config("dede").is_some());
        assert!(config.get_locale_config("frfr").is_none());
    }

    #[test]
    fn test_real_world_sample() {
        // This is a sample from actual WoW product config
        let json_data = r#"
        {
            "all": {
                "config": {
                    "data_dir": "Data/",
                    "display_locales": ["enUS", "esMX", "ptBR", "deDE", "esES", "frFR", "ruRU", "koKR", "zhTW", "zhCN"],
                    "form": {
                        "game_dir": {
                            "dirname": "World of Warcraft"
                        }
                    },
                    "launch_arguments": ["-launch"],
                    "launcher_install_info": {
                        "bootstrapper_branch": "launcher",
                        "bootstrapper_product": "bts",
                        "product_tag": "wow"
                    },
                    "opaque_product_specific": {
                        "uses_web_credentials": "true"
                    },
                    "product": "wow_classic",
                    "shared_container_default_subfolder": "_classic_",
                    "supported_locales": ["enUS", "esMX", "ptBR", "deDE", "esES", "frFR", "ruRU", "koKR", "zhTW", "zhCN"]
                }
            }
        }
        "#;

        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");
        config.validate().expect("Config should be valid");

        assert_eq!(config.product_name(), Some("wow_classic"));
        assert_eq!(config.data_dir(), Some("Data/"));
        assert_eq!(config.game_dir_name(), Some("World of Warcraft"));
        assert_eq!(
            config.launch_arguments(),
            Some(vec!["-launch".to_string()].as_slice())
        );

        let supported_locales = config
            .supported_locales()
            .expect("Should have supported locales");
        assert_eq!(supported_locales.len(), 10);
        assert!(supported_locales.contains(&"enUS".to_string()));
        assert!(supported_locales.contains(&"zhCN".to_string()));
    }

    #[test]
    fn test_build_methods() {
        let json_data = r#"{"all":{"config":{"product":"wow_classic"}}}"#;
        let config = ProductConfig::parse(json_data.as_bytes()).expect("Failed to parse config");

        let compact = config.build_compact();
        let pretty = config.build();

        // Compact should be shorter than pretty
        assert!(compact.len() < pretty.len());

        // Both should be valid JSON and parse to the same config
        let from_compact = ProductConfig::parse(&compact[..]).expect("Failed to parse compact");
        let from_pretty = ProductConfig::parse(&pretty[..]).expect("Failed to parse pretty");

        assert_eq!(from_compact.product_name(), from_pretty.product_name());
    }

    #[test]
    fn test_invalid_json() {
        let invalid_json = r#"{"all": {"config": }}"#;
        let result = ProductConfig::parse(invalid_json.as_bytes());
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProductConfigError::JsonError(_)
        ));
    }
}
