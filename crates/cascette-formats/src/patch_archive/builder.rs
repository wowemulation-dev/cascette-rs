//! Patch Archive builder implementation

use crate::patch_archive::{
    PatchArchive, PatchArchiveHeader, PatchEntry,
    error::PatchArchiveResult,
    header::{STANDARD_BLOCK_SIZE_BITS, STANDARD_KEY_SIZE},
};
use binrw::BinWrite;
use binrw::io::Cursor;

/// Builder for creating Patch Archives
pub struct PatchArchiveBuilder {
    /// Format version
    version: u8,
    /// Block size bits
    block_size_bits: u8,
    /// Patch entries to include
    entries: Vec<PatchEntry>,
}

impl PatchArchiveBuilder {
    /// Create new builder with default settings
    pub fn new() -> Self {
        Self {
            version: 2,
            block_size_bits: STANDARD_BLOCK_SIZE_BITS,
            entries: Vec::new(),
        }
    }

    /// Set format version
    pub fn version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    /// Set block size bits
    pub fn block_size_bits(mut self, bits: u8) -> Self {
        self.block_size_bits = bits;
        self
    }

    /// Add a patch entry
    pub fn add_patch(
        &mut self,
        old_key: [u8; 16],
        new_key: [u8; 16],
        patch_key: [u8; 16],
        compression_info: String,
    ) {
        self.entries.push(PatchEntry::new(
            old_key,
            new_key,
            patch_key,
            compression_info,
        ));
    }

    /// Add a patch entry with additional data
    pub fn add_patch_with_data(
        &mut self,
        old_key: [u8; 16],
        new_key: [u8; 16],
        patch_key: [u8; 16],
        compression_info: String,
        additional_data: Vec<u8>,
    ) {
        let mut entry = PatchEntry::new(old_key, new_key, patch_key, compression_info);
        entry.additional_data = additional_data;
        self.entries.push(entry);
    }

    /// Add an existing patch entry
    pub fn add_entry(mut self, entry: PatchEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Get current number of entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Build the patch archive as binary data
    pub fn build(&self) -> PatchArchiveResult<Vec<u8>> {
        // Create header
        let mut header = PatchArchiveHeader::new(self.entries.len() as u16);
        header.version = self.version;
        header.block_size_bits = self.block_size_bits;

        // Validate header
        header.validate()?;

        // Create archive
        let archive = PatchArchive {
            header,
            entries: self.entries.clone(),
        };

        // Serialize to bytes
        let mut output = Vec::new();
        archive.write_options(&mut Cursor::new(&mut output), binrw::Endian::Big, ())?;

        Ok(output)
    }

    /// Build the patch archive object
    pub fn build_archive(&self) -> PatchArchiveResult<PatchArchive> {
        let mut header = PatchArchiveHeader::new(self.entries.len() as u16);
        header.version = self.version;
        header.block_size_bits = self.block_size_bits;

        header.validate()?;

        Ok(PatchArchive {
            header,
            entries: self.entries.clone(),
        })
    }

    /// Calculate total serialized size
    pub fn calculate_size(&self) -> usize {
        let header_size = 10; // PA header is always 10 bytes
        let entries_size: usize = self
            .entries
            .iter()
            .map(|entry| {
                entry.serialized_size(STANDARD_KEY_SIZE, STANDARD_KEY_SIZE, STANDARD_KEY_SIZE)
            })
            .sum();

        header_size + entries_size
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Remove entry at index
    pub fn remove_entry(&mut self, index: usize) -> Option<PatchEntry> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    /// Get reference to entries
    pub fn entries(&self) -> &[PatchEntry] {
        &self.entries
    }

    /// Sort entries by old content key for better compression
    pub fn sort_entries(&mut self) {
        self.entries
            .sort_by(|a, b| a.old_content_key.cmp(&b.old_content_key));
    }
}

impl Default for PatchArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;
    use crate::patch_archive::PatchArchive;

    #[test]
    fn test_builder_creation() {
        let builder = PatchArchiveBuilder::new();
        assert_eq!(builder.version, 2);
        assert_eq!(builder.block_size_bits, 16);
        assert_eq!(builder.entry_count(), 0);
    }

    #[test]
    fn test_builder_configuration() {
        let builder = PatchArchiveBuilder::new().version(3).block_size_bits(14);

        assert_eq!(builder.version, 3);
        assert_eq!(builder.block_size_bits, 14);
    }

    #[test]
    fn test_add_patch() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());

        assert_eq!(builder.entry_count(), 1);
        assert_eq!(builder.entries()[0].old_content_key, [0x01; 16]);
    }

    #[test]
    fn test_build_empty() {
        let builder = PatchArchiveBuilder::new();
        let data = builder.build().expect("Operation should succeed");

        // Should be just the 10-byte header
        assert_eq!(data.len(), 10);

        // Parse it back
        let archive = PatchArchive::parse(&data).expect("Operation should succeed");
        assert_eq!(archive.header.block_count, 0);
        assert!(archive.entries.is_empty());
    }

    #[test]
    fn test_build_with_entries() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0x11; 16], [0x22; 16], [0x33; 16], "{*=n}".to_string());
        builder.add_patch([0xAA; 16], [0xBB; 16], [0xCC; 16], "{22=z,*=n}".to_string());

        let data = builder.build().expect("Operation should succeed");
        let archive = PatchArchive::parse(&data).expect("Operation should succeed");

        assert_eq!(archive.header.block_count, 2);
        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.entries[0].old_content_key, [0x11; 16]);
        assert_eq!(archive.entries[1].old_content_key, [0xAA; 16]);
    }

    #[test]
    fn test_calculate_size() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());

        let calculated_size = builder.calculate_size();
        let actual_data = builder.build().expect("Operation should succeed");

        assert_eq!(calculated_size, actual_data.len());
    }

    #[test]
    fn test_entry_management() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());
        builder.add_patch([0x04; 16], [0x05; 16], [0x06; 16], "{*=n}".to_string());

        assert_eq!(builder.entry_count(), 2);

        let removed = builder.remove_entry(0);
        assert!(removed.is_some());
        assert_eq!(
            removed.expect("Operation should succeed").old_content_key,
            [0x01; 16]
        );
        assert_eq!(builder.entry_count(), 1);

        builder.clear();
        assert_eq!(builder.entry_count(), 0);
    }

    #[test]
    fn test_sort_entries() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0xFF; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());
        builder.add_patch([0x01; 16], [0x05; 16], [0x06; 16], "{*=n}".to_string());

        builder.sort_entries();

        assert_eq!(builder.entries()[0].old_content_key, [0x01; 16]);
        assert_eq!(builder.entries()[1].old_content_key, [0xFF; 16]);
    }
}
