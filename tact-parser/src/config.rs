//! Configuration file parser for TACT
//!
//! Parses build and CDN configuration files which use a simple key-value format.
//! Some values contain hash-size pairs for referencing other TACT files.

use std::collections::HashMap;
use tracing::{debug, trace};

use crate::Result;

/// A hash-size pair found in config values
#[derive(Debug, Clone, PartialEq)]
pub struct HashPair {
    /// The hash (usually MD5 hex string)
    pub hash: String,
    /// The size in bytes
    pub size: u64,
}

/// Configuration file (build or CDN)
#[derive(Debug, Clone)]
pub struct ConfigFile {
    /// All key-value pairs
    pub values: HashMap<String, String>,
    /// Hash-size pairs extracted from values
    pub hashes: HashMap<String, HashPair>,
}

impl ConfigFile {
    /// Parse a configuration file from text
    pub fn parse(text: &str) -> Result<Self> {
        let mut values = HashMap::new();
        let mut hashes = HashMap::new();
        
        for line in text.lines() {
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            
            // Split on " = " (space equals space) or " =" (space equals, for empty values)
            let (key, value) = if let Some(eq_pos) = line.find(" = ") {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 3..].trim(); // Skip " = " and trim
                (key, value)
            } else if let Some(eq_pos) = line.find(" =") {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 2..].trim(); // Skip " =" and trim
                (key, value)
            } else {
                continue; // No valid key = value format found
            };
            
            trace!("Config entry: '{}' = '{}'", key, value);
            
            // Always insert the value, even if empty
            values.insert(key.to_string(), value.to_string());
            
            // Check if value contains hash and size
            if !value.is_empty() {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 2 && is_hex_hash(parts[0]) {
                    if let Ok(size) = parts[1].parse::<u64>() {
                        hashes.insert(key.to_string(), HashPair {
                            hash: parts[0].to_string(),
                            size,
                        });
                    }
                }
            }
        }
        
        debug!("Parsed config with {} entries, {} hash pairs", values.len(), hashes.len());
        
        Ok(ConfigFile { values, hashes })
    }
    
    /// Get a value by key
    pub fn get_value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }
    
    /// Get a hash by key (extracts from hash-size pairs)
    pub fn get_hash(&self, key: &str) -> Option<&str> {
        self.hashes.get(key).map(|hp| hp.hash.as_str())
    }
    
    /// Get a size by key (extracts from hash-size pairs)
    pub fn get_size(&self, key: &str) -> Option<u64> {
        self.hashes.get(key).map(|hp| hp.size)
    }
    
    /// Get a hash pair by key
    pub fn get_hash_pair(&self, key: &str) -> Option<&HashPair> {
        self.hashes.get(key)
    }
    
    /// Check if a key exists
    pub fn has_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
    
    /// Get all keys
    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|s| s.as_str()).collect()
    }
}

/// Common configuration keys for build configs
pub mod build_keys {
    /// Root file hash and size
    pub const ROOT: &str = "root";
    /// Install manifest hash and size
    pub const INSTALL: &str = "install";
    /// Download manifest hash and size
    pub const DOWNLOAD: &str = "download";
    /// Encoding file hash and size
    pub const ENCODING: &str = "encoding";
    /// Size file hash and size
    pub const SIZE: &str = "size";
    /// Patch file hash and size
    pub const PATCH: &str = "patch";
    /// Patch config hash and size
    pub const PATCH_CONFIG: &str = "patch-config";
    /// Build name
    pub const BUILD_NAME: &str = "build-name";
    /// Build UID
    pub const BUILD_UID: &str = "build-uid";
    /// Build product
    pub const BUILD_PRODUCT: &str = "build-product";
    /// Encoding sizes
    pub const ENCODING_SIZE: &str = "encoding-size";
    /// Install sizes
    pub const INSTALL_SIZE: &str = "install-size";
    /// Download sizes
    pub const DOWNLOAD_SIZE: &str = "download-size";
    /// Size sizes
    pub const SIZE_SIZE: &str = "size-size";
    /// VFS root
    pub const VFS_ROOT: &str = "vfs-root";
}

/// Common configuration keys for CDN configs
pub mod cdn_keys {
    /// Archive group
    pub const ARCHIVE_GROUP: &str = "archive-group";
    /// Archives list
    pub const ARCHIVES: &str = "archives";
    /// Patch archives
    pub const PATCH_ARCHIVES: &str = "patch-archives";
    /// File index
    pub const FILE_INDEX: &str = "file-index";
    /// Patch file index
    pub const PATCH_FILE_INDEX: &str = "patch-file-index";
}

/// Check if a string looks like a hex hash
fn is_hex_hash(s: &str) -> bool {
    s.len() >= 6 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Underlying config file
    pub config: ConfigFile,
}

impl BuildConfig {
    /// Parse a build configuration
    pub fn parse(text: &str) -> Result<Self> {
        let config = ConfigFile::parse(text)?;
        Ok(BuildConfig { config })
    }
    
    /// Get the root file hash
    pub fn root_hash(&self) -> Option<&str> {
        self.config.get_hash(build_keys::ROOT)
    }
    
    /// Get the encoding file hash
    pub fn encoding_hash(&self) -> Option<&str> {
        self.config.get_hash(build_keys::ENCODING)
    }
    
    /// Get the install manifest hash
    pub fn install_hash(&self) -> Option<&str> {
        self.config.get_hash(build_keys::INSTALL)
    }
    
    /// Get the download manifest hash
    pub fn download_hash(&self) -> Option<&str> {
        self.config.get_hash(build_keys::DOWNLOAD)
    }
    
    /// Get the size file hash
    pub fn size_hash(&self) -> Option<&str> {
        self.config.get_hash(build_keys::SIZE)
    }
    
    /// Get the build name
    pub fn build_name(&self) -> Option<&str> {
        self.config.get_value(build_keys::BUILD_NAME)
    }
}

/// CDN configuration
#[derive(Debug, Clone)]
pub struct CdnConfig {
    /// Underlying config file
    pub config: ConfigFile,
}

impl CdnConfig {
    /// Parse a CDN configuration
    pub fn parse(text: &str) -> Result<Self> {
        let config = ConfigFile::parse(text)?;
        Ok(CdnConfig { config })
    }
    
    /// Get the archives list
    pub fn archives(&self) -> Vec<&str> {
        self.config
            .get_value(cdn_keys::ARCHIVES)
            .map(|v| v.split_whitespace().collect())
            .unwrap_or_default()
    }
    
    /// Get the archive group
    pub fn archive_group(&self) -> Option<&str> {
        self.config.get_value(cdn_keys::ARCHIVE_GROUP)
    }
    
    /// Get the file index
    pub fn file_index(&self) -> Option<&str> {
        self.config.get_value(cdn_keys::FILE_INDEX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_config() {
        let config_text = r#"
# This is a comment
key1 = value1
key2 = value2

# Empty lines are ignored
key3 = value with spaces
        "#;
        
        let config = ConfigFile::parse(config_text).unwrap();
        assert_eq!(config.get_value("key1"), Some("value1"));
        assert_eq!(config.get_value("key2"), Some("value2"));
        assert_eq!(config.get_value("key3"), Some("value with spaces"));
        assert_eq!(config.get_value("nonexistent"), None);
    }
    
    #[test]
    fn test_parse_hash_pairs() {
        let config_text = r#"
encoding = abc123def456789 123456
root = 0123456789abcdef 789
install = fedcba9876543210 456789
invalid = not_a_hash 123
        "#;
        
        let config = ConfigFile::parse(config_text).unwrap();
        
        // Check hash extraction
        assert_eq!(config.get_hash("encoding"), Some("abc123def456789"));
        assert_eq!(config.get_size("encoding"), Some(123456));
        
        assert_eq!(config.get_hash("root"), Some("0123456789abcdef"));
        assert_eq!(config.get_size("root"), Some(789));
        
        assert_eq!(config.get_hash("install"), Some("fedcba9876543210"));
        assert_eq!(config.get_size("install"), Some(456789));
        
        // Invalid hash should not be extracted
        assert_eq!(config.get_hash("invalid"), None);
        
        // But the raw value should still be there
        assert_eq!(config.get_value("invalid"), Some("not_a_hash 123"));
    }
    
    #[test]
    fn test_build_config() {
        let config_text = r#"
root = abc123 100
encoding = def456 200
install = 789abc 300
download = cdef01 400
size = 234567 500
build-name = 10.0.0.12345
build-uid = wow/game
        "#;
        
        let build = BuildConfig::parse(config_text).unwrap();
        
        assert_eq!(build.root_hash(), Some("abc123"));
        assert_eq!(build.encoding_hash(), Some("def456"));
        assert_eq!(build.install_hash(), Some("789abc"));
        assert_eq!(build.download_hash(), Some("cdef01"));
        assert_eq!(build.size_hash(), Some("234567"));
        assert_eq!(build.build_name(), Some("10.0.0.12345"));
    }
    
    #[test]
    fn test_cdn_config() {
        let config_text = r#"
archives = archive1 archive2 archive3
archive-group = abc123def456
file-index = 789abcdef012
patch-archives = patch1 patch2
        "#;
        
        let cdn = CdnConfig::parse(config_text).unwrap();
        
        let archives = cdn.archives();
        assert_eq!(archives.len(), 3);
        assert_eq!(archives[0], "archive1");
        assert_eq!(archives[1], "archive2");
        assert_eq!(archives[2], "archive3");
        
        assert_eq!(cdn.archive_group(), Some("abc123def456"));
        assert_eq!(cdn.file_index(), Some("789abcdef012"));
    }
    
    #[test]
    fn test_is_hex_hash() {
        assert!(is_hex_hash("abc123def456"));
        assert!(is_hex_hash("0123456789ABCDEF"));
        assert!(!is_hex_hash("not_hex"));
        assert!(!is_hex_hash("abc12g")); // 'g' is not hex
        assert!(!is_hex_hash("abc")); // Too short
    }
    
}