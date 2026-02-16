//! CDN Config file format implementation
//!
//! CDN Config files specify the location and organization of content archives on the CDN.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

use super::{is_valid_md5_hex, parse_line};

/// CDN Configuration containing archive references
#[derive(Debug, Clone)]
pub struct CdnConfig {
    /// Raw key-value pairs from the file
    entries: HashMap<String, Vec<String>>,
}

/// Information about a content archive
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    /// Content key of the archive
    pub content_key: String,
    /// Size of the archive's index (if available)
    pub index_size: Option<u64>,
}

impl CdnConfig {
    /// Create a new empty `CdnConfig`
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Parse `CdnConfig` from a reader
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

                // CDN config typically doesn't have duplicate keys
                // But we'll extend to support it if needed
                entries.entry(key).or_insert_with(Vec::new).extend(values);
            }
        }

        Ok(Self { entries })
    }

    /// Build the config file content
    pub fn build(&self) -> Vec<u8> {
        let mut output = Vec::new();

        // Output in a specific order for consistency
        let order = [
            "archives",
            "archives-index-size",
            "archive-group",
            "patch-archives",
            "patch-archives-index-size",
            "patch-archive-group",
            "file-index",
            "file-index-size",
            "patch-file-index",
            "patch-file-index-size",
        ];

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

        output
    }

    /// Get all archive entries with their index sizes
    pub fn archives(&self) -> Vec<ArchiveInfo> {
        let archives = self.entries.get("archives").cloned().unwrap_or_default();

        let sizes = self
            .entries
            .get("archives-index-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        archives
            .into_iter()
            .enumerate()
            .map(|(i, content_key)| ArchiveInfo {
                content_key,
                index_size: sizes.get(i).copied(),
            })
            .collect()
    }

    /// Get the archive group hash if present
    pub fn archive_group(&self) -> Option<&str> {
        self.entries
            .get("archive-group")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get the patch archive group hash if present
    pub fn patch_archive_group(&self) -> Option<&str> {
        self.entries
            .get("patch-archive-group")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Check if patch archives are present
    pub fn has_patch_archives(&self) -> bool {
        self.entries.contains_key("patch-archives")
    }

    /// Get patch archive entries
    pub fn patch_archives(&self) -> Vec<ArchiveInfo> {
        let archives = self
            .entries
            .get("patch-archives")
            .cloned()
            .unwrap_or_default();

        let sizes = self
            .entries
            .get("patch-archives-index-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        archives
            .into_iter()
            .enumerate()
            .map(|(i, content_key)| ArchiveInfo {
                content_key,
                index_size: sizes.get(i).copied(),
            })
            .collect()
    }

    /// Get file index hash if present (single hash, historical)
    pub fn file_index(&self) -> Option<&str> {
        self.entries
            .get("file-index")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get file index entries (historical, multiple indices)
    pub fn file_indices(&self) -> Vec<ArchiveInfo> {
        let indices = self.entries.get("file-index").cloned().unwrap_or_default();

        let sizes = self
            .entries
            .get("file-index-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        indices
            .into_iter()
            .enumerate()
            .map(|(i, content_key)| ArchiveInfo {
                content_key,
                index_size: sizes.get(i).copied(),
            })
            .collect()
    }

    /// Set archives with their index sizes
    pub fn set_archives(&mut self, archives: Vec<ArchiveInfo>) {
        let mut archive_hashes = Vec::new();
        let mut index_sizes = Vec::new();
        let mut has_any_size = false;

        for info in archives {
            archive_hashes.push(info.content_key);
            if let Some(size) = info.index_size {
                index_sizes.push(size.to_string());
                has_any_size = true;
            } else {
                // If any archive has a size, we need to maintain position
                // This will be fixed later if all archives have sizes
                index_sizes.push(String::new());
            }
        }

        self.entries.insert("archives".to_string(), archive_hashes);

        // Only add index sizes if at least one archive has a size
        // and trim trailing empty entries
        if has_any_size {
            // Remove trailing empty strings
            while index_sizes.last() == Some(&String::new()) {
                index_sizes.pop();
            }

            if !index_sizes.is_empty() {
                self.entries
                    .insert("archives-index-size".to_string(), index_sizes);
            }
        }
    }

    /// Set the archive group
    pub fn set_archive_group(&mut self, hash: impl Into<String>) {
        self.entries
            .insert("archive-group".to_string(), vec![hash.into()]);
    }

    /// Validate the configuration
    #[allow(clippy::expect_used)] // archives key existence verified above
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Must have archives
        if !self.entries.contains_key("archives") {
            return Err(ValidationError::NoArchives);
        }

        let archives = self
            .entries
            .get("archives")
            .expect("archives key existence verified above");
        let sizes = self.entries.get("archives-index-size");

        // If sizes present, must match archive count
        if let Some(sizes) = sizes
            && archives.len() != sizes.len()
        {
            return Err(ValidationError::SizeMismatch {
                archives: archives.len(),
                sizes: sizes.len(),
            });
        }

        // Validate hash formats
        for hash in archives {
            if !is_valid_md5_hex(hash) {
                return Err(ValidationError::InvalidHash(hash.clone()));
            }
        }

        // Validate patch archives if present
        if let Some(patch_archives) = self.entries.get("patch-archives") {
            let patch_sizes = self.entries.get("patch-archives-index-size");

            if let Some(sizes) = patch_sizes
                && patch_archives.len() != sizes.len()
            {
                return Err(ValidationError::PatchSizeMismatch {
                    archives: patch_archives.len(),
                    sizes: sizes.len(),
                });
            }

            for hash in patch_archives {
                if !is_valid_md5_hex(hash) {
                    return Err(ValidationError::InvalidHash(hash.clone()));
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

    /// Get the total number of archives
    pub fn archive_count(&self) -> usize {
        self.entries.get("archives").map_or(0, std::vec::Vec::len)
    }

    /// Check if file indices are present (historical)
    pub fn has_file_indices(&self) -> bool {
        self.entries.contains_key("file-index")
    }

    /// Get patch file index hash if present
    pub fn patch_file_index(&self) -> Option<&str> {
        self.entries
            .get("patch-file-index")
            .and_then(|v| v.first())
            .map(std::string::String::as_str)
    }

    /// Get patch file index size if present
    pub fn patch_file_index_size(&self) -> Option<u64> {
        self.entries
            .get("patch-file-index-size")
            .and_then(|v| v.first())
            .and_then(|s| s.parse().ok())
    }

    /// Get patch file index entries with sizes
    pub fn patch_file_indices(&self) -> Vec<ArchiveInfo> {
        let indices = self
            .entries
            .get("patch-file-index")
            .cloned()
            .unwrap_or_default();

        let sizes = self
            .entries
            .get("patch-file-index-size")
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        indices
            .into_iter()
            .enumerate()
            .map(|(i, content_key)| ArchiveInfo {
                content_key,
                index_size: sizes.get(i).copied(),
            })
            .collect()
    }
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// CDN config validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("missing archives field")]
    NoArchives,
    #[error("archive count ({archives}) doesn't match size count ({sizes})")]
    SizeMismatch { archives: usize, sizes: usize },
    #[error("patch archive count ({archives}) doesn't match size count ({sizes})")]
    PatchSizeMismatch { archives: usize, sizes: usize },
    #[error("invalid hash format: {0}")]
    InvalidHash(String),
}

impl crate::CascFormat for CdnConfig {
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

    // NOTE: test_parse_sample_config removed after tools restructuring - external test data no longer available

    #[test]
    fn test_round_trip() {
        // Create a minimal config for predictable round-trip
        let mut config = CdnConfig::new();
        config.set_archives(vec![
            ArchiveInfo {
                content_key: "0036fbcc88e4c2e817b1bbaa89397c75".to_string(),
                index_size: Some(12_345),
            },
            ArchiveInfo {
                content_key: "00f40d4a63bcc2e87cf0fb62a3c47da4".to_string(),
                index_size: Some(67_890),
            },
        ]);
        config.set_archive_group("9e13aa0f34968b1f9b4fc7e09ae88d26");

        // Use the test utility for basic build-parse validation
        crate::test_utils::test_build_parse(&config).expect("Build-parse should succeed");

        // Still do detailed validation since we need to check specific fields
        let built = config.build();
        let reparsed = CdnConfig::parse(&built[..]).expect("Failed to reparse");

        // Compare archives
        let original_archives = config.archives();
        let reparsed_archives = reparsed.archives();
        assert_eq!(original_archives.len(), reparsed_archives.len());

        for (orig, rebuilt) in original_archives.iter().zip(reparsed_archives.iter()) {
            assert_eq!(orig.content_key, rebuilt.content_key);
            assert_eq!(orig.index_size, rebuilt.index_size);
        }

        assert_eq!(config.archive_group(), reparsed.archive_group());
    }

    // NOTE: test_validation removed after tools restructuring - external test data no longer available

    #[test]
    fn test_patch_file_index() {
        let mut config = CdnConfig::new();
        config.set(
            "patch-file-index",
            vec!["aabbccddee0011223344556677889900".to_string()],
        );

        assert_eq!(
            config.patch_file_index(),
            Some("aabbccddee0011223344556677889900")
        );
    }

    #[test]
    fn test_patch_file_index_missing() {
        let config = CdnConfig::new();
        assert!(config.patch_file_index().is_none());
    }

    #[test]
    fn test_patch_file_index_size() {
        let mut config = CdnConfig::new();
        config.set("patch-file-index-size", vec!["54321".to_string()]);

        assert_eq!(config.patch_file_index_size(), Some(54321));
    }

    #[test]
    fn test_patch_file_index_size_missing() {
        let config = CdnConfig::new();
        assert!(config.patch_file_index_size().is_none());
    }

    #[test]
    fn test_patch_file_indices() {
        let mut config = CdnConfig::new();
        config.set(
            "patch-file-index",
            vec![
                "aabbccddee0011223344556677889900".to_string(),
                "0099887766554433221100eeddccbbaa".to_string(),
            ],
        );
        config.set(
            "patch-file-index-size",
            vec!["1000".to_string(), "2000".to_string()],
        );

        let indices = config.patch_file_indices();
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0].content_key, "aabbccddee0011223344556677889900");
        assert_eq!(indices[0].index_size, Some(1000));
        assert_eq!(indices[1].content_key, "0099887766554433221100eeddccbbaa");
        assert_eq!(indices[1].index_size, Some(2000));
    }

    #[test]
    fn test_patch_file_indices_empty() {
        let config = CdnConfig::new();
        assert!(config.patch_file_indices().is_empty());
    }

    #[test]
    fn test_round_trip_with_patch_file_index() {
        let mut config = CdnConfig::new();
        config.set_archives(vec![ArchiveInfo {
            content_key: "0036fbcc88e4c2e817b1bbaa89397c75".to_string(),
            index_size: Some(12_345),
        }]);
        config.set(
            "patch-file-index",
            vec!["aabbccddee0011223344556677889900".to_string()],
        );
        config.set("patch-file-index-size", vec!["5000".to_string()]);

        let built = config.build();
        let reparsed = CdnConfig::parse(&built[..]).expect("reparse should succeed");

        assert_eq!(
            reparsed.patch_file_index(),
            Some("aabbccddee0011223344556677889900")
        );
        assert_eq!(reparsed.patch_file_index_size(), Some(5000));
    }

    #[test]
    fn test_archive_with_sizes() {
        let mut config = CdnConfig::new();

        let archives = vec![
            ArchiveInfo {
                content_key: "aabbccddee0011223344556677889900".to_string(),
                index_size: Some(1000),
            },
            ArchiveInfo {
                content_key: "0099887766554433221100eeddccbbaa".to_string(),
                index_size: Some(2000),
            },
        ];

        config.set_archives(archives.clone());

        let retrieved = config.archives();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].content_key, archives[0].content_key);
        assert_eq!(retrieved[0].index_size, archives[0].index_size);
        assert_eq!(retrieved[1].content_key, archives[1].content_key);
        assert_eq!(retrieved[1].index_size, archives[1].index_size);
    }
}
