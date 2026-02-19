//! `.build.info` parser for installation metadata.
//!
//! The `.build.info` file is a BPSV file at the installation root
//! that contains product identification and build configuration
//! references. It determines which product
//! is installed, its build/CDN config hashes, and CDN endpoints.
//!
//! Known columns (from real WoW installations):
//! - `Branch!STRING:0` -- branch name (e.g., "us")
//! - `Active!DEC:1` -- whether this entry is active (1 = active)
//! - `Build Key!HEX:16` -- build config hash
//! - `CDN Key!HEX:16` -- CDN config hash
//! - `Install Key!HEX:16` -- install manifest key
//! - `IM Size!DEC:4` -- install manifest size
//! - `CDN Path!STRING:0` -- CDN path prefix
//! - `CDN Hosts!STRING:0` -- space-separated CDN hostnames
//! - `CDN Servers!STRING:0` -- space-separated CDN server URLs
//! - `Tags!STRING:0` -- tag string (e.g., platform, architecture)
//! - `Armadillo!STRING:0` -- Armadillo key (DRM)
//! - `Last Activated!STRING:0` -- timestamp
//! - `Version!STRING:0` -- build version string
//! - `Product!STRING:0` -- product code (e.g., "wow", "wow_classic")

use std::path::Path;

use cascette_formats::bpsv::{BpsvDocument, parse};

use crate::{Result, StorageError};

/// Parsed `.build.info` file.
///
/// Wraps a BPSV document and provides typed access to known columns.
pub struct BuildInfoFile {
    /// Underlying BPSV document.
    document: BpsvDocument,
}

impl BuildInfoFile {
    /// Parse a `.build.info` file from its contents.
    pub fn parse_str(content: &str) -> Result<Self> {
        let document = parse(content).map_err(|e| {
            StorageError::InvalidFormat(format!("failed to parse .build.info: {e}"))
        })?;

        Ok(Self { document })
    }

    /// Read and parse a `.build.info` file from disk.
    pub async fn from_path(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            StorageError::Io(std::io::Error::other(format!(
                "failed to read .build.info at {}: {e}",
                path.display()
            )))
        })?;

        Self::parse_str(&content)
    }

    /// Get the number of entries (rows) in the file.
    pub fn entry_count(&self) -> usize {
        self.document.row_count()
    }

    /// Get the active entry (first row with `Active` == 1).
    ///
    /// Most installations have a single active entry.
    pub fn active_entry(&self) -> Option<BuildInfoEntry<'_>> {
        let schema = self.document.schema();
        for row in self.document.rows() {
            if let Some(active) = row.get_raw_by_name("Active", schema)
                && active == "1"
            {
                return Some(BuildInfoEntry { row, schema });
            }
        }
        None
    }

    /// Get all entries.
    pub fn entries(&self) -> Vec<BuildInfoEntry<'_>> {
        let schema = self.document.schema();
        self.document
            .rows()
            .iter()
            .map(|row| BuildInfoEntry { row, schema })
            .collect()
    }

    /// Get the underlying BPSV document.
    pub fn document(&self) -> &BpsvDocument {
        &self.document
    }

    /// Check if a column exists.
    pub fn has_column(&self, name: &str) -> bool {
        self.document.has_field(name)
    }
}

/// A single entry (row) in a `.build.info` file.
pub struct BuildInfoEntry<'a> {
    row: &'a cascette_formats::bpsv::BpsvRow,
    schema: &'a cascette_formats::bpsv::BpsvSchema,
}

impl BuildInfoEntry<'_> {
    /// Get the branch name (e.g., "us").
    pub fn branch(&self) -> Option<&str> {
        self.row.get_raw_by_name("Branch", self.schema)
    }

    /// Check if this entry is active.
    pub fn is_active(&self) -> bool {
        self.row
            .get_raw_by_name("Active", self.schema)
            .is_some_and(|v| v == "1")
    }

    /// Get the build config hash (hex string).
    pub fn build_key(&self) -> Option<&str> {
        self.row.get_raw_by_name("Build Key", self.schema)
    }

    /// Get the CDN config hash (hex string).
    pub fn cdn_key(&self) -> Option<&str> {
        self.row.get_raw_by_name("CDN Key", self.schema)
    }

    /// Get the install manifest key (hex string).
    pub fn install_key(&self) -> Option<&str> {
        self.row.get_raw_by_name("Install Key", self.schema)
    }

    /// Get the install manifest size.
    pub fn install_size(&self) -> Option<u64> {
        self.row
            .get_raw_by_name("IM Size", self.schema)
            .and_then(|s| s.parse().ok())
    }

    /// Get the CDN path prefix.
    pub fn cdn_path(&self) -> Option<&str> {
        self.row.get_raw_by_name("CDN Path", self.schema)
    }

    /// Get CDN hostnames (space-separated in the file).
    pub fn cdn_hosts(&self) -> Vec<&str> {
        self.row
            .get_raw_by_name("CDN Hosts", self.schema)
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// Get CDN server URLs (space-separated in the file).
    pub fn cdn_servers(&self) -> Vec<&str> {
        self.row
            .get_raw_by_name("CDN Servers", self.schema)
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// Get the tags string.
    pub fn tags(&self) -> Option<&str> {
        self.row.get_raw_by_name("Tags", self.schema)
    }

    /// Get the Armadillo key.
    pub fn armadillo(&self) -> Option<&str> {
        self.row.get_raw_by_name("Armadillo", self.schema)
    }

    /// Get the last activated timestamp.
    pub fn last_activated(&self) -> Option<&str> {
        self.row.get_raw_by_name("Last Activated", self.schema)
    }

    /// Get the version string.
    pub fn version(&self) -> Option<&str> {
        self.row.get_raw_by_name("Version", self.schema)
    }

    /// Get the product code (e.g., "wow", "wow_classic").
    pub fn product(&self) -> Option<&str> {
        self.row.get_raw_by_name("Product", self.schema)
    }

    /// Get a raw field value by column name.
    pub fn get_raw(&self, name: &str) -> Option<&str> {
        self.row.get_raw_by_name(name, self.schema)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const SAMPLE_BUILD_INFO: &str = "\
Branch!STRING:0|Active!DEC:1|Build Key!HEX:16|CDN Key!HEX:16|Install Key!HEX:16|IM Size!DEC:4|CDN Path!STRING:0|CDN Hosts!STRING:0|CDN Servers!STRING:0|Tags!STRING:0|Armadillo!STRING:0|Last Activated!STRING:0|Version!STRING:0|Product!STRING:0
us|1|abcdef1234567890abcdef1234567890|fedcba0987654321fedcba0987654321|11111111111111111111111111111111|12345|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://us.patch.battle.net:1119/tpr/wow http://blzddist1-a.akamaihd.net/tpr/wow|Windows x86_64||2024-01-01T00:00:00Z|11.0.7.12345|wow
eu|0|abcdef1234567890abcdef1234567890|fedcba0987654321fedcba0987654321|11111111111111111111111111111111|12345|tpr/wow|blzddist1-a.akamaihd.net|http://eu.patch.battle.net:1119/tpr/wow|Windows x86_64|||11.0.7.12345|wow";

    #[test]
    fn test_parse_build_info() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        assert_eq!(info.entry_count(), 2);
        assert!(info.has_column("Build Key"));
        assert!(info.has_column("Product"));
    }

    #[test]
    fn test_active_entry() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        let active = info.active_entry().expect("active entry");

        assert_eq!(active.branch(), Some("us"));
        assert!(active.is_active());
        assert_eq!(active.build_key(), Some("abcdef1234567890abcdef1234567890"));
        assert_eq!(active.cdn_key(), Some("fedcba0987654321fedcba0987654321"));
        assert_eq!(active.version(), Some("11.0.7.12345"));
        assert_eq!(active.product(), Some("wow"));
    }

    #[test]
    fn test_cdn_hosts() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        let active = info.active_entry().expect("active entry");

        let hosts = active.cdn_hosts();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0], "blzddist1-a.akamaihd.net");
        assert_eq!(hosts[1], "level3.blizzard.com");
    }

    #[test]
    fn test_cdn_servers() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        let active = info.active_entry().expect("active entry");

        let servers = active.cdn_servers();
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn test_install_size() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        let active = info.active_entry().expect("active entry");
        assert_eq!(active.install_size(), Some(12345));
    }

    #[test]
    fn test_all_entries() {
        let info = BuildInfoFile::parse_str(SAMPLE_BUILD_INFO).expect("parse");
        let entries = info.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].branch(), Some("us"));
        assert_eq!(entries[1].branch(), Some("eu"));
        assert!(!entries[1].is_active());
    }

    #[test]
    fn test_empty_build_info() {
        let content = "Branch!STRING:0|Active!DEC:1\n";
        let info = BuildInfoFile::parse_str(content).expect("parse");
        assert_eq!(info.entry_count(), 0);
        assert!(info.active_entry().is_none());
    }

    #[test]
    fn test_missing_column() {
        let content = "Branch!STRING:0\nus\n";
        let info = BuildInfoFile::parse_str(content).expect("parse");
        assert!(!info.has_column("Build Key"));

        let entry = &info.entries()[0];
        assert_eq!(entry.build_key(), None);
        assert_eq!(entry.product(), None);
    }
}
