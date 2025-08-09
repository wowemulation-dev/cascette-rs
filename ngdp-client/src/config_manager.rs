//! Configuration management for persistent storage of user settings

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("Configuration key '{key}' not found")]
    KeyNotFound { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NgdpConfig {
    /// Built-in configuration values
    #[serde(flatten)]
    pub defaults: DefaultConfig,
    /// User-defined custom configuration values
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultConfig {
    pub default_region: String,
    pub cache_dir: String,
    pub timeout: u32,
    pub cache_enabled: bool,
    pub cache_ttl: u32,
    pub max_concurrent_downloads: u32,
    pub user_agent: String,
    pub verify_certificates: bool,
    pub proxy_url: String,
    pub ribbit_timeout: u32,
    pub tact_timeout: u32,
    pub retry_attempts: u32,
    pub log_file: String,
    pub color_output: bool,
    pub fallback_to_tact: bool,
    pub use_community_cdn_fallbacks: bool,
    pub custom_cdn_fallbacks: String,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            default_region: "us".to_string(),
            cache_dir: "~/.cache/ngdp".to_string(),
            timeout: 30,
            cache_enabled: true,
            cache_ttl: 1800, // 30 minutes in seconds
            max_concurrent_downloads: 4,
            user_agent: "ngdp-client/0.1.2".to_string(),
            verify_certificates: true,
            proxy_url: String::new(),
            ribbit_timeout: 30,
            tact_timeout: 30,
            retry_attempts: 3,
            log_file: String::new(),
            color_output: true,
            fallback_to_tact: true,
            use_community_cdn_fallbacks: true,
            custom_cdn_fallbacks: String::new(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    config: NgdpConfig,
}

impl ConfigManager {
    /// Create a new config manager and load existing configuration
    pub fn new() -> Result<Self, ConfigError> {
        let config_path = Self::get_config_path()?;
        let config = Self::load_config(&config_path)?;

        Ok(Self {
            config_path,
            config,
        })
    }

    /// Get the configuration file path
    fn get_config_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cascette");

        // Ensure directory exists
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("ngdp-client.toml"))
    }

    /// Load configuration from file or create default
    fn load_config(config_path: &PathBuf) -> Result<NgdpConfig, ConfigError> {
        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            let config: NgdpConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config
            let config = NgdpConfig::default();
            Self::save_config_to_file(config_path, &config)?;
            Ok(config)
        }
    }

    /// Save configuration to file
    fn save_config_to_file(config_path: &PathBuf, config: &NgdpConfig) -> Result<(), ConfigError> {
        let toml_content = toml::to_string_pretty(config)?;
        fs::write(config_path, toml_content)?;
        Ok(())
    }

    /// Save the current configuration to file
    pub fn save(&self) -> Result<(), ConfigError> {
        Self::save_config_to_file(&self.config_path, &self.config)
    }

    /// Get a configuration value by key
    pub fn get(&self, key: &str) -> Result<String, ConfigError> {
        // First check custom values
        if let Some(value) = self.config.custom.get(key) {
            return Ok(value.clone());
        }

        // Then check built-in defaults
        let value = match key {
            "default_region" => &self.config.defaults.default_region,
            "cache_dir" => &self.config.defaults.cache_dir,
            "timeout" => return Ok(self.config.defaults.timeout.to_string()),
            "cache_enabled" => return Ok(self.config.defaults.cache_enabled.to_string()),
            "cache_ttl" => return Ok(self.config.defaults.cache_ttl.to_string()),
            "max_concurrent_downloads" => {
                return Ok(self.config.defaults.max_concurrent_downloads.to_string());
            }
            "user_agent" => &self.config.defaults.user_agent,
            "verify_certificates" => {
                return Ok(self.config.defaults.verify_certificates.to_string());
            }
            "proxy_url" => &self.config.defaults.proxy_url,
            "ribbit_timeout" => return Ok(self.config.defaults.ribbit_timeout.to_string()),
            "tact_timeout" => return Ok(self.config.defaults.tact_timeout.to_string()),
            "retry_attempts" => return Ok(self.config.defaults.retry_attempts.to_string()),
            "log_file" => &self.config.defaults.log_file,
            "color_output" => return Ok(self.config.defaults.color_output.to_string()),
            "fallback_to_tact" => return Ok(self.config.defaults.fallback_to_tact.to_string()),
            "use_community_cdn_fallbacks" => {
                return Ok(self.config.defaults.use_community_cdn_fallbacks.to_string());
            }
            "custom_cdn_fallbacks" => &self.config.defaults.custom_cdn_fallbacks,
            _ => {
                return Err(ConfigError::KeyNotFound {
                    key: key.to_string(),
                });
            }
        };

        Ok(value.clone())
    }

    /// Set a configuration value
    pub fn set(&mut self, key: String, value: String) -> Result<(), ConfigError> {
        // Always store custom values in the custom section
        self.config.custom.insert(key, value);
        self.save()?;
        Ok(())
    }

    /// Get all configuration values as a HashMap
    pub fn get_all(&self) -> HashMap<String, String> {
        let mut all_config = HashMap::new();

        // Add built-in defaults
        all_config.insert(
            "default_region".to_string(),
            self.config.defaults.default_region.clone(),
        );
        all_config.insert(
            "cache_dir".to_string(),
            self.config.defaults.cache_dir.clone(),
        );
        all_config.insert(
            "timeout".to_string(),
            self.config.defaults.timeout.to_string(),
        );
        all_config.insert(
            "cache_enabled".to_string(),
            self.config.defaults.cache_enabled.to_string(),
        );
        all_config.insert(
            "cache_ttl".to_string(),
            self.config.defaults.cache_ttl.to_string(),
        );
        all_config.insert(
            "max_concurrent_downloads".to_string(),
            self.config.defaults.max_concurrent_downloads.to_string(),
        );
        all_config.insert(
            "user_agent".to_string(),
            self.config.defaults.user_agent.clone(),
        );
        all_config.insert(
            "verify_certificates".to_string(),
            self.config.defaults.verify_certificates.to_string(),
        );
        all_config.insert(
            "proxy_url".to_string(),
            self.config.defaults.proxy_url.clone(),
        );
        all_config.insert(
            "ribbit_timeout".to_string(),
            self.config.defaults.ribbit_timeout.to_string(),
        );
        all_config.insert(
            "tact_timeout".to_string(),
            self.config.defaults.tact_timeout.to_string(),
        );
        all_config.insert(
            "retry_attempts".to_string(),
            self.config.defaults.retry_attempts.to_string(),
        );
        all_config.insert(
            "log_file".to_string(),
            self.config.defaults.log_file.clone(),
        );
        all_config.insert(
            "color_output".to_string(),
            self.config.defaults.color_output.to_string(),
        );
        all_config.insert(
            "fallback_to_tact".to_string(),
            self.config.defaults.fallback_to_tact.to_string(),
        );
        all_config.insert(
            "use_community_cdn_fallbacks".to_string(),
            self.config.defaults.use_community_cdn_fallbacks.to_string(),
        );
        all_config.insert(
            "custom_cdn_fallbacks".to_string(),
            self.config.defaults.custom_cdn_fallbacks.clone(),
        );

        // Override with custom values
        for (key, value) in &self.config.custom {
            all_config.insert(key.clone(), value.clone());
        }

        all_config
    }

    /// Reset configuration to defaults
    pub fn reset(&mut self) -> Result<(), ConfigError> {
        self.config = NgdpConfig::default();
        self.save()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_config_creation_and_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.toml");

        // Create config with custom value
        let mut config = NgdpConfig::default();
        config
            .custom
            .insert("test.key".to_string(), "test_value".to_string());

        // Save it
        ConfigManager::save_config_to_file(&config_path, &config).unwrap();

        // Load it back
        let loaded_config = ConfigManager::load_config(&config_path).unwrap();

        assert_eq!(loaded_config.custom.get("test.key").unwrap(), "test_value");
        assert_eq!(loaded_config.defaults.default_region, "us");
    }

    #[test]
    fn test_config_get_set() {
        // Use a temporary directory for this test
        let temp_dir = TempDir::new().unwrap();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let mut manager = ConfigManager::new().unwrap();

        // Test setting and getting custom value
        manager
            .set("test.product".to_string(), "wow_classic_era".to_string())
            .unwrap();
        let value = manager.get("test.product").unwrap();
        assert_eq!(value, "wow_classic_era");

        // Test getting default value
        let default_region = manager.get("default_region").unwrap();
        assert_eq!(default_region, "us");

        // Test non-existent key
        let result = manager.get("nonexistent.key");
        assert!(result.is_err());
    }
}
