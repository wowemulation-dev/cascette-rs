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
///
/// Format from RE spec (`ParsePatchConfigEntry`):
/// ```text
/// patch-entry = <keyword> <target_ckey> <target_csize> <target_ekey> <target_esize> <target_espec>
///               [<original_ekey> <original_csize> <p_key> <p_size>]*
/// ```
///
/// The first 5 fields identify the target file. The optional `espec` field
/// specifies the target encoding specification. Following that, zero or more
/// groups of 4 fields describe available patches from different source versions.
#[derive(Debug, Clone)]
pub struct PatchEntry {
    /// Type of entry (e.g., "encoding")
    pub entry_type: String,
    /// Content key hash (target CKey)
    pub content_key: String,
    /// Content size in bytes (target content size)
    pub content_size: u64,
    /// Encoding key hash (target EKey)
    pub encoding_key: String,
    /// Encoded size in bytes (target encoded size)
    pub encoded_size: u64,
    /// Target encoding specification (e.g., "b:{*=z}")
    pub espec: String,
    /// Repeating patch record groups (0 or more)
    pub patch_records: Vec<PatchRecord>,
}

/// A single patch record within a `PatchEntry`
///
/// Each record describes one available patch from a specific source version
/// to the target version defined by the parent entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchRecord {
    /// Base file encoding key (hex)
    pub original_ekey: String,
    /// Base file content size
    pub original_size: u64,
    /// Patch data encoding key (hex)
    pub patch_key: String,
    /// Patch data encoded size
    pub patch_size: u64,
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
    ///
    /// Full format (RE spec):
    /// ```text
    /// patch-entry = <type> <ckey> <csize> <ekey> <esize> [<espec>] [<orig_ekey> <orig_size> <p_key> <p_size>]*
    /// ```
    ///
    /// Backward compatibility: entries with only 6-7 parts (no espec) are accepted
    /// with espec defaulting to empty string.
    fn parse_patch_entry(line: &str) -> Result<PatchEntry, PatchConfigError> {
        let parts: Vec<&str> = line.split_whitespace().collect();

        // Minimum: "patch-entry" "=" type ckey csize ekey = 6 parts
        if parts.len() < 6 {
            return Err(PatchConfigError::InvalidPatchEntry(format!(
                "Expected at least 6 parts, found {}: {}",
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

        // Validate hash formats
        if !is_valid_md5_hex(&content_key) {
            return Err(PatchConfigError::InvalidHash(content_key));
        }
        if !is_valid_md5_hex(&encoding_key) {
            return Err(PatchConfigError::InvalidHash(encoding_key));
        }

        // Determine encoded_size, espec, and patch records based on part count.
        //
        // Old format (6-7 parts): type ckey csize ekey [esize]
        // New format (8+ parts):   type ckey csize ekey esize espec [records...]
        let (encoded_size, espec, record_start) = if parts.len() <= 7 {
            // Old format: no espec, optional encoded_size
            let esize = if parts.len() == 7 {
                parts[6]
                    .parse::<u64>()
                    .map_err(|_| PatchConfigError::InvalidSize(parts[6].to_string()))?
            } else {
                content_size
            };
            (esize, String::new(), parts.len())
        } else {
            // New format: parts[6] = encoded_size, parts[7] = espec
            let esize = parts[6]
                .parse::<u64>()
                .map_err(|_| PatchConfigError::InvalidSize(parts[6].to_string()))?;
            let espec = parts[7].to_string();
            (esize, espec, 8)
        };

        // Parse repeating patch record groups (4 fields each)
        let remaining = &parts[record_start..];
        if !remaining.len().is_multiple_of(4) {
            return Err(PatchConfigError::InvalidPatchEntry(format!(
                "Patch records must be groups of 4 fields, got {} trailing fields",
                remaining.len()
            )));
        }

        let mut patch_records = Vec::new();
        for chunk in remaining.chunks(4) {
            let original_ekey = chunk[0].to_string();
            if !is_valid_md5_hex(&original_ekey) {
                return Err(PatchConfigError::InvalidHash(original_ekey));
            }
            let original_size = chunk[1]
                .parse::<u64>()
                .map_err(|_| PatchConfigError::InvalidSize(chunk[1].to_string()))?;
            let patch_key = chunk[2].to_string();
            if !is_valid_md5_hex(&patch_key) {
                return Err(PatchConfigError::InvalidHash(patch_key));
            }
            let patch_size = chunk[3]
                .parse::<u64>()
                .map_err(|_| PatchConfigError::InvalidSize(chunk[3].to_string()))?;

            patch_records.push(PatchRecord {
                original_ekey,
                original_size,
                patch_key,
                patch_size,
            });
        }

        Ok(PatchEntry {
            entry_type,
            content_key,
            content_size,
            encoding_key,
            encoded_size,
            espec,
            patch_records,
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
            let mut line = format!(
                "patch-entry = {} {} {} {} {}",
                entry.entry_type,
                entry.content_key,
                entry.content_size,
                entry.encoding_key,
                entry.encoded_size
            );

            // Emit espec and patch records when espec is present or records exist
            if !entry.espec.is_empty() || !entry.patch_records.is_empty() {
                line.push(' ');
                line.push_str(if entry.espec.is_empty() {
                    "n"
                } else {
                    &entry.espec
                });

                for rec in &entry.patch_records {
                    use std::fmt::Write as _;
                    let _ = write!(
                        line,
                        " {} {} {} {}",
                        rec.original_ekey, rec.original_size, rec.patch_key, rec.patch_size
                    );
                }
            }

            let _ = writeln!(output, "{line}");
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
            espec: String::new(),
            patch_records: Vec::new(),
        }
    }

    /// Create a new patch entry with an encoding specification
    pub fn with_espec(
        entry_type: impl Into<String>,
        content_key: impl Into<String>,
        content_size: u64,
        encoding_key: impl Into<String>,
        encoded_size: u64,
        espec: impl Into<String>,
    ) -> Self {
        Self {
            entry_type: entry_type.into(),
            content_key: content_key.into(),
            content_size,
            encoding_key: encoding_key.into(),
            encoded_size,
            espec: espec.into(),
            patch_records: Vec::new(),
        }
    }

    /// Check if this entry matches a specific type
    pub fn is_type(&self, entry_type: &str) -> bool {
        self.entry_type == entry_type
    }

    /// Whether this entry has any patch records
    pub fn has_patch_records(&self) -> bool {
        !self.patch_records.is_empty()
    }

    /// Number of patch records (chain length)
    pub fn patch_chain_length(&self) -> usize {
        self.patch_records.len()
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
            assert_eq!(orig.espec, rebuilt.espec);
            assert_eq!(orig.patch_records.len(), rebuilt.patch_records.len());
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
        assert!(entry.espec.is_empty());
        assert!(!entry.has_patch_records());
        assert_eq!(entry.patch_chain_length(), 0);
    }

    #[test]
    fn test_parse_entry_with_espec() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322 b:{*=z}
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        assert_eq!(config.entry_count(), 1);
        let entry = &config.entries()[0];
        assert_eq!(entry.espec, "b:{*=z}");
        assert!(!entry.has_patch_records());
    }

    #[test]
    fn test_parse_entry_with_patch_records() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322 b:{*=z} c08c992e5538cda8cf2a2a3f00f9d33f 13000000 d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6 500000
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let entry = &config.entries()[0];
        assert_eq!(entry.espec, "b:{*=z}");
        assert!(entry.has_patch_records());
        assert_eq!(entry.patch_chain_length(), 1);

        let rec = &entry.patch_records[0];
        assert_eq!(rec.original_ekey, "c08c992e5538cda8cf2a2a3f00f9d33f");
        assert_eq!(rec.original_size, 13_000_000);
        assert_eq!(rec.patch_key, "d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6");
        assert_eq!(rec.patch_size, 500_000);
    }

    #[test]
    fn test_parse_entry_with_multiple_patch_records() {
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef 14004322 n c08c992e5538cda8cf2a2a3f00f9d33f 13000000 d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6 500000 e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6 12000000 f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6 400000
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let entry = &config.entries()[0];
        assert_eq!(entry.patch_chain_length(), 2);
        assert_eq!(entry.patch_records[0].original_size, 13_000_000);
        assert_eq!(entry.patch_records[1].original_size, 12_000_000);
    }

    #[test]
    fn test_round_trip_with_espec_and_records() {
        let mut config = PatchConfig::new();
        config.set_patch_hash("658506593cf1f98a1d9300c418ee5355");
        config.set_patch_size(22837);

        let mut entry = PatchEntry::with_espec(
            "encoding",
            "b07b881f4527bda7cf8a1a2f99e8622e",
            14_004_322,
            "a1b2c3d4e5f678901234567890abcdef",
            14_004_322,
            "b:{*=z}",
        );
        entry.patch_records.push(PatchRecord {
            original_ekey: "c08c992e5538cda8cf2a2a3f00f9d33f".to_string(),
            original_size: 13_000_000,
            patch_key: "d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6".to_string(),
            patch_size: 500_000,
        });
        config.add_entry(entry);

        let built = config.build();
        let reparsed = PatchConfig::parse(&built[..]).expect("Failed to reparse");
        let re_entry = &reparsed.entries()[0];
        assert_eq!(re_entry.espec, "b:{*=z}");
        assert_eq!(re_entry.patch_chain_length(), 1);
        assert_eq!(
            re_entry.patch_records[0].original_ekey,
            "c08c992e5538cda8cf2a2a3f00f9d33f"
        );
    }

    #[test]
    fn test_backward_compat_no_espec() {
        // Old format without espec should still parse
        let config_data = r"# Patch Configuration
patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837
patch-entry = encoding b07b881f4527bda7cf8a1a2f99e8622e 14004322 a1b2c3d4e5f678901234567890abcdef
";

        let config = PatchConfig::parse(config_data.as_bytes()).expect("Failed to parse config");
        let entry = &config.entries()[0];
        assert_eq!(entry.encoded_size, 14_004_322); // defaults to content_size
        assert!(entry.espec.is_empty());
        assert!(!entry.has_patch_records());
    }
}
