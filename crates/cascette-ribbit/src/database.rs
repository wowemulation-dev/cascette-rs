//! Build database management.
//!
//! Loads and indexes game build metadata from JSON files for efficient querying.

use crate::error::DatabaseError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::SystemTime;

/// A single game build record with all metadata.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BuildRecord {
    /// Unique build identifier (monotonically increasing)
    pub id: u64,

    /// Product code (e.g., "wow", "`wow_classic`", "`wow_classic_era`")
    pub product: String,

    /// Full version string (e.g., "1.13.2.32600")
    pub version: String,

    /// Build number only (e.g., "32600")
    pub build: String,

    /// Build configuration MD5 hash (32 hex characters)
    pub build_config: String,

    /// CDN configuration MD5 hash (32 hex characters)
    pub cdn_config: String,

    /// `KeyRing` MD5 hash (32 hex characters, nullable)
    pub keyring: Option<String>,

    /// Product configuration MD5 hash (32 hex characters, nullable)
    pub product_config: Option<String>,

    /// ISO 8601 timestamp of build creation
    pub build_time: String,

    /// Encoding file content key (32 hex characters)
    pub encoding_ekey: String,

    /// Root file content key (32 hex characters)
    pub root_ekey: String,

    /// Install file content key (32 hex characters)
    pub install_ekey: String,

    /// Download file content key (32 hex characters)
    pub download_ekey: String,

    /// Optional product-specific CDN path override (e.g., "tpr/wow")
    /// If None, server uses default path from CLI configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdn_path: Option<String>,
}

impl BuildRecord {
    /// Validate all fields in the build record.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::InvalidField` if any field is invalid.
    pub fn validate(&self) -> Result<(), DatabaseError> {
        // Validate product name
        if self.product.is_empty() {
            return Err(DatabaseError::InvalidField {
                field: "product".to_string(),
                build_id: self.id,
                reason: "product name cannot be empty".to_string(),
            });
        }

        // Validate version and build
        if self.version.is_empty() {
            return Err(DatabaseError::InvalidField {
                field: "version".to_string(),
                build_id: self.id,
                reason: "version cannot be empty".to_string(),
            });
        }

        if self.build.is_empty() {
            return Err(DatabaseError::InvalidField {
                field: "build".to_string(),
                build_id: self.id,
                reason: "build cannot be empty".to_string(),
            });
        }

        // Validate MD5 hashes (32 hex characters)
        self.validate_hash("build_config", &self.build_config)?;
        self.validate_hash("cdn_config", &self.cdn_config)?;
        if let Some(ref config) = self.product_config {
            self.validate_hash("product_config", config)?;
        }

        // Validate content keys (32 hex characters)
        self.validate_hash("encoding_ekey", &self.encoding_ekey)?;
        self.validate_hash("root_ekey", &self.root_ekey)?;
        self.validate_hash("install_ekey", &self.install_ekey)?;
        self.validate_hash("download_ekey", &self.download_ekey)?;

        // Validate ISO 8601 timestamp format (basic check)
        if !self.build_time.contains('T') || !self.build_time.contains(':') {
            return Err(DatabaseError::InvalidField {
                field: "build_time".to_string(),
                build_id: self.id,
                reason: format!(
                    "invalid ISO 8601 format: '{}' (expected format: '2019-11-21T18:33:35+00:00')",
                    self.build_time
                ),
            });
        }

        Ok(())
    }

    /// Validate that a hash string is exactly 32 hex characters.
    fn validate_hash(&self, field: &str, value: &str) -> Result<(), DatabaseError> {
        if value.len() != 32 {
            return Err(DatabaseError::InvalidField {
                field: field.to_string(),
                build_id: self.id,
                reason: format!("expected 32 hex characters, got {} characters", value.len()),
            });
        }

        if !value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DatabaseError::InvalidField {
                field: field.to_string(),
                build_id: self.id,
                reason: "contains non-hexadecimal characters".to_string(),
            });
        }

        Ok(())
    }
}

/// In-memory database of builds, indexed by product.
#[derive(Debug, Clone)]
pub struct BuildDatabase {
    /// Builds indexed by product name
    builds_by_product: HashMap<String, Vec<BuildRecord>>,

    /// Total number of builds
    total_builds: usize,

    /// Timestamp when database was loaded
    loaded_at: SystemTime,
}

impl BuildDatabase {
    /// Load build database from JSON file.
    ///
    /// The file should contain a JSON array of `BuildRecord` objects.
    /// Builds are automatically indexed by product and sorted by `build_time` (newest first).
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError` if:
    /// - File cannot be read
    /// - JSON is malformed
    /// - Database is empty
    /// - Any build record fails validation
    pub fn from_file(path: &Path) -> Result<Self, DatabaseError> {
        let file = File::open(path).map_err(|source| DatabaseError::LoadFailed {
            path: path.to_path_buf(),
            source,
        })?;

        let reader = BufReader::new(file);
        let builds: Vec<BuildRecord> = serde_json::from_reader(reader)?;

        if builds.is_empty() {
            return Err(DatabaseError::EmptyDatabase);
        }

        // Validate all builds
        for build in &builds {
            build.validate()?;
        }

        // Index builds by product
        let mut builds_by_product: HashMap<String, Vec<BuildRecord>> = HashMap::new();
        for build in builds {
            builds_by_product
                .entry(build.product.clone())
                .or_default()
                .push(build);
        }

        // Sort each product's builds by build_time (newest first)
        for builds in builds_by_product.values_mut() {
            builds.sort_by(|a, b| b.build_time.cmp(&a.build_time));
        }

        let total_builds = builds_by_product.values().map(Vec::len).sum();

        Ok(Self {
            builds_by_product,
            total_builds,
            loaded_at: SystemTime::now(),
        })
    }

    /// Get the latest build for a product.
    ///
    /// Returns None if the product doesn't exist.
    #[must_use]
    pub fn latest_build(&self, product: &str) -> Option<&BuildRecord> {
        self.builds_by_product
            .get(product)
            .and_then(|builds| builds.first())
    }

    /// Get all product names in the database.
    pub fn products(&self) -> Vec<&str> {
        self.builds_by_product.keys().map(String::as_str).collect()
    }

    /// Get total number of builds loaded.
    #[must_use]
    pub const fn total_builds(&self) -> usize {
        self.total_builds
    }

    /// Get timestamp when database was loaded.
    #[must_use]
    pub const fn loaded_at(&self) -> SystemTime {
        self.loaded_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_build() -> BuildRecord {
        BuildRecord {
            id: 1,
            product: "test_product".to_string(),
            version: "1.0.0.1".to_string(),
            build: "1".to_string(),
            build_config: "0123456789abcdef0123456789abcdef".to_string(),
            cdn_config: "fedcba9876543210fedcba9876543210".to_string(),
            keyring: None,
            product_config: None,
            build_time: "2024-01-01T00:00:00+00:00".to_string(),
            encoding_ekey: "aaaabbbbccccddddeeeeffffaaaaffff".to_string(),
            root_ekey: "bbbbccccddddeeeeffffaaaabbbbcccc".to_string(),
            install_ekey: "ccccddddeeeeffffaaaabbbbccccdddd".to_string(),
            download_ekey: "ddddeeeeffffaaaabbbbccccddddeeee".to_string(),
            cdn_path: None,
        }
    }

    #[test]
    fn test_build_record_validation() {
        let build = create_test_build();
        assert!(build.validate().is_ok());
    }

    #[test]
    fn test_invalid_hash_length() {
        let mut build = create_test_build();
        build.build_config = "invalid".to_string();
        let err = build.validate().unwrap_err();
        assert!(matches!(err, DatabaseError::InvalidField { .. }));
    }

    #[test]
    fn test_invalid_hash_characters() {
        let mut build = create_test_build();
        build.cdn_config = "gggggggggggggggggggggggggggggggg".to_string();
        let err = build.validate().unwrap_err();
        assert!(matches!(err, DatabaseError::InvalidField { .. }));
    }

    #[test]
    fn test_database_from_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let build = create_test_build();
        let json = serde_json::to_string(&vec![build]).unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();

        let db = BuildDatabase::from_file(temp_file.path()).unwrap();
        assert_eq!(db.total_builds(), 1);
        assert_eq!(db.products().len(), 1);
        assert!(db.latest_build("test_product").is_some());
    }

    #[test]
    fn test_database_empty_error() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"[]").unwrap();

        let err = BuildDatabase::from_file(temp_file.path()).unwrap_err();
        assert!(matches!(err, DatabaseError::EmptyDatabase));
    }
}
