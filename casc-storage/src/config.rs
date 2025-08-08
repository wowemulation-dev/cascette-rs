//! Configuration discovery and parsing for WoW installations
//!
//! This module discovers and parses NGDP configuration files stored in
//! WoW installations under the `Data/config/` directory.

use crate::error::{CascError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tact_parser::config::{BuildConfig, CdnConfig, ConfigFile};
use tracing::{debug, trace};

/// Discovered configuration files for a WoW installation
#[derive(Debug, Clone)]
pub struct WowConfigSet {
    /// All discovered CDN configs
    pub cdn_configs: Vec<CdnConfig>,

    /// All discovered build configs
    pub build_configs: Vec<BuildConfig>,

    /// Directory where configs were found
    pub config_dir: PathBuf,
}

impl WowConfigSet {
    /// Get the most recent CDN config (if any)
    pub fn latest_cdn_config(&self) -> Option<&CdnConfig> {
        self.cdn_configs.first()
    }

    /// Get the most recent build config (if any)
    pub fn latest_build_config(&self) -> Option<&BuildConfig> {
        self.build_configs.first()
    }

    /// Get all archive hashes from CDN configs
    pub fn all_archive_hashes(&self) -> Vec<String> {
        let mut hashes = Vec::new();
        for cdn_config in &self.cdn_configs {
            hashes.extend(cdn_config.archives().iter().map(|s| s.to_string()));
        }
        hashes.sort();
        hashes.dedup();
        hashes
    }

    /// Get file index hashes
    pub fn file_index_hashes(&self) -> Vec<String> {
        let mut hashes = Vec::new();
        for cdn_config in &self.cdn_configs {
            if let Some(file_index) = cdn_config.file_index() {
                hashes.push(file_index.to_string());
            }
        }
        hashes.sort();
        hashes.dedup();
        hashes
    }
}

/// Configuration discovery for WoW installations
pub struct ConfigDiscovery;

impl ConfigDiscovery {
    /// Discover all configuration files in a WoW installation
    pub fn discover_configs<P: AsRef<Path>>(wow_path: P) -> Result<WowConfigSet> {
        let wow_path = wow_path.as_ref();

        // Check for Data/config directory
        let config_dir = Self::find_config_directory(wow_path)?;
        debug!("Found config directory: {:?}", config_dir);

        let mut cdn_configs = Vec::new();
        let mut build_configs = Vec::new();

        // Scan all config files
        let config_files = Self::scan_config_files(&config_dir)?;
        debug!("Found {} config files", config_files.len());

        for config_path in config_files {
            match Self::parse_config_file(&config_path)? {
                ConfigType::Cdn(cdn_config) => {
                    trace!("Found CDN config: {:?}", config_path.file_name());
                    cdn_configs.push(cdn_config);
                }
                ConfigType::Build(build_config) => {
                    trace!("Found build config: {:?}", config_path.file_name());
                    build_configs.push(build_config);
                }
                ConfigType::Unknown => {
                    trace!("Unknown config type: {:?}", config_path.file_name());
                }
            }
        }

        debug!(
            "Discovered {} CDN configs, {} build configs",
            cdn_configs.len(),
            build_configs.len()
        );

        Ok(WowConfigSet {
            cdn_configs,
            build_configs,
            config_dir,
        })
    }

    /// Find the config directory in a WoW installation
    fn find_config_directory<P: AsRef<Path>>(wow_path: P) -> Result<PathBuf> {
        let wow_path = wow_path.as_ref();

        // Try Data/config first
        let data_config = wow_path.join("Data").join("config");
        if data_config.exists() && data_config.is_dir() {
            return Ok(data_config);
        }

        // Try just config if wow_path ends with Data
        let config_dir = wow_path.join("config");
        if config_dir.exists() && config_dir.is_dir() {
            return Ok(config_dir);
        }

        Err(CascError::InvalidIndexFormat(format!(
            "No config directory found in WoW installation: {wow_path:?}"
        )))
    }

    /// Scan for all config files in the config directory
    fn scan_config_files(config_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut config_files = Vec::new();

        // Config files are stored in hash-based subdirectories like ab/cd/abcd1234...
        for entry in fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check subdirectories for config files
                if let Ok(subentries) = fs::read_dir(&path) {
                    for subentry in subentries {
                        let subentry = subentry?;
                        let subpath = subentry.path();

                        if subpath.is_dir() {
                            // Check files in the hash subdirectory
                            if let Ok(files) = fs::read_dir(&subpath) {
                                for file in files {
                                    let file = file?;
                                    let file_path = file.path();

                                    if file_path.is_file() {
                                        config_files.push(file_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        trace!("Scanned config files: {:?}", config_files);
        Ok(config_files)
    }

    /// Parse a config file and determine its type
    fn parse_config_file(path: &Path) -> Result<ConfigType> {
        let content = fs::read_to_string(path).map_err(CascError::Io)?;

        // Skip empty files
        if content.trim().is_empty() {
            return Ok(ConfigType::Unknown);
        }

        // Parse as generic config first
        let config = ConfigFile::parse(&content)
            .map_err(|e| CascError::InvalidIndexFormat(format!("Config parse error: {e}")))?;

        // Determine type based on keys present
        if Self::is_cdn_config(&config) {
            let cdn_config = CdnConfig::parse(&content).map_err(|e| {
                CascError::InvalidIndexFormat(format!("CDN config parse error: {e}"))
            })?;
            Ok(ConfigType::Cdn(cdn_config))
        } else if Self::is_build_config(&config) {
            let build_config = BuildConfig::parse(&content).map_err(|e| {
                CascError::InvalidIndexFormat(format!("Build config parse error: {e}"))
            })?;
            Ok(ConfigType::Build(build_config))
        } else {
            Ok(ConfigType::Unknown)
        }
    }

    /// Check if a config file is a CDN config based on its keys
    fn is_cdn_config(config: &ConfigFile) -> bool {
        // CDN configs typically have archives, file-index, etc.
        config.has_key("archives")
            || config.has_key("archive-group")
            || config.has_key("file-index")
    }

    /// Check if a config file is a build config based on its keys
    fn is_build_config(config: &ConfigFile) -> bool {
        // Build configs typically have root, encoding, install, etc.
        config.has_key("root")
            || config.has_key("encoding")
            || config.has_key("install")
            || config.has_key("build-name")
    }
}

/// Type of configuration file
#[derive(Debug)]
enum ConfigType {
    /// CDN configuration
    Cdn(CdnConfig),
    /// Build configuration
    Build(BuildConfig),
    /// Unknown or unsupported type
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config_structure() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("Data").join("config");

        // Create the directory structure: config/ab/cd/abcd1234...
        let hash_dir = config_dir.join("ab").join("cd");
        fs::create_dir_all(&hash_dir).unwrap();

        // Create a CDN config file
        let cdn_config_content = r#"# CDN Configuration
archives = abc123 def456 789abc
archive-group = group123
file-index = index456
"#;
        fs::write(hash_dir.join("abcd1234567890abcdef"), cdn_config_content).unwrap();

        // Create another directory for build config
        let build_hash_dir = config_dir.join("12").join("34");
        fs::create_dir_all(&build_hash_dir).unwrap();

        // Create a build config file
        let build_config_content = r#"# Build Configuration
root = abc123def456 100
encoding = 789abcdef012 200
install = fedcba987654 300
build-name = 1.13.2.31650
"#;
        fs::write(
            build_hash_dir.join("1234567890abcdef1234"),
            build_config_content,
        )
        .unwrap();

        temp_dir
    }

    #[test]
    fn test_discover_configs() {
        let temp_dir = create_test_config_structure();
        let config_set = ConfigDiscovery::discover_configs(temp_dir.path()).unwrap();

        assert_eq!(config_set.cdn_configs.len(), 1);
        assert_eq!(config_set.build_configs.len(), 1);

        let cdn_config = config_set.latest_cdn_config().unwrap();
        let archives = cdn_config.archives();
        assert_eq!(archives.len(), 3);
        assert_eq!(archives[0], "abc123");

        let build_config = config_set.latest_build_config().unwrap();
        assert_eq!(build_config.build_name(), Some("1.13.2.31650"));
    }

    #[test]
    fn test_config_type_detection() {
        let cdn_content = "archives = abc def\nfile-index = 123\n";
        let cdn_config = ConfigFile::parse(cdn_content).unwrap();
        assert!(ConfigDiscovery::is_cdn_config(&cdn_config));
        assert!(!ConfigDiscovery::is_build_config(&cdn_config));

        let build_content = "root = abc123 100\nencoding = def456 200\n";
        let build_config = ConfigFile::parse(build_content).unwrap();
        assert!(!ConfigDiscovery::is_cdn_config(&build_config));
        assert!(ConfigDiscovery::is_build_config(&build_config));
    }
}
