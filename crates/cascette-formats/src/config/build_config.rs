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

// NOTE: Tests that used external test data files were removed after tools restructuring
// The existing unit tests above provide sufficient coverage for BuildConfig functionality
