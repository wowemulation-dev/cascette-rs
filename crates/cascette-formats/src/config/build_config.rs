//! Build Config file format implementation
//!
//! Build Config files specify system file references and metadata for a specific game build.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

use super::{is_valid_md5_hex, parse_line};

/// Build Configuration containing system file references
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Raw key-value pairs from the file
    entries: HashMap<String, Vec<String>>,
}

/// A partial priority entry mapping a content key to a download priority
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialPriority {
    /// Content key identifier
    pub key: String,
    /// Download priority value
    pub priority: u32,
}

/// Information about a referenced build file
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// Content key for the file
    pub content_key: String,
    /// Optional encoding key (for encoding file)
    pub encoding_key: Option<String>,
    /// Optional size information
    pub size: Option<u64>,
}

impl BuildConfig {
    /// Create a new empty `BuildConfig`
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Parse `BuildConfig` from a reader
    pub fn parse<R: Read>(reader: R) -> Result<Self, Box<dyn std::error::Error>> {
        let mut entries = HashMap::new();
        let reader = BufReader::new(reader);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key-value pair
            if let Some((key, value)) = parse_line(line) {
                // Split value by spaces
                let values: Vec<String> = value.split_whitespace().map(String::from).collect();

                entries.insert(key, values);
            }
        }

        Ok(Self { entries })
    }

    /// Build the config file content
    pub fn build(&self) -> Vec<u8> {
        let mut output = Vec::new();

        // Output in a specific order for consistency
        let order = [
            "root",
            "install",
            "install-size",
            "download",
            "download-size",
            "size",
            "size-size",
            "encoding",
            "encoding-size",
            "patch",
            "patch-size",
            "patch-config",
            "patch-index",
            "patch-index-size",
            "build-name",
            "build-uid",
            "build-product",
            "build-playbuild-installer",
            "build-partial-priority",
            "build-playtime-url",
            "build-product-espec",
            "vfs-root",
            "vfs-root-size",
        ];

        // Write header comment
        let _ = writeln!(output, "# Build Configuration");
        let _ = writeln!(output);

        for key in &order {
            if let Some(values) = self.entries.get(*key) {
                let _ = writeln!(output, "{} = {}", key, values.join(" "));
            }
        }

        // Output any remaining keys not in our order
        let mut remaining: Vec<_> = self
            .entries
            .keys()
            .filter(|k| !order.contains(&k.as_str()))
            .collect();
        remaining.sort();

        for key in remaining {
            let values = &self.entries[key];
            let _ = writeln!(output, "{} = {}", key, values.join(" "));
        }

        // Add trailing newlines to match common format
        let _ = writeln!(output);
        let _ = writeln!(output);

        output
    }

    /// Get the root file hash
    pub fn root(&self) -> Option<&str> {
        self.entries
            .get("root")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get encoding file information
    pub fn encoding(&self) -> Option<BuildInfo> {
        let values = self.entries.get("encoding")?;
        let content_key = values.first()?.clone();
        let encoding_key = values.get(1).cloned(); // Second value is encoding key

        let size = self
            .entries
            .get("encoding-size")
            .and_then(|v| v.get(1))
            .and_then(|s| s.parse().ok());

        Some(BuildInfo {
            content_key,
            encoding_key,
            size,
        })
    }

    /// Get encoding key for the encoding file
    pub fn encoding_key(&self) -> Option<&str> {
        self.entries
            .get("encoding")
            .and_then(|values| values.get(1)) // Second hash is encoding key
            .map(std::string::String::as_str)
    }

    /// Get install file information
    /// Format: install = <content_key_1> <encoding_key_1> [<content_key_2> <encoding_key_2> ...]
    pub fn install(&self) -> Vec<BuildInfo> {
        let install = self.entries.get("install").cloned().unwrap_or_default();

        let sizes = self
            .entries
            .get("install-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Values alternate: content_key, encoding_key, content_key, encoding_key, ...
        install
            .chunks(2)
            .enumerate()
            .filter_map(|(i, chunk)| {
                if chunk.len() >= 2 {
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: Some(chunk[1].clone()),
                        size: sizes.get(i).copied(),
                    })
                } else if chunk.len() == 1 {
                    // Handle odd number of values (content_key without encoding_key)
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: None,
                        size: sizes.get(i).copied(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get download file information
    /// Format: download = <content_key_1> <encoding_key_1> [<content_key_2> <encoding_key_2> ...]
    pub fn download(&self) -> Vec<BuildInfo> {
        let download = self.entries.get("download").cloned().unwrap_or_default();

        let sizes = self
            .entries
            .get("download-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Values alternate: content_key, encoding_key, content_key, encoding_key, ...
        download
            .chunks(2)
            .enumerate()
            .filter_map(|(i, chunk)| {
                if chunk.len() >= 2 {
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: Some(chunk[1].clone()),
                        size: sizes.get(i).copied(),
                    })
                } else if chunk.len() == 1 {
                    // Handle odd number of values (content_key without encoding_key)
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: None,
                        size: sizes.get(i).copied(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get patch information if available.
    ///
    /// Format: `patch = CONTENT_KEY [ENCODING_KEY]`
    pub fn patch(&self) -> Option<BuildInfo> {
        let values = self.entries.get("patch")?;
        let content_key = values.first()?.clone();
        let encoding_key = values.get(1).cloned(); // Optional second value is encoding key

        let size = self
            .entries
            .get("patch-size")
            .and_then(|v| v.first())
            .and_then(|s| s.parse().ok());

        Some(BuildInfo {
            content_key,
            encoding_key,
            size,
        })
    }

    /// Get patch config hash if available
    pub fn patch_config(&self) -> Option<&str> {
        self.entries
            .get("patch-config")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get patch index file information
    /// Format: patch-index = <content_key_1> <encoding_key_1> [<content_key_2> <encoding_key_2> ...]
    pub fn patch_index(&self) -> Vec<BuildInfo> {
        let patch_index = self.entries.get("patch-index").cloned().unwrap_or_default();

        let sizes = self
            .entries
            .get("patch-index-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Values alternate: content_key, encoding_key, content_key, encoding_key, ...
        patch_index
            .chunks(2)
            .enumerate()
            .filter_map(|(i, chunk)| {
                if chunk.len() >= 2 {
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: Some(chunk[1].clone()),
                        size: sizes.get(i).copied(),
                    })
                } else if chunk.len() == 1 {
                    // Handle odd number of values (content_key without encoding_key)
                    Some(BuildInfo {
                        content_key: chunk[0].clone(),
                        encoding_key: None,
                        size: sizes.get(i).copied(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get build metadata
    pub fn build_name(&self) -> Option<&str> {
        self.entries
            .get("build-name")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get build UID
    pub fn build_uid(&self) -> Option<&str> {
        self.entries
            .get("build-uid")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get build product
    pub fn build_product(&self) -> Option<&str> {
        self.entries
            .get("build-product")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get size file information
    ///
    /// Format: `size = CONTENT_KEY [ENCODING_KEY]`, `size-size = SIZE [ENCODING_SIZE]`
    pub fn size(&self) -> Option<BuildInfo> {
        let values = self.entries.get("size")?;
        let content_key = values.first()?.clone();
        let encoding_key = values.get(1).cloned();

        let size = self
            .entries
            .get("size-size")
            .and_then(|v| v.get(1))
            .and_then(|s| s.parse().ok());

        Some(BuildInfo {
            content_key,
            encoding_key,
            size,
        })
    }

    /// Get VFS root file information
    ///
    /// Format: `vfs-root = CONTENT_KEY [ENCODING_KEY]`, `vfs-root-size = SIZE [ENCODING_SIZE]`
    pub fn vfs_root(&self) -> Option<BuildInfo> {
        let values = self.entries.get("vfs-root")?;
        let content_key = values.first()?.clone();
        let encoding_key = values.get(1).cloned();

        let size = self
            .entries
            .get("vfs-root-size")
            .and_then(|v| v.get(1))
            .and_then(|s| s.parse().ok());

        Some(BuildInfo {
            content_key,
            encoding_key,
            size,
        })
    }

    /// Get build playtime URL
    pub fn build_playtime_url(&self) -> Option<&str> {
        self.entries
            .get("build-playtime-url")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get build product espec
    pub fn build_product_espec(&self) -> Option<&str> {
        self.entries
            .get("build-product-espec")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get VFS file entries
    ///
    /// Keys: `vfs-1` through `vfs-N` with parallel `vfs-1-size` etc.
    /// Each entry uses the dual-hash format. Returns entries with their 1-based index.
    /// Iterates sequentially from 1 and stops at the first missing index.
    pub fn vfs_entries(&self) -> Vec<(u32, BuildInfo)> {
        let mut result = Vec::new();
        let mut index = 1u32;

        loop {
            let key = format!("vfs-{index}");
            let Some(values) = self.entries.get(&key) else {
                break;
            };

            let content_key = match values.first() {
                Some(k) => k.clone(),
                None => break,
            };
            let encoding_key = values.get(1).cloned();

            let size_key = format!("vfs-{index}-size");
            let size = self
                .entries
                .get(&size_key)
                .and_then(|v| v.get(1))
                .and_then(|s| s.parse().ok());

            result.push((
                index,
                BuildInfo {
                    content_key,
                    encoding_key,
                    size,
                },
            ));
            index += 1;
        }

        result
    }

    /// Get build partial priority entries
    ///
    /// Format: `build-partial-priority = key1:priority1,key2:priority2,...`
    ///
    /// Values are stored as a single comma-separated string. Malformed entries are skipped.
    pub fn build_partial_priority(&self) -> Vec<PartialPriority> {
        let Some(values) = self.entries.get("build-partial-priority") else {
            return Vec::new();
        };

        // The value is stored as space-separated tokens (from the generic parser),
        // but the actual format is comma-separated within a single value.
        // Join back and split on commas.
        let joined = values.join(" ");
        joined
            .split(',')
            .filter_map(|entry| {
                let entry = entry.trim();
                let (key, priority_str) = entry.rsplit_once(':')?;
                let priority = priority_str.parse::<u32>().ok()?;
                Some(PartialPriority {
                    key: key.to_string(),
                    priority,
                })
            })
            .collect()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Must have root
        if !self.entries.contains_key("root") {
            return Err(ValidationError::MissingRoot);
        }

        // Must have encoding with two hashes
        if let Some(encoding) = self.entries.get("encoding") {
            if encoding.len() < 2 {
                return Err(ValidationError::InvalidEncoding);
            }
            // Validate hash formats
            for hash in encoding {
                if !is_valid_md5_hex(hash) {
                    return Err(ValidationError::InvalidHash(hash.clone()));
                }
            }
        } else {
            return Err(ValidationError::MissingEncoding);
        }

        // Validate all hash values
        for (key, values) in &self.entries {
            // Skip non-hash fields
            if key.starts_with("build-") || key.ends_with("-size") {
                continue;
            }

            for value in values {
                // Skip numeric values
                if value.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                // Must be valid MD5 hash
                if !is_valid_md5_hex(value) {
                    return Err(ValidationError::InvalidHash(value.clone()));
                }
            }
        }

        Ok(())
    }

    /// Get raw entry by key
    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.entries.get(key)
    }

    /// Set a key-value pair
    pub fn set(&mut self, key: impl Into<String>, values: Vec<String>) {
        self.entries.insert(key.into(), values);
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Build config validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("missing root field")]
    MissingRoot,
    #[error("missing encoding field")]
    MissingEncoding,
    #[error("encoding field must have two hashes")]
    InvalidEncoding,
    #[error("invalid hash format: {0}")]
    InvalidHash(String),
}

impl crate::CascFormat for BuildConfig {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(self.build())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn hash(n: u8) -> String {
        format!("{:032x}", n)
    }

    #[test]
    fn test_size_accessor() {
        let mut config = BuildConfig::new();
        config.set("size", vec![hash(1), hash(2)]);
        config.set("size-size", vec!["100".into(), "200".into()]);

        let info = config.size().expect("size should be present");
        assert_eq!(info.content_key, hash(1));
        assert_eq!(info.encoding_key.as_deref(), Some(hash(2)).as_deref());
        assert_eq!(info.size, Some(200));
    }

    #[test]
    fn test_size_missing() {
        let config = BuildConfig::new();
        assert!(config.size().is_none());
    }

    #[test]
    fn test_vfs_root_accessor() {
        let mut config = BuildConfig::new();
        config.set("vfs-root", vec![hash(3), hash(4)]);
        config.set("vfs-root-size", vec!["300".into(), "400".into()]);

        let info = config.vfs_root().expect("vfs_root should be present");
        assert_eq!(info.content_key, hash(3));
        assert_eq!(info.encoding_key.as_deref(), Some(hash(4)).as_deref());
        assert_eq!(info.size, Some(400));
    }

    #[test]
    fn test_vfs_root_missing() {
        let config = BuildConfig::new();
        assert!(config.vfs_root().is_none());
    }

    #[test]
    fn test_build_playtime_url() {
        let mut config = BuildConfig::new();
        config.set(
            "build-playtime-url",
            vec!["https://example.com/playtime".into()],
        );

        assert_eq!(
            config.build_playtime_url(),
            Some("https://example.com/playtime")
        );
    }

    #[test]
    fn test_build_playtime_url_missing() {
        let config = BuildConfig::new();
        assert!(config.build_playtime_url().is_none());
    }

    #[test]
    fn test_build_product_espec() {
        let mut config = BuildConfig::new();
        config.set("build-product-espec", vec!["wow_classic".into()]);

        assert_eq!(config.build_product_espec(), Some("wow_classic"));
    }

    #[test]
    fn test_build_product_espec_missing() {
        let config = BuildConfig::new();
        assert!(config.build_product_espec().is_none());
    }

    #[test]
    fn test_build_partial_priority() {
        let mut config = BuildConfig::new();
        config.set(
            "build-partial-priority",
            vec!["speech:0,world:1,base:2".into()],
        );

        let priorities = config.build_partial_priority();
        assert_eq!(priorities.len(), 3);
        assert_eq!(priorities[0].key, "speech");
        assert_eq!(priorities[0].priority, 0);
        assert_eq!(priorities[1].key, "world");
        assert_eq!(priorities[1].priority, 1);
        assert_eq!(priorities[2].key, "base");
        assert_eq!(priorities[2].priority, 2);
    }

    #[test]
    fn test_build_partial_priority_malformed_skipped() {
        let mut config = BuildConfig::new();
        config.set(
            "build-partial-priority",
            vec!["speech:0,bad_entry,world:abc,base:2".into()],
        );

        let priorities = config.build_partial_priority();
        assert_eq!(priorities.len(), 2);
        assert_eq!(priorities[0].key, "speech");
        assert_eq!(priorities[1].key, "base");
    }

    #[test]
    fn test_build_partial_priority_empty() {
        let config = BuildConfig::new();
        assert!(config.build_partial_priority().is_empty());
    }

    #[test]
    fn test_vfs_entries() {
        let mut config = BuildConfig::new();
        config.set("vfs-1", vec![hash(10), hash(11)]);
        config.set("vfs-1-size", vec!["1000".into(), "1100".into()]);
        config.set("vfs-2", vec![hash(20), hash(21)]);
        config.set("vfs-2-size", vec!["2000".into(), "2100".into()]);

        let entries = config.vfs_entries();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].0, 1);
        assert_eq!(entries[0].1.content_key, hash(10));
        assert_eq!(
            entries[0].1.encoding_key.as_deref(),
            Some(hash(11)).as_deref()
        );
        assert_eq!(entries[0].1.size, Some(1100));

        assert_eq!(entries[1].0, 2);
        assert_eq!(entries[1].1.content_key, hash(20));
    }

    #[test]
    fn test_vfs_entries_stops_at_gap() {
        let mut config = BuildConfig::new();
        config.set("vfs-1", vec![hash(10)]);
        // Skip vfs-2
        config.set("vfs-3", vec![hash(30)]);

        let entries = config.vfs_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, 1);
    }

    #[test]
    fn test_vfs_entries_empty() {
        let config = BuildConfig::new();
        assert!(config.vfs_entries().is_empty());
    }

    #[test]
    fn test_round_trip_new_accessors() {
        let mut config = BuildConfig::new();
        // Required fields for a valid-ish config
        config.set("root", vec![hash(1)]);
        config.set("encoding", vec![hash(2), hash(3)]);
        config.set("encoding-size", vec!["100".into(), "200".into()]);

        // New fields
        config.set("size", vec![hash(4), hash(5)]);
        config.set("size-size", vec!["300".into(), "400".into()]);
        config.set("vfs-root", vec![hash(6), hash(7)]);
        config.set("vfs-root-size", vec!["500".into(), "600".into()]);
        config.set("build-playtime-url", vec!["https://example.com/pt".into()]);
        config.set("build-product-espec", vec!["wow".into()]);
        config.set("build-partial-priority", vec!["speech:0,world:1".into()]);

        let built = config.build();
        let reparsed = BuildConfig::parse(&built[..]).expect("reparse should succeed");

        // Verify all new accessors survive round-trip
        let size = reparsed.size().expect("size");
        assert_eq!(size.content_key, hash(4));
        assert_eq!(size.size, Some(400));

        let vfs_root = reparsed.vfs_root().expect("vfs_root");
        assert_eq!(vfs_root.content_key, hash(6));

        assert_eq!(
            reparsed.build_playtime_url(),
            Some("https://example.com/pt")
        );
        assert_eq!(reparsed.build_product_espec(), Some("wow"));

        let priorities = reparsed.build_partial_priority();
        assert_eq!(priorities.len(), 2);
        assert_eq!(priorities[0].key, "speech");
    }
}
