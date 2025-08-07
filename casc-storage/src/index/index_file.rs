//! Generic index file handling

use crate::error::Result;
use crate::types::{ArchiveLocation, EKey};
use std::path::Path;

/// Version of the index format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexVersion {
    V5,  // Older format
    V7,  // Current format
}

/// Generic index file interface
pub struct IndexFile {
    version: IndexVersion,
    entries: std::collections::HashMap<EKey, ArchiveLocation>,
}

impl IndexFile {
    /// Create a new empty index
    pub fn new(version: IndexVersion) -> Self {
        Self {
            version,
            entries: std::collections::HashMap::new(),
        }
    }

    /// Load an index from a file
    pub fn load(path: &Path) -> Result<Self> {
        // Determine type based on extension
        if path.extension().and_then(|s| s.to_str()) == Some("idx") {
            let parser = super::IdxParser::parse_file(path)?;
            let mut entries = std::collections::HashMap::new();
            for (ekey, location) in parser.entries() {
                entries.insert(*ekey, *location);
            }
            Ok(Self {
                version: IndexVersion::V7,
                entries,
            })
        } else if path.extension().and_then(|s| s.to_str()) == Some("index") {
            let parser = super::GroupIndex::parse_file(path)?;
            let mut entries = std::collections::HashMap::new();
            for (ekey, location) in parser.entries() {
                entries.insert(*ekey, *location);
            }
            Ok(Self {
                version: IndexVersion::V7,
                entries,
            })
        } else {
            Err(crate::error::CascError::InvalidIndexFormat(
                format!("Unknown index file extension: {:?}", path.extension())
            ))
        }
    }

    /// Look up an entry by EKey
    pub fn lookup(&self, ekey: &EKey) -> Option<&ArchiveLocation> {
        self.entries.get(ekey)
    }

    /// Add an entry to the index
    pub fn add_entry(&mut self, ekey: EKey, location: ArchiveLocation) {
        self.entries.insert(ekey, location);
    }

    /// Remove an entry from the index
    pub fn remove_entry(&mut self, ekey: &EKey) -> Option<ArchiveLocation> {
        self.entries.remove(ekey)
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the index version
    pub fn version(&self) -> IndexVersion {
        self.version
    }

    /// Iterate over all entries
    pub fn entries(&self) -> impl Iterator<Item = (&EKey, &ArchiveLocation)> {
        self.entries.iter()
    }
}