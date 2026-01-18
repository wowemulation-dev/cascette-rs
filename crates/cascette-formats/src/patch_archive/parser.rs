//! Streaming parser for Patch Archives

use crate::patch_archive::{
    PatchArchiveHeader, PatchEntry,
    error::{PatchArchiveError, PatchArchiveResult},
};
use binrw::{
    BinRead,
    io::{Read, Seek},
};

/// Streaming parser for processing large Patch Archives without loading everything into memory
pub struct PatchArchiveParser<R: Read + Seek> {
    reader: R,
    header: PatchArchiveHeader,
    entries_read: usize,
}

impl<R: Read + Seek> PatchArchiveParser<R> {
    /// Create new parser from reader
    pub fn new(mut reader: R) -> PatchArchiveResult<Self> {
        // Read header (big-endian)
        let header = PatchArchiveHeader::read_options(&mut reader, binrw::Endian::Big, ())
            .map_err(|e| match e {
                binrw::Error::Io(io_err) => PatchArchiveError::DecompressionError(io_err),
                binrw::Error::AssertFail { .. } => {
                    PatchArchiveError::InvalidHeader("PA magic assertion failed".to_string())
                }
                other => PatchArchiveError::BinRw(other),
            })?;

        // Validate header
        header.validate()?;

        Ok(Self {
            reader,
            header,
            entries_read: 0,
        })
    }

    /// Get reference to the header
    pub fn header(&self) -> &PatchArchiveHeader {
        &self.header
    }

    /// Get number of entries already read
    pub fn entries_read(&self) -> usize {
        self.entries_read
    }

    /// Get total number of entries in the archive
    pub fn total_entries(&self) -> usize {
        self.header.block_count as usize
    }

    /// Check if there are more entries to read
    pub fn has_more_entries(&self) -> bool {
        self.entries_read < self.total_entries()
    }

    /// Read the next patch entry
    pub fn next_entry(&mut self) -> PatchArchiveResult<Option<PatchEntry>> {
        if self.entries_read >= self.header.block_count as usize {
            return Ok(None);
        }

        let args = (
            self.header.file_key_size,
            self.header.old_key_size,
            self.header.patch_key_size,
        );

        let entry = PatchEntry::read_options(
            &mut self.reader,
            binrw::Endian::Little, // Entries may use different endianness
            args,
        )
        .map_err(|e| match e {
            binrw::Error::Io(io_err) => PatchArchiveError::DecompressionError(io_err),
            other => PatchArchiveError::BinRw(other),
        })?;

        self.entries_read += 1;
        Ok(Some(entry))
    }

    /// Collect all remaining entries into a vector
    pub fn collect_all_entries(mut self) -> PatchArchiveResult<Vec<PatchEntry>> {
        let mut entries = Vec::with_capacity(self.header.block_count as usize);

        while let Some(entry) = self.next_entry()? {
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Skip the next entry without parsing it fully
    pub fn skip_entry(&mut self) -> PatchArchiveResult<bool> {
        if self.entries_read >= self.header.block_count as usize {
            return Ok(false);
        }

        // Read the keys to advance the reader
        let key_total = (self.header.file_key_size
            + self.header.old_key_size
            + self.header.patch_key_size) as usize;
        let mut key_buffer = vec![0u8; key_total];
        self.reader.read_exact(&mut key_buffer)?;

        // Skip compression info string
        let mut byte = [0u8; 1];
        loop {
            self.reader.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break; // Found null terminator
            }
        }

        // Note: We assume no additional data for skipping
        // In a real implementation, this might need format-specific logic

        self.entries_read += 1;
        Ok(true)
    }

    /// Find first entry matching predicate without loading all entries
    pub fn find_entry<F>(&mut self, predicate: F) -> PatchArchiveResult<Option<PatchEntry>>
    where
        F: Fn(&PatchEntry) -> bool,
    {
        while let Some(entry) = self.next_entry()? {
            if predicate(&entry) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Find entry with specific old content key
    pub fn find_entry_by_old_key(
        &mut self,
        key: &[u8; 16],
    ) -> PatchArchiveResult<Option<PatchEntry>> {
        self.find_entry(|entry| &entry.old_content_key == key)
    }

    /// Process entries with a closure without storing them all
    pub fn process_entries<F, T>(&mut self, mut processor: F) -> PatchArchiveResult<Vec<T>>
    where
        F: FnMut(&PatchEntry) -> PatchArchiveResult<T>,
    {
        let mut results = Vec::new();

        while let Some(entry) = self.next_entry()? {
            let result = processor(&entry)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get current reader position for debugging
    pub fn current_position(&mut self) -> std::io::Result<u64> {
        self.reader.stream_position()
    }

    /// Reset parser to beginning (requires Seek)
    pub fn reset(&mut self) -> PatchArchiveResult<()> {
        use std::io::SeekFrom;

        self.reader.seek(SeekFrom::Start(0))?;
        self.entries_read = 0;

        // Re-read and validate header
        let header = PatchArchiveHeader::read_options(&mut self.reader, binrw::Endian::Big, ())?;
        header.validate()?;
        self.header = header;

        Ok(())
    }
}

/// Iterator implementation for streaming parsing
impl<R: Read + Seek> Iterator for PatchArchiveParser<R> {
    type Item = PatchArchiveResult<PatchEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_entry() {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.total_entries() - self.entries_read;
        (remaining, Some(remaining))
    }
}

impl<R: Read + Seek> std::iter::ExactSizeIterator for PatchArchiveParser<R> {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::patch_archive::PatchArchiveBuilder;
    use std::io::Cursor;

    fn create_test_archive() -> Vec<u8> {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_patch([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());
        builder.add_patch([0x04; 16], [0x05; 16], [0x06; 16], "{*=n}".to_string());
        builder.add_patch([0x07; 16], [0x08; 16], [0x09; 16], "{22=z,*=n}".to_string());
        builder.build().expect("Operation should succeed")
    }

    #[test]
    fn test_parser_creation() {
        let data = create_test_archive();
        let parser = PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        assert_eq!(parser.header().block_count, 3);
        assert_eq!(parser.entries_read(), 0);
        assert_eq!(parser.total_entries(), 3);
        assert!(parser.has_more_entries());
    }

    #[test]
    fn test_streaming_iteration() {
        let data = create_test_archive();
        let mut parser =
            PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        // Read entries one by one
        let entry1 = parser
            .next_entry()
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        assert_eq!(entry1.old_content_key, [0x01; 16]);
        assert_eq!(parser.entries_read(), 1);

        let entry2 = parser
            .next_entry()
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        assert_eq!(entry2.old_content_key, [0x04; 16]);
        assert_eq!(parser.entries_read(), 2);

        let entry3 = parser
            .next_entry()
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        assert_eq!(entry3.old_content_key, [0x07; 16]);
        assert_eq!(parser.entries_read(), 3);

        // No more entries
        assert!(
            parser
                .next_entry()
                .expect("Parser should succeed")
                .is_none()
        );
        assert!(!parser.has_more_entries());
    }

    #[test]
    fn test_collect_all() {
        let data = create_test_archive();
        let parser = PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        let entries = parser
            .collect_all_entries()
            .expect("Operation should succeed");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].old_content_key, [0x01; 16]);
        assert_eq!(entries[1].old_content_key, [0x04; 16]);
        assert_eq!(entries[2].old_content_key, [0x07; 16]);
    }

    #[test]
    fn test_find_entry() {
        let data = create_test_archive();
        let mut parser =
            PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        let found = parser
            .find_entry_by_old_key(&[0x04; 16])
            .expect("Operation should succeed");
        assert!(found.is_some());
        assert_eq!(
            found.expect("Entry should be found").new_content_key,
            [0x05; 16]
        );

        // Should have consumed entries up to the found one
        assert_eq!(parser.entries_read(), 2);
    }

    #[test]
    fn test_iterator_interface() {
        let data = create_test_archive();
        let parser = PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        let entries: Result<Vec<_>, _> = parser.collect();
        let entries = entries.expect("Operation should succeed");

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].old_content_key, [0x01; 16]);
        assert_eq!(entries[2].compression_info, "{22=z,*=n}");
    }

    #[test]
    fn test_size_hint() {
        let data = create_test_archive();
        let parser = PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        let (lower, upper) = parser.size_hint();
        assert_eq!(lower, 3);
        assert_eq!(upper, Some(3));
    }

    #[test]
    fn test_process_entries() {
        let data = create_test_archive();
        let mut parser =
            PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        let hex_keys = parser
            .process_entries(|entry| Ok(entry.old_content_key_hex()))
            .expect("Operation should succeed");

        assert_eq!(hex_keys.len(), 3);
        assert_eq!(hex_keys[0], "01010101010101010101010101010101");
        assert_eq!(hex_keys[1], "04040404040404040404040404040404");
    }

    #[test]
    fn test_reset() {
        let data = create_test_archive();
        let mut parser =
            PatchArchiveParser::new(Cursor::new(&data)).expect("Operation should succeed");

        // Read some entries
        parser.next_entry().expect("Operation should succeed");
        parser.next_entry().expect("Operation should succeed");
        assert_eq!(parser.entries_read(), 2);

        // Reset
        parser.reset().expect("Operation should succeed");
        assert_eq!(parser.entries_read(), 0);
        assert!(parser.has_more_entries());

        // Should be able to read from the beginning again
        let entry = parser
            .next_entry()
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        assert_eq!(entry.old_content_key, [0x01; 16]);
    }
}
