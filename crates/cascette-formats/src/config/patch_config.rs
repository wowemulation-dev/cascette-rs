//! Patch Config file format implementation
//!
//! Patch Config files contain patch information including patch hashes, sizes,
//! and entry mappings for encoded content.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

use super::is_valid_md5_hex;

/// Patch Configuration containing patch metadata and entry mappings
#[derive(Debug, Clone)]
pub struct PatchConfig {
    /// Basic patch information (patch hash, size, etc.)
    properties: HashMap<String, String>,
    /// Patch entries mapping types to content information
    entries: Vec<PatchEntry>,
}

/// Information about a patch entry
#[derive(Debug, Clone)]
pub struct PatchEntry {
    /// Type of entry (e.g., "encoding")
    pub entry_type: String,
    /// Content key hash
    pub content_key: String,
    /// Content size in bytes
    pub content_size: u64,
    /// Encoding key hash
    pub encoding_key: String,
    /// Encoded size in bytes
    pub encoded_size: u64,
}

impl PatchConfig {
    /// Create a new empty `PatchConfig`
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// Parse `PatchConfig` from a reader
    pub fn parse<R: Read>(reader: R) -> Result<Self, PatchConfigError> {
        let reader = BufReader::new(reader);
        let mut properties = HashMap::new();
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(PatchConfigError::IoError)?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse patch-entry lines separately
            if line.starts_with("patch-entry") {
                let entry = Self::parse_patch_entry(line)?;
                entries.push(entry);
                continue;
            }

            // Parse regular key-value pairs
            if let Some((key, value)) = Self::parse_key_value(line) {
                properties.insert(key, value);
            }
        }

        Ok(Self {
            properties,
            entries,
        })
    }

    /// Parse a key-value line
    fn parse_key_value(line: &str) -> Option<(String, String)> {
        let mut parts = line.splitn(2, " = ");
        let key = parts.next()?.trim();
        let value = parts.next()?.trim();

        // Validate key format (alphanumeric plus hyphens)
        if key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            Some((key.to_string(), value.to_string()))
        } else {
            None
        }
    }

    /// Parse a patch-entry line
    fn parse_patch_entry(line: &str) -> Result<PatchEntry, PatchConfigError> {
        // Format: patch-entry = <type> <content_key> <content_size> <encoding_key> <encoded_size>
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 6 || parts.len() > 7 {
            return Err(PatchConfigError::InvalidPatchEntry(format!(
                "Expected 6-7 parts, found {}: {}",
                parts.len(),
                line
            )));
        }

        // Skip "patch-entry" and "="
        if parts[0] != "patch-entry" || parts[1] != "=" {
            return Err(PatchConfigError::InvalidPatchEntry(format!(
                "Invalid patch-entry format: {line}"
            )));
        }

        let entry_type = parts[2].to_string();
        let content_key = parts[3].to_string();
        let content_size = parts[4]
            .parse::<u64>()
            .map_err(|_| PatchConfigError::InvalidSize(parts[4].to_string()))?;
        let encoding_key = parts[5].to_string();

        // Handle encoded size - either provided as 7th part or defaults to content size
        let encoded_size = if parts.len() == 7 {
            parts[6]
                .parse::<u64>()
                .map_err(|_| PatchConfigError::InvalidSize(parts[6].to_string()))?
        } else {
            content_size // Default to content size if not provided
        };

        // Validate hash formats
        if !is_valid_md5_hex(&content_key) {
            return Err(PatchConfigError::InvalidHash(content_key));
        }
        if !is_valid_md5_hex(&encoding_key) {
            return Err(PatchConfigError::InvalidHash(encoding_key));
        }

        Ok(PatchEntry {
            entry_type,
            content_key,
            content_size,
            encoding_key,
            encoded_size,
        })
    }

    /// Build the config file content
    pub fn build(&self) -> Vec<u8> {
        let mut output = Vec::new();

        // Write header comment
        let _ = writeln!(output, "# Patch Configuration");

        // Write properties in sorted order for consistency
        let property_order = ["patch", "patch-size"];

        // Write ordered properties first
        for key in &property_order {
            if let Some(value) = self.properties.get(*key) {
                let _ = writeln!(output, "{key} = {value}");
            }
        }

        // Write remaining properties
        let mut remaining: Vec<_> = self
            .properties
            .keys()
            .filter(|k| !property_order.contains(&k.as_str()))
            .collect();
        remaining.sort();

        for key in remaining {
            let value = &self.properties[key];
            let _ = writeln!(output, "{key} = {value}");
        }

        // Write patch entries
        for entry in &self.entries {
            let _ = writeln!(
                output,
                "patch-entry = {} {} {} {} {}",
                entry.entry_type,
                entry.content_key,
                entry.content_size,
                entry.encoding_key,
                entry.encoded_size
            );
        }

        output
    }

    /// Get the main patch hash
    pub fn patch_hash(&self) -> Option<&str> {
        self.properties
            .get("patch")
            .map(std::string::String::as_str)
    }

    /// Get the patch size
    pub fn patch_size(&self) -> Option<u64> {
        self.properties
            .get("patch-size")
            .and_then(|s| s.parse().ok())
    }

    /// Get all patch entries
    pub fn entries(&self) -> &[PatchEntry] {
        &self.entries
    }

    /// Get patch entries of a specific type
    pub fn entries_by_type(&self, entry_type: &str) -> Vec<&PatchEntry> {
        self.entries
            .iter()
            .filter(|e| e.entry_type == entry_type)
            .collect()
    }

    /// Set the main patch hash
    pub fn set_patch_hash(&mut self, hash: impl Into<String>) {
        self.properties.insert("patch".to_string(), hash.into());
    }

    /// Set the patch size
    pub fn set_patch_size(&mut self, size: u64) {
        self.properties
            .insert("patch-size".to_string(), size.to_string());
    }

    /// Add a patch entry
    pub fn add_entry(&mut self, entry: PatchEntry) {
        self.entries.push(entry);
    }

    /// Remove all entries of a specific type
    pub fn remove_entries_by_type(&mut self, entry_type: &str) {
        self.entries.retain(|e| e.entry_type != entry_type);
    }

    /// Get a raw property value
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(std::string::String::as_str)
    }

    /// Set a raw property value
    pub fn set_property(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(key.into(), value.into());
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), PatchConfigError> {
        // Must have patch hash
        if let Some(patch_hash) = self.patch_hash() {
            if !is_valid_md5_hex(patch_hash) {
                return Err(PatchConfigError::InvalidHash(patch_hash.to_string()));
            }
        } else {
            return Err(PatchConfigError::MissingPatch);
        }

        // Must have patch size
        if self.patch_size().is_none() {
            return Err(PatchConfigError::MissingPatchSize);
        }

        // Validate all patch entries
        for entry in &self.entries {
            if !is_valid_md5_hex(&entry.content_key) {
                return Err(PatchConfigError::InvalidHash(entry.content_key.clone()));
            }
            if !is_valid_md5_hex(&entry.encoding_key) {
                return Err(PatchConfigError::InvalidHash(entry.encoding_key.clone()));
            }

            if entry.entry_type.is_empty() {
                return Err(PatchConfigError::EmptyEntryType);
            }
        }

        Ok(())
    }

    /// Check if config has any entries
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.entries.is_empty()
    }

    /// Get number of patch entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get number of properties
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Clear all entries
    pub fn clear_entries(&mut self) {
        self.entries.clear();
    }

    /// Clear all properties
    pub fn clear_properties(&mut self) {
        self.properties.clear();
    }
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchEntry {
    /// Create a new patch entry
    pub fn new(
        entry_type: impl Into<String>,
        content_key: impl Into<String>,
        content_size: u64,
        encoding_key: impl Into<String>,
        encoded_size: u64,
    ) -> Self {
        Self {
            entry_type: entry_type.into(),
            content_key: content_key.into(),
            content_size,
            encoding_key: encoding_key.into(),
            encoded_size,
        }
    }

    /// Check if this entry matches a specific type
    pub fn is_type(&self, entry_type: &str) -> bool {
        self.entry_type == entry_type
    }
}

/// Patch config parsing and validation errors
#[derive(Debug, thiserror::Error)]
pub enum PatchConfigError {
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    #[error("invalid patch entry format: {0}")]
    InvalidPatchEntry(String),
    #[error("invalid size value: {0}")]
    InvalidSize(String),
    #[error("invalid hash format: {0}")]
    InvalidHash(String),
    #[error("missing patch field")]
    MissingPatch,
    #[error("missing patch-size field")]
    MissingPatchSize,
    #[error("empty entry type")]
    EmptyEntryType,
}

impl crate::CascFormat for PatchConfig {
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
    fn test_parse_sample_config() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");

        // Check basic properties
        assert_eq!(
            config.patch_hash(),
            Some("658506593cf1f98a1d9300c418ee5355")
        );
        assert_eq!(config.patch_size(), Some(22837));

        // Check entries
        assert_eq!(config.entry_count(), 1);
        let entries = config.entries();
        assert_eq!(entries[0].entry_type, "encoding");
        assert_eq!(entries[0].content_key, "b07b881f4527bda7cf8a1a2f99e8622e");
        assert_eq!(entries[0].content_size, 14_004_322);
        assert_eq!(entries[0].encoding_key, "a1b2c3d4e5f678901234567890abcdef");
        assert_eq!(entries[0].encoded_size, 14_004_322);
    }

    #[test]
    fn test_round_trip() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
custom-field = custom-value
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322
patch-entry = install c08c992e5538cda8cf2a2a3f00f9d33f 25005433 b2c3d4e5f67890123456789abcdef012 25005433
";

        let original = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse");
        let rebuilt = original.build();
        let reparsed = PatchConfig::parse(&rebuilt[..]).expect("Failed to reparse");

        // Compare basic properties
        assert_eq!(original.patch_hash(), reparsed.patch_hash());
        assert_eq!(original.patch_size(), reparsed.patch_size());
        assert_eq!(
            original.get_property("custom-field"),
            reparsed.get_property("custom-field")
        );

        // Compare entries
        assert_eq!(original.entry_count(), reparsed.entry_count());
        let orig_entries = original.entries();
        let reparsed_entries = reparsed.entries();

        for (orig, rebuilt) in orig_entries.iter().zip(reparsed_entries.iter()) {
            assert_eq!(orig.entry_type, rebuilt.entry_type);
            assert_eq!(orig.content_key, rebuilt.content_key);
            assert_eq!(orig.content_size, rebuilt.content_size);
            assert_eq!(orig.encoding_key, rebuilt.encoding_key);
            assert_eq!(orig.encoded_size, rebuilt.encoded_size);
        }
    }

    #[test]
    fn test_validation() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        config.validate().expect("Config should be valid");
    }

    #[test]
    fn test_invalid_hash() {
        let config_data = r"# Patch Configuration
patch = invalid_hash
patch-size = 22837
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            PatchConfigError::InvalidHash(_)
        ));
    }

    #[test]
    fn test_missing_patch() {
        let config_data = r"# Patch Configuration
patch-size = 22837
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            PatchConfigError::MissingPatch
        ));
    }

    #[test]
    fn test_missing_patch_size() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            PatchConfigError::MissingPatchSize
        ));
    }

    #[test]
    fn test_invalid_patch_entry_format() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding invalid_format
";

        let result = PatchConfig::parse(config_data.as_bytes());
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            PatchConfigError::InvalidPatchEntry(_)
        ));
    }

    #[test]
    fn test_entry_operations() {
        let mut config = PatchConfig::new();

        config.set_patch_hash("658506593cf1f98a1d9300c418ee5355");
        config.set_patch_size(22837);

        let entry = PatchEntry::new(
            "encoding",
            "b07b881f4527bda7cf8a1a2f99e8622e",
            14_004_322,
            "a1b2c3d4e5f678901234567890abcdef",
            14_004_322,
        );

        config.add_entry(entry.clone());
        assert_eq!(config.entry_count(), 1);

        let encoding_entries = config.entries_by_type("encoding");
        assert_eq!(encoding_entries.len(), 1);
        assert_eq!(encoding_entries[0].content_key, entry.content_key);

        config.remove_entries_by_type("encoding");
        assert_eq!(config.entry_count(), 0);
    }

    #[test]
    fn test_property_operations() {
        let mut config = PatchConfig::new();

        config.set_property("custom", "value");
        assert_eq!(config.get_property("custom"), Some("value"));
        assert_eq!(config.property_count(), 1);

        config.clear_properties();
        assert_eq!(config.property_count(), 0);
        assert!(config.get_property("custom").is_none());
    }

    #[test]
    fn test_patch_entry_helper() {
        let entry = PatchEntry::new(
            "test",
            "b07b881f4527bda7cf8a1a2f99e8622e",
            1234,
            "a1b2c3d4e5f678901234567890abcdef",
            5678,
        );

        assert!(entry.is_type("test"));
        assert!(!entry.is_type("other"));
        assert_eq!(entry.entry_type, "test");
        assert_eq!(entry.content_size, 1234);
        assert_eq!(entry.encoded_size, 5678);
    }
}
