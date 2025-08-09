//! Patch file format for NGDP patch manifests

use crate::error::{PatchError, Result};
use crate::patch_entry::{PatchEntry, PatchIndex};
use crate::{ContentKey, EncodingKey};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tracing::{debug, info, trace};

/// Magic signature for patch files
const PATCH_MAGIC: &[u8; 2] = b"PA";

/// Patch file header
#[derive(Debug, Clone)]
pub struct PatchHeader {
    /// Magic signature
    pub magic: [u8; 2],
    /// Version of the patch format
    pub version: u8,
    /// Encoding version
    pub encoding_version: u8,
    /// Size key size (typically 9 for truncated MD5)
    pub size_key_size: u8,
    /// Page size as power of 2 (e.g., 12 = 4096 bytes)
    pub page_size_bits: u8,
    /// Number of patch blocks
    pub block_count: u32,
    /// Patch specification flags
    pub flags: u8,
}

impl PatchHeader {
    /// Create a new patch header with default values
    pub fn new() -> Self {
        Self {
            magic: *PATCH_MAGIC,
            version: 1,
            encoding_version: 1,
            size_key_size: 9,
            page_size_bits: 12, // 4096 bytes
            block_count: 0,
            flags: 0,
        }
    }

    /// Read header from a stream
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;

        if magic != *PATCH_MAGIC {
            return Err(PatchError::InvalidFormat(format!(
                "Invalid magic: expected {PATCH_MAGIC:?}, got {magic:?}"
            )));
        }

        let version = reader.read_u8()?;
        let encoding_version = reader.read_u8()?;
        let size_key_size = reader.read_u8()?;
        let page_size_bits = reader.read_u8()?;
        let block_count = reader.read_u32::<BigEndian>()?;
        let flags = reader.read_u8()?;

        // Skip padding
        let mut _padding = [0u8; 3];
        reader.read_exact(&mut _padding)?;

        Ok(Self {
            magic,
            version,
            encoding_version,
            size_key_size,
            page_size_bits,
            block_count,
            flags,
        })
    }

    /// Write header to a stream
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_u8(self.version)?;
        writer.write_u8(self.encoding_version)?;
        writer.write_u8(self.size_key_size)?;
        writer.write_u8(self.page_size_bits)?;
        writer.write_u32::<BigEndian>(self.block_count)?;
        writer.write_u8(self.flags)?;

        // Write padding
        writer.write_all(&[0u8; 3])?;

        Ok(())
    }

    /// Get the page size in bytes
    pub fn page_size(&self) -> usize {
        1 << self.page_size_bits
    }
}

impl Default for PatchHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// NGDP patch file containing patch entries
#[derive(Debug)]
pub struct PatchFile {
    /// File header
    pub header: PatchHeader,
    /// Index of patch entries
    pub entries: PatchIndex,
    /// File checksum
    pub checksum: Option<ContentKey>,
}

impl PatchFile {
    /// Create a new empty patch file
    pub fn new() -> Self {
        Self {
            header: PatchHeader::new(),
            entries: PatchIndex::new(),
            checksum: None,
        }
    }

    /// Load a patch file from disk
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        debug!("Loading patch file from: {:?}", path);

        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read(&mut reader)
    }

    /// Read a patch file from a stream
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        // Read header
        let header = PatchHeader::read(reader)?;
        trace!("Read patch header: {:?}", header);

        // Read block offsets
        let mut block_offsets = Vec::with_capacity(header.block_count as usize);
        for _ in 0..header.block_count {
            // Skip last page key
            let mut _key = vec![0u8; header.size_key_size as usize];
            reader.read_exact(&mut _key)?;

            // Skip page hash
            let mut _hash = [0u8; 16];
            reader.read_exact(&mut _hash)?;

            // Read offset
            let offset = reader.read_u32::<BigEndian>()?;
            block_offsets.push(offset);
        }

        // Read patch entries from blocks
        let mut entries = PatchIndex::new();
        let page_size = header.page_size();

        for offset in block_offsets {
            reader.seek(SeekFrom::Start(offset as u64))?;

            // Read page
            let mut page_data = vec![0u8; page_size];
            let bytes_read = reader.read(&mut page_data)?;
            page_data.truncate(bytes_read);

            // Parse entries from page
            let mut page_cursor = std::io::Cursor::new(page_data);
            while page_cursor.position() < page_cursor.get_ref().len() as u64 {
                match PatchEntry::read(&mut page_cursor) {
                    Ok(entry) => entries.add_entry(entry),
                    Err(_) => break, // End of valid entries in page
                }
            }
        }

        info!("Loaded {} patch entries", entries.len());

        Ok(Self {
            header,
            entries,
            checksum: None,
        })
    }

    /// Save patch file to disk
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        debug!("Saving patch file to: {:?}", path);

        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write(&mut writer)?;

        Ok(())
    }

    /// Write patch file to a stream
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Update block count
        let mut header = self.header.clone();
        header.block_count = self.calculate_block_count();

        // Write header
        header.write(writer)?;

        // Group entries into pages
        let pages = self.build_pages();

        // Write block index
        for (idx, _page) in pages.iter().enumerate() {
            // Write truncated key (first N bytes of first entry's old_ckey)
            if let Some(first_entry) = self.entries.iter().nth(idx * 100) {
                writer.write_all(&first_entry.old_ckey[..header.size_key_size as usize])?;
            } else {
                writer.write_all(&vec![0u8; header.size_key_size as usize])?;
            }

            // Write page hash (placeholder)
            writer.write_all(&[0u8; 16])?;

            // Write offset (will be updated later)
            writer.write_u32::<BigEndian>(0)?;
        }

        // Write pages
        for page in pages {
            writer.write_all(&page)?;
        }

        Ok(())
    }

    /// Calculate number of blocks needed
    fn calculate_block_count(&self) -> u32 {
        let entries_per_page = self.header.page_size() / 100; // Approximate entry size
        self.entries.len().div_ceil(entries_per_page) as u32
    }

    /// Build pages from entries
    fn build_pages(&self) -> Vec<Vec<u8>> {
        let mut pages = Vec::new();
        let mut current_page = Vec::new();
        let page_size = self.header.page_size();

        for entry in self.entries.iter() {
            let mut entry_data = Vec::new();
            if entry.write(&mut entry_data).is_ok() {
                if current_page.len() + entry_data.len() > page_size {
                    // Start new page
                    current_page.resize(page_size, 0);
                    pages.push(current_page);
                    current_page = entry_data;
                } else {
                    current_page.extend_from_slice(&entry_data);
                }
            }
        }

        // Add final page
        if !current_page.is_empty() {
            current_page.resize(page_size, 0);
            pages.push(current_page);
        }

        pages
    }

    /// Find a patch for updating from old to new content
    pub fn find_patch(
        &self,
        old_ckey: &ContentKey,
        new_ckey: &ContentKey,
    ) -> Option<Vec<&PatchEntry>> {
        self.entries.find_patch_chain(old_ckey, new_ckey)
    }

    /// Apply a patch to transform old content to new content
    pub async fn apply_patch(
        &self,
        old_data: &[u8],
        old_ckey: &ContentKey,
        new_ckey: &ContentKey,
        patch_provider: impl Fn(&EncodingKey) -> Result<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        // Find patch chain
        let chain = self
            .find_patch(old_ckey, new_ckey)
            .ok_or_else(|| PatchError::PatchNotFound(format!("{:02x?}", &old_ckey[..4])))?;

        debug!("Found patch chain with {} steps", chain.len());

        // Apply patches in sequence
        let mut current_data = old_data.to_vec();

        for entry in chain {
            trace!(
                "Applying patch: {:02x?} -> {:02x?}",
                &entry.old_ckey[..4],
                &entry.new_ckey[..4]
            );

            // Get patch data
            let patch_data = patch_provider(&entry.patch_ekey)?;

            // Apply patch
            current_data = crate::zbsdiff::apply_patch(&current_data, &patch_data)?;

            // Verify size
            if current_data.len() != entry.new_size as usize {
                return Err(PatchError::SizeMismatch {
                    expected: entry.new_size as usize,
                    actual: current_data.len(),
                });
            }

            // TODO: Verify checksum
        }

        Ok(current_data)
    }
}

impl Default for PatchFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_header_roundtrip() {
        let header = PatchHeader::new();

        let mut buffer = Vec::new();
        header.write(&mut buffer).unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let read_header = PatchHeader::read(&mut cursor).unwrap();

        assert_eq!(header.magic, read_header.magic);
        assert_eq!(header.version, read_header.version);
        assert_eq!(header.page_size_bits, read_header.page_size_bits);
    }

    #[test]
    fn test_page_size() {
        let mut header = PatchHeader::new();

        header.page_size_bits = 12;
        assert_eq!(header.page_size(), 4096);

        header.page_size_bits = 14;
        assert_eq!(header.page_size(), 16384);
    }

    #[test]
    fn test_patch_file_creation() {
        let mut patch_file = PatchFile::new();

        // Add some entries
        let entry1 = PatchEntry::new([1u8; 16], [2u8; 16], [10u8; 16], 100, 150);
        let entry2 = PatchEntry::new([2u8; 16], [3u8; 16], [11u8; 16], 150, 200);

        patch_file.entries.add_entry(entry1);
        patch_file.entries.add_entry(entry2);

        // Find patch chain
        let chain = patch_file.find_patch(&[1u8; 16], &[3u8; 16]);
        assert!(chain.is_some());
        assert_eq!(chain.unwrap().len(), 2);
    }
}
