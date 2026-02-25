//! Builder for creating encoding files from scratch
//!
//! This module provides [`EncodingBuilder`] for creating NGDP encoding files from individual
//! content and encoding key entries. The builder handles:
//!
//! - `ESpec` table construction from unique compression specifications
//! - Page-based organization of `CKey` and `EKey` entries
//! - Proper sorting and indexing for binary search compatibility
//! - Page checksums and index generation
//! - Trailing `ESpec` generation for self-describing files
//! - Round-trip compatibility with the parser
//!
//! # Example
//!
//! ```rust
//! use cascette_formats::encoding::{EncodingBuilder, CKeyEntryData, EKeyEntryData};
//! use cascette_crypto::{ContentKey, EncodingKey};
//!
//! let mut builder = EncodingBuilder::new();
//!
//! // Add a content key entry
//! let content_key = ContentKey::from_bytes([1u8; 16]);
//! let encoding_key = EncodingKey::from_bytes([2u8; 16]);
//!
//! builder.add_ckey_entry(CKeyEntryData {
//!     content_key,
//!     file_size: 1024,
//!     encoding_keys: vec![encoding_key],
//! });
//!
//! // Add corresponding encoding key entry
//! builder.add_ekey_entry(EKeyEntryData {
//!     encoding_key,
//!     espec: "z".to_string(), // ZLib compression
//!     file_size: 512, // Compressed size
//! });
//!
//! // Build the encoding file
//! let encoding_file = builder.build().expect("Failed to build encoding file");
//!
//! // Serialize to bytes
//! let data = encoding_file.build().expect("Failed to serialize");
//!
//! // Or compress with BLTE
//! let blte_data = encoding_file.build_blte().expect("Failed to compress with BLTE");
//! ```

use crate::encoding::{
    ESpecTable, EncodingError, EncodingFile, EncodingHeader, IndexEntry, Page,
    entry::{CKeyPageEntry, EKeyPageEntry},
};
use binrw::BinWrite;
use cascette_crypto::{ContentKey, EncodingKey};
use std::collections::HashMap;
use std::io::Cursor;

/// Entry data for building `CKey` pages
#[derive(Debug, Clone)]
pub struct CKeyEntryData {
    /// Content key (MD5 hash of file content)
    pub content_key: ContentKey,
    /// File size (uncompressed)
    pub file_size: u64,
    /// List of encoding keys for this content
    pub encoding_keys: Vec<EncodingKey>,
}

/// Entry data for building `EKey` pages
#[derive(Debug, Clone)]
pub struct EKeyEntryData {
    /// Encoding key
    pub encoding_key: EncodingKey,
    /// `ESpec` string for compression/encoding
    pub espec: String,
    /// Compressed file size
    pub file_size: u64,
}

/// Builder for creating encoding files
#[derive(Debug, Clone)]
pub struct EncodingBuilder {
    /// `CKey` entries (sorted by content key)
    ckey_entries: Vec<CKeyEntryData>,
    /// `EKey` entries (sorted by encoding key)
    ekey_entries: Vec<EKeyEntryData>,
    /// Page sizes in KB
    ckey_page_size_kb: u16,
    ekey_page_size_kb: u16,
    /// Self-describing `ESpec` for the encoding file itself
    trailing_espec: Option<String>,
}

impl EncodingBuilder {
    /// Create a new encoding builder with default page sizes (4KB)
    pub fn new() -> Self {
        Self {
            ckey_entries: Vec::new(),
            ekey_entries: Vec::new(),
            ckey_page_size_kb: 4,
            ekey_page_size_kb: 4,
            trailing_espec: None,
        }
    }

    /// Set page sizes in KB
    #[must_use]
    pub fn with_page_sizes(mut self, ckey_page_size_kb: u16, ekey_page_size_kb: u16) -> Self {
        self.ckey_page_size_kb = ckey_page_size_kb;
        self.ekey_page_size_kb = ekey_page_size_kb;
        self
    }

    /// Set the trailing `ESpec` for the file itself
    #[must_use]
    pub fn with_trailing_espec(mut self, espec: String) -> Self {
        self.trailing_espec = Some(espec);
        self
    }

    /// Add a content key entry
    pub fn add_ckey_entry(&mut self, entry: CKeyEntryData) {
        self.ckey_entries.push(entry);
    }

    /// Add an encoding key entry
    pub fn add_ekey_entry(&mut self, entry: EKeyEntryData) {
        self.ekey_entries.push(entry);
    }

    /// Build the `ESpec` table from all `EKey` entries
    fn build_espec_table(&self) -> ESpecTable {
        let mut table = ESpecTable::default();
        let mut espec_map = HashMap::new();

        // Collect unique ESpec strings
        for entry in &self.ekey_entries {
            if !espec_map.contains_key(&entry.espec) {
                let index = table.add(entry.espec.clone());
                espec_map.insert(entry.espec.clone(), index);
            }
        }

        table
    }

    /// Build `CKey` pages from entries
    fn build_ckey_pages(
        &self,
        page_size: usize,
    ) -> Result<Vec<Page<CKeyPageEntry>>, EncodingError> {
        let mut pages = Vec::new();
        let mut current_page_entries = Vec::new();
        let mut current_page_size = 0;

        // Sort entries by content key for consistent ordering
        let mut sorted_entries: Vec<_> = self.ckey_entries.iter().collect();
        sorted_entries.sort_by_key(|entry| entry.content_key.as_bytes());

        for entry_data in sorted_entries {
            // Calculate entry size: 1 (key_count) + 5 (file_size) + 16 (content_key) + 16 * key_count (encoding_keys)
            let entry_size = 1 + 5 + 16 + (16 * entry_data.encoding_keys.len());

            // Check if adding this entry would exceed page size
            if current_page_size + entry_size > page_size && !current_page_entries.is_empty() {
                // Finalize current page
                let page_data = self.serialize_ckey_page(&current_page_entries, page_size)?;
                pages.push(Page {
                    entries: current_page_entries,
                    original_data: page_data,
                });

                current_page_entries = Vec::new();
                current_page_size = 0;
            }

            // Convert to page entry
            let page_entry = CKeyPageEntry {
                #[allow(clippy::cast_possible_truncation)]
                key_count: entry_data.encoding_keys.len() as u8,
                file_size: entry_data.file_size,
                content_key: entry_data.content_key,
                encoding_keys: entry_data.encoding_keys.clone(),
            };

            current_page_entries.push(page_entry);
            current_page_size += entry_size;
        }

        // Add final page if not empty
        if !current_page_entries.is_empty() {
            let page_data = self.serialize_ckey_page(&current_page_entries, page_size)?;
            pages.push(Page {
                entries: current_page_entries,
                original_data: page_data,
            });
        }

        Ok(pages)
    }

    /// Build `EKey` pages from entries
    fn build_ekey_pages(
        &self,
        page_size: usize,
        espec_table: &ESpecTable,
    ) -> Result<Vec<Page<EKeyPageEntry>>, EncodingError> {
        let mut pages = Vec::new();
        let mut current_page_entries = Vec::new();
        let mut current_page_size = 0;

        // Create reverse lookup for ESpec indices
        let mut espec_indices = HashMap::new();
        for (index, espec) in espec_table.entries.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            espec_indices.insert(espec, index as u32);
        }

        // Sort entries by encoding key for consistent ordering
        let mut sorted_entries: Vec<_> = self.ekey_entries.iter().collect();
        sorted_entries.sort_by_key(|entry| entry.encoding_key.as_bytes());

        for entry_data in sorted_entries {
            // Calculate entry size: 16 (encoding_key) + 4 (espec_index) + 5 (file_size)
            let entry_size = 16 + 4 + 5;

            // Check if adding this entry would exceed page size
            if current_page_size + entry_size > page_size && !current_page_entries.is_empty() {
                // Finalize current page
                let page_data = self.serialize_ekey_page(&current_page_entries, page_size)?;
                pages.push(Page {
                    entries: current_page_entries,
                    original_data: page_data,
                });

                current_page_entries = Vec::new();
                current_page_size = 0;
            }

            // Get ESpec index
            let espec_index = espec_indices
                .get(&entry_data.espec)
                .copied()
                .ok_or_else(|| EncodingError::InvalidESpec(entry_data.espec.clone()))?;

            // Convert to page entry
            let page_entry = EKeyPageEntry {
                encoding_key: entry_data.encoding_key,
                espec_index,
                file_size: entry_data.file_size,
            };

            current_page_entries.push(page_entry);
            current_page_size += entry_size;
        }

        // Add final page if not empty
        if !current_page_entries.is_empty() {
            let page_data = self.serialize_ekey_page(&current_page_entries, page_size)?;
            pages.push(Page {
                entries: current_page_entries,
                original_data: page_data,
            });
        }

        Ok(pages)
    }

    /// Serialize `CKey` page entries to binary data
    #[allow(clippy::unused_self)]
    fn serialize_ckey_page(
        &self,
        entries: &[CKeyPageEntry],
        page_size: usize,
    ) -> Result<Vec<u8>, EncodingError> {
        let mut page_data = vec![0u8; page_size];
        let mut cursor = Cursor::new(&mut page_data);

        for entry in entries {
            entry
                .write_options(&mut cursor, binrw::Endian::Big, (16, 16))
                .map_err(EncodingError::BinRw)?;
        }

        Ok(page_data)
    }

    /// Serialize `EKey` page entries to binary data
    #[allow(clippy::unused_self)]
    fn serialize_ekey_page(
        &self,
        entries: &[EKeyPageEntry],
        page_size: usize,
    ) -> Result<Vec<u8>, EncodingError> {
        let mut page_data = vec![0u8; page_size];
        let mut cursor = Cursor::new(&mut page_data);

        for entry in entries {
            entry
                .write_options(&mut cursor, binrw::Endian::Big, (16,))
                .map_err(EncodingError::BinRw)?;
        }

        Ok(page_data)
    }

    /// Build index entries for pages
    fn build_index<T>(pages: &[Page<T>]) -> Vec<IndexEntry>
    where
        T: HasFirstKey,
    {
        pages
            .iter()
            .map(|page| {
                let first_key = if let Some(first_entry) = page.entries.first() {
                    first_entry.first_key()
                } else {
                    [0u8; 16] // Empty page
                };

                let checksum = md5::compute(&page.original_data);
                IndexEntry::new(first_key, *checksum)
            })
            .collect()
    }

    /// Build the complete encoding file
    pub fn build(self) -> Result<EncodingFile, EncodingError> {
        let ckey_page_size = self.ckey_page_size_kb as usize * 1024;
        let ekey_page_size = self.ekey_page_size_kb as usize * 1024;

        // Build ESpec table first
        let espec_table = self.build_espec_table();

        // Build pages
        let ckey_pages = self.build_ckey_pages(ckey_page_size)?;
        let ekey_pages = self.build_ekey_pages(ekey_page_size, &espec_table)?;

        // Build indices
        let ckey_index = Self::build_index(&ckey_pages);
        let ekey_index = Self::build_index(&ekey_pages);

        // Create header
        let espec_data = espec_table.build();
        let header = EncodingHeader {
            magic: *b"EN",
            version: 1,
            ckey_hash_size: 16,
            ekey_hash_size: 16,
            ckey_page_size_kb: self.ckey_page_size_kb,
            ekey_page_size_kb: self.ekey_page_size_kb,
            #[allow(clippy::cast_possible_truncation)]
            ckey_page_count: ckey_pages.len() as u32,
            #[allow(clippy::cast_possible_truncation)]
            ekey_page_count: ekey_pages.len() as u32,
            flags: 0,
            #[allow(clippy::cast_possible_truncation)]
            espec_block_size: espec_data.len() as u32,
        };

        Ok(EncodingFile {
            header,
            espec_table,
            ckey_index,
            ckey_pages,
            ekey_index,
            ekey_pages,
            trailing_espec: self.trailing_espec,
        })
    }

    /// Generate trailing `ESpec` for the encoding file itself
    pub fn generate_trailing_espec(encoding_file: &EncodingFile) -> String {
        let mut sections = Vec::new();
        // Header (22 bytes) - uncompressed
        sections.push(format!("{}=n", 22));

        // ESpec table - compressed
        let espec_size = encoding_file.header.espec_block_size;
        if espec_size > 0 {
            sections.push(format!("{espec_size}=z"));
        }

        // CKey index - uncompressed
        let ckey_index_size = encoding_file.header.ckey_page_count * 32; // 16 + 16 bytes per entry
        if ckey_index_size > 0 {
            sections.push(format!("{ckey_index_size}=n"));
        }

        // CKey pages - uncompressed
        let ckey_pages_size = encoding_file.header.ckey_page_count
            * (u32::from(encoding_file.header.ckey_page_size_kb) * 1024);
        if ckey_pages_size > 0 {
            sections.push(format!("{ckey_pages_size}=n"));
        }

        // EKey index - uncompressed
        let ekey_index_size = encoding_file.header.ekey_page_count * 32;
        if ekey_index_size > 0 {
            sections.push(format!("{ekey_index_size}=n"));
        }

        // EKey pages - uncompressed
        let ekey_pages_size = encoding_file.header.ekey_page_count
            * (u32::from(encoding_file.header.ekey_page_size_kb) * 1024);
        if ekey_pages_size > 0 {
            sections.push(format!("{ekey_pages_size}=n"));
        }

        // Remainder (the trailing ESpec itself) - compressed
        sections.push("*=z".to_string());

        format!("b:{{{}}}", sections.join(","))
    }
}

impl Default for EncodingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EncodingBuilder {
    /// Create builder from existing encoding file (for modification)
    ///
    /// This allows loading an existing encoding file, modifying its contents,
    /// and rebuilding it with the changes applied.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cascette_formats::encoding::{EncodingBuilder, EncodingFile, CKeyEntryData};
    /// use cascette_crypto::{ContentKey, EncodingKey};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Load existing encoding file
    /// let data = std::fs::read("encoding.bin")?;
    /// let encoding_file = EncodingFile::parse(&data)?;
    ///
    /// // Convert to builder for modification
    /// let mut builder = EncodingBuilder::from_encoding_file(&encoding_file);
    ///
    /// // Add new entry
    /// let content_key = ContentKey::from_bytes([1u8; 16]);
    /// let encoding_key = EncodingKey::from_bytes([2u8; 16]);
    /// builder.add_ckey_entry(CKeyEntryData {
    ///     content_key,
    ///     file_size: 1024,
    ///     encoding_keys: vec![encoding_key],
    /// });
    ///
    /// // Rebuild
    /// let modified = builder.build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_encoding_file(encoding_file: &EncodingFile) -> Self {
        let mut builder = Self::new().with_page_sizes(
            encoding_file.header.ckey_page_size_kb,
            encoding_file.header.ekey_page_size_kb,
        );

        // Copy trailing espec if present
        if let Some(ref trailing) = encoding_file.trailing_espec {
            builder = builder.with_trailing_espec(trailing.clone());
        }

        // Extract CKey entries from pages
        for page in &encoding_file.ckey_pages {
            for entry in &page.entries {
                builder.add_ckey_entry(CKeyEntryData {
                    content_key: entry.content_key,
                    file_size: entry.file_size,
                    encoding_keys: entry.encoding_keys.clone(),
                });
            }
        }

        // Extract EKey entries from pages
        for page in &encoding_file.ekey_pages {
            for entry in &page.entries {
                // Look up ESpec string from index
                let espec = encoding_file
                    .espec_table
                    .get(entry.espec_index)
                    .unwrap_or("z")
                    .to_string();

                builder.add_ekey_entry(EKeyEntryData {
                    encoding_key: entry.encoding_key,
                    espec,
                    file_size: entry.file_size,
                });
            }
        }

        builder
    }

    /// Remove a CKey entry by content key
    ///
    /// Returns true if an entry was removed, false if no matching entry was found.
    pub fn remove_ckey_entry(&mut self, content_key: &ContentKey) -> bool {
        let original_len = self.ckey_entries.len();
        self.ckey_entries.retain(|e| e.content_key != *content_key);
        self.ckey_entries.len() < original_len
    }

    /// Remove an EKey entry by encoding key
    ///
    /// Returns true if an entry was removed, false if no matching entry was found.
    pub fn remove_ekey_entry(&mut self, encoding_key: &EncodingKey) -> bool {
        let original_len = self.ekey_entries.len();
        self.ekey_entries
            .retain(|e| e.encoding_key != *encoding_key);
        self.ekey_entries.len() < original_len
    }

    /// Check if a CKey entry exists
    pub fn has_ckey_entry(&self, content_key: &ContentKey) -> bool {
        self.ckey_entries
            .iter()
            .any(|e| e.content_key == *content_key)
    }

    /// Check if an EKey entry exists
    pub fn has_ekey_entry(&self, encoding_key: &EncodingKey) -> bool {
        self.ekey_entries
            .iter()
            .any(|e| e.encoding_key == *encoding_key)
    }

    /// Get the number of CKey entries
    pub fn ckey_count(&self) -> usize {
        self.ckey_entries.len()
    }

    /// Get the number of EKey entries
    pub fn ekey_count(&self) -> usize {
        self.ekey_entries.len()
    }

    /// Clear all entries (useful for rebuilding from scratch)
    pub fn clear(&mut self) {
        self.ckey_entries.clear();
        self.ekey_entries.clear();
    }
}

/// Trait for types that have a first key for indexing
trait HasFirstKey {
    fn first_key(&self) -> [u8; 16];
}

impl HasFirstKey for CKeyPageEntry {
    fn first_key(&self) -> [u8; 16] {
        *self.content_key.as_bytes()
    }
}

impl HasFirstKey for EKeyPageEntry {
    fn first_key(&self) -> [u8; 16] {
        *self.encoding_key.as_bytes()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::{ContentKey, EncodingKey};

    #[test]
    fn test_builder_basic() {
        let mut builder = EncodingBuilder::new();

        // Add a simple CKey entry
        let content_key = ContentKey::from_bytes([1u8; 16]);
        let encoding_key = EncodingKey::from_bytes([2u8; 16]);

        builder.add_ckey_entry(CKeyEntryData {
            content_key,
            file_size: 1024,
            encoding_keys: vec![encoding_key],
        });

        builder.add_ekey_entry(EKeyEntryData {
            encoding_key,
            espec: "z".to_string(),
            file_size: 512,
        });

        let encoding_file = builder.build().expect("Failed to build encoding file");

        // Verify basic structure
        assert_eq!(encoding_file.header.magic, *b"EN");
        assert_eq!(encoding_file.header.version, 1);
        assert_eq!(encoding_file.ckey_pages.len(), 1);
        assert_eq!(encoding_file.ekey_pages.len(), 1);
        assert_eq!(encoding_file.espec_table.entries.len(), 1);
        assert_eq!(encoding_file.espec_table.entries[0], "z");
    }

    #[test]
    fn test_round_trip_compatibility() {
        let mut builder = EncodingBuilder::new();

        // Add test data
        for i in 0..10 {
            let content_key = ContentKey::from_bytes([i; 16]);
            let encoding_key = EncodingKey::from_bytes([i + 100; 16]);

            builder.add_ckey_entry(CKeyEntryData {
                content_key,
                file_size: 1024 * (u64::from(i) + 1),
                encoding_keys: vec![encoding_key],
            });

            builder.add_ekey_entry(EKeyEntryData {
                encoding_key,
                espec: if i % 2 == 0 {
                    "z".to_string()
                } else {
                    "n".to_string()
                },
                file_size: 512 * (u64::from(i) + 1),
            });
        }

        // Build and serialize
        let encoding_file = builder.build().expect("Failed to build encoding file");
        let serialized = encoding_file
            .build()
            .expect("Failed to serialize encoding file");

        // Parse back
        let parsed = EncodingFile::parse(&serialized).expect("Failed to parse encoding file");

        // Verify key counts match
        assert_eq!(encoding_file.ckey_count(), parsed.ckey_count());
        assert_eq!(encoding_file.ekey_count(), parsed.ekey_count());
        assert_eq!(
            encoding_file.espec_table.entries.len(),
            parsed.espec_table.entries.len()
        );
    }

    #[test]
    fn test_trailing_espec_generation() {
        let mut builder = EncodingBuilder::new();

        // Add minimal data
        let content_key = ContentKey::from_bytes([1u8; 16]);
        let encoding_key = EncodingKey::from_bytes([2u8; 16]);

        builder.add_ckey_entry(CKeyEntryData {
            content_key,
            file_size: 1024,
            encoding_keys: vec![encoding_key],
        });

        builder.add_ekey_entry(EKeyEntryData {
            encoding_key,
            espec: "z".to_string(),
            file_size: 512,
        });

        let encoding_file = builder.build().expect("Failed to build encoding file");
        let trailing_espec = EncodingBuilder::generate_trailing_espec(&encoding_file);

        // Verify format matches expected pattern
        assert!(trailing_espec.starts_with("b:{22=n"));
        assert!(trailing_espec.contains("=z"));
        assert!(trailing_espec.contains("=n"));
        assert!(trailing_espec.ends_with("*=z}"));
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_builder_with_complex_data() {
        let mut builder = EncodingBuilder::new();

        // Add multiple entries with various configurations
        for i in 0..50 {
            let content_key = ContentKey::from_bytes({
                let mut bytes = [0u8; 16];
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    bytes[0] = i as u8;
                    bytes[1] = (i >> 8) as u8;
                }
                bytes
            });

            let encoding_key1 = EncodingKey::from_bytes({
                let mut bytes = [0u8; 16];
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    bytes[0] = (i + 100) as u8;
                    bytes[1] = ((i + 100) >> 8) as u8;
                }
                bytes
            });

            let encoding_key2 = EncodingKey::from_bytes({
                let mut bytes = [0u8; 16];
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    bytes[0] = (i + 200) as u8;
                    bytes[1] = ((i + 200) >> 8) as u8;
                }
                bytes
            });

            // Some entries have multiple encoding keys
            let encoding_keys = if i % 3 == 0 {
                vec![encoding_key1, encoding_key2]
            } else {
                vec![encoding_key1]
            };

            builder.add_ckey_entry(CKeyEntryData {
                content_key,
                #[allow(clippy::cast_sign_loss)]
                file_size: 1024 * (i as u64 + 1),
                encoding_keys: encoding_keys.clone(),
            });

            // Add corresponding EKey entries
            for encoding_key in encoding_keys {
                let espec = match i % 4 {
                    0 => "z".to_string(),
                    1 => "n".to_string(),
                    2 => "b:{0,1000},z".to_string(),
                    3 => "z,b:{1000,500}".to_string(),
                    _ => unreachable!(),
                };

                builder.add_ekey_entry(EKeyEntryData {
                    encoding_key,
                    espec,
                    #[allow(clippy::cast_sign_loss)]
                    file_size: 512 * (i as u64 + 1),
                });
            }
        }

        // Build and test
        let encoding_file = builder.build().expect("Failed to build encoding file");

        // Verify structure
        assert_eq!(encoding_file.header.magic, *b"EN");
        assert_eq!(encoding_file.header.version, 1);
        assert!(encoding_file.ckey_count() > 0);
        assert!(encoding_file.ekey_count() > 0);
        assert!(!encoding_file.espec_table.entries.is_empty());

        // Test serialization/deserialization
        let serialized = encoding_file.build().expect("Failed to serialize");
        let parsed = EncodingFile::parse(&serialized).expect("Failed to parse");

        // Verify round-trip consistency
        assert_eq!(encoding_file.ckey_count(), parsed.ckey_count());
        assert_eq!(encoding_file.ekey_count(), parsed.ekey_count());
        assert_eq!(
            encoding_file.espec_table.entries.len(),
            parsed.espec_table.entries.len()
        );

        // Test with trailing ESpec
        let trailing_espec = EncodingBuilder::generate_trailing_espec(&encoding_file);
        let builder_with_trailing = EncodingBuilder::new().with_trailing_espec(trailing_espec);
        let file_with_trailing = builder_with_trailing
            .build()
            .expect("Failed to build with trailing ESpec");
        assert!(file_with_trailing.trailing_espec.is_some());
    }

    #[test]
    fn test_builder_blte_compression() {
        let mut builder = EncodingBuilder::new();

        let content_key = ContentKey::from_bytes([1u8; 16]);
        let encoding_key = EncodingKey::from_bytes([2u8; 16]);

        builder.add_ckey_entry(CKeyEntryData {
            content_key,
            file_size: 1024,
            encoding_keys: vec![encoding_key],
        });

        builder.add_ekey_entry(EKeyEntryData {
            encoding_key,
            espec: "z".to_string(),
            file_size: 512,
        });

        let encoding_file = builder.build().expect("Failed to build encoding file");

        // Test BLTE compression
        let blte_compressed = encoding_file
            .build_blte()
            .expect("Failed to compress with BLTE");

        // Verify we can decompress it back
        let decompressed =
            EncodingFile::parse_blte(&blte_compressed).expect("Failed to decompress BLTE");

        assert_eq!(encoding_file.ckey_count(), decompressed.ckey_count());
        assert_eq!(encoding_file.ekey_count(), decompressed.ekey_count());
        assert_eq!(
            encoding_file.espec_table.entries.len(),
            decompressed.espec_table.entries.len()
        );
    }

    #[test]
    fn test_from_encoding_file() {
        // Build original encoding file
        let mut builder = EncodingBuilder::new();

        for i in 0..5 {
            let content_key = ContentKey::from_bytes([i; 16]);
            let encoding_key = EncodingKey::from_bytes([i + 100; 16]);

            builder.add_ckey_entry(CKeyEntryData {
                content_key,
                file_size: 1024 * (u64::from(i) + 1),
                encoding_keys: vec![encoding_key],
            });

            builder.add_ekey_entry(EKeyEntryData {
                encoding_key,
                espec: "z".to_string(),
                file_size: 512 * (u64::from(i) + 1),
            });
        }

        let original = builder.build().expect("Failed to build original");
        let serialized = original.build().expect("Failed to serialize");
        let parsed = EncodingFile::parse(&serialized).expect("Failed to parse");

        // Create builder from parsed file
        let mut rebuilt_builder = EncodingBuilder::from_encoding_file(&parsed);

        // Verify entry counts match
        assert_eq!(rebuilt_builder.ckey_count(), 5);
        assert_eq!(rebuilt_builder.ekey_count(), 5);

        // Add a new entry
        let new_ckey = ContentKey::from_bytes([200u8; 16]);
        let new_ekey = EncodingKey::from_bytes([201u8; 16]);
        rebuilt_builder.add_ckey_entry(CKeyEntryData {
            content_key: new_ckey,
            file_size: 9999,
            encoding_keys: vec![new_ekey],
        });
        rebuilt_builder.add_ekey_entry(EKeyEntryData {
            encoding_key: new_ekey,
            espec: "n".to_string(),
            file_size: 8888,
        });

        // Verify new entry was added
        assert_eq!(rebuilt_builder.ckey_count(), 6);
        assert_eq!(rebuilt_builder.ekey_count(), 6);

        // Build modified file
        let modified = rebuilt_builder.build().expect("Failed to build modified");
        assert_eq!(modified.ckey_count(), 6);
        assert_eq!(modified.ekey_count(), 6);

        // Verify new entry can be found
        assert!(modified.find_encoding(&new_ckey).is_some());
    }

    #[test]
    fn test_remove_entries() {
        let mut builder = EncodingBuilder::new();

        let ckey1 = ContentKey::from_bytes([1u8; 16]);
        let ckey2 = ContentKey::from_bytes([2u8; 16]);
        let ekey1 = EncodingKey::from_bytes([101u8; 16]);
        let ekey2 = EncodingKey::from_bytes([102u8; 16]);

        builder.add_ckey_entry(CKeyEntryData {
            content_key: ckey1,
            file_size: 1024,
            encoding_keys: vec![ekey1],
        });
        builder.add_ckey_entry(CKeyEntryData {
            content_key: ckey2,
            file_size: 2048,
            encoding_keys: vec![ekey2],
        });
        builder.add_ekey_entry(EKeyEntryData {
            encoding_key: ekey1,
            espec: "z".to_string(),
            file_size: 512,
        });
        builder.add_ekey_entry(EKeyEntryData {
            encoding_key: ekey2,
            espec: "n".to_string(),
            file_size: 1024,
        });

        assert_eq!(builder.ckey_count(), 2);
        assert_eq!(builder.ekey_count(), 2);
        assert!(builder.has_ckey_entry(&ckey1));
        assert!(builder.has_ekey_entry(&ekey1));

        // Remove first entries
        assert!(builder.remove_ckey_entry(&ckey1));
        assert!(builder.remove_ekey_entry(&ekey1));

        assert_eq!(builder.ckey_count(), 1);
        assert_eq!(builder.ekey_count(), 1);
        assert!(!builder.has_ckey_entry(&ckey1));
        assert!(!builder.has_ekey_entry(&ekey1));
        assert!(builder.has_ckey_entry(&ckey2));
        assert!(builder.has_ekey_entry(&ekey2));

        // Try to remove non-existent entry
        assert!(!builder.remove_ckey_entry(&ckey1));

        // Build should still work
        let encoding_file = builder.build().expect("Failed to build");
        assert_eq!(encoding_file.ckey_count(), 1);
        assert_eq!(encoding_file.ekey_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut builder = EncodingBuilder::new();

        let ckey = ContentKey::from_bytes([1u8; 16]);
        let ekey = EncodingKey::from_bytes([2u8; 16]);

        builder.add_ckey_entry(CKeyEntryData {
            content_key: ckey,
            file_size: 1024,
            encoding_keys: vec![ekey],
        });
        builder.add_ekey_entry(EKeyEntryData {
            encoding_key: ekey,
            espec: "z".to_string(),
            file_size: 512,
        });

        assert_eq!(builder.ckey_count(), 1);
        assert_eq!(builder.ekey_count(), 1);

        builder.clear();

        assert_eq!(builder.ckey_count(), 0);
        assert_eq!(builder.ekey_count(), 0);
    }
}
