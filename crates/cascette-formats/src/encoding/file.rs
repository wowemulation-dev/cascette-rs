use crate::blte::BlteFile;
use crate::encoding::{
    ESpecTable, EncodingError, EncodingHeader,
    entry::{CKeyPageEntry, EKeyPageEntry},
    index::IndexEntry,
};
use binrw::{BinRead, BinWrite};
use cascette_crypto::{ContentKey, EncodingKey};
use std::io::{Cursor, Read};

/// Page data with entries
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// List of entries in this page
    pub entries: Vec<T>,
    /// Original binary page data (preserved for exact reconstruction)
    pub original_data: Vec<u8>,
}

/// Complete encoding file structure
#[derive(Debug, Clone)]
pub struct EncodingFile {
    /// File header with metadata
    pub header: EncodingHeader,
    /// `ESpec` compression specification table (comes after header)
    pub espec_table: ESpecTable,
    /// Content key index for page lookups
    pub ckey_index: Vec<IndexEntry>,
    /// Content key pages
    pub ckey_pages: Vec<Page<CKeyPageEntry>>,
    /// Encoding key index for page lookups
    pub ekey_index: Vec<IndexEntry>,
    /// Encoding key pages
    pub ekey_pages: Vec<Page<EKeyPageEntry>>,
    /// Self-describing `ESpec` at end of file
    pub trailing_espec: Option<String>,
}

impl EncodingFile {
    /// Parse `CKey` pages from cursor
    fn parse_ckey_pages(
        cursor: &mut Cursor<&[u8]>,
        header: &EncodingHeader,
        ckey_index: &[IndexEntry],
    ) -> Result<Vec<Page<CKeyPageEntry>>, EncodingError> {
        let mut ckey_pages = Vec::with_capacity(header.ckey_page_count as usize);
        let ckey_page_size = header.ckey_page_size();

        for index in ckey_index {
            let mut page_data = vec![0u8; ckey_page_size];
            cursor.read_exact(&mut page_data)?;

            // Verify checksum
            if !index.verify(&page_data) {
                return Err(EncodingError::ChecksumMismatch);
            }

            // Parse entries from page using BinRead
            let mut page_cursor = Cursor::new(&page_data);
            let mut entries = Vec::new();

            while page_cursor.position() < page_data.len() as u64 {
                let pos_before = page_cursor.position();
                // Try to read entry using BinRead implementation
                match CKeyPageEntry::read_options(&mut page_cursor, binrw::Endian::Big, ()) {
                    Ok(entry) => {
                        entries.push(entry);
                    }
                    Err(e) => {
                        // Check if we're at the end of meaningful data
                        let remaining = page_data.len() as u64 - pos_before;
                        if remaining < 22 {
                            // Minimum entry size (1 + 5 + 16 + 0*16)
                            break; // Not enough space for another entry
                        }
                        // If we have space but still failed, it might be padding
                        // Check the next byte to see if it's zero (padding)
                        page_cursor.set_position(pos_before);
                        if let Ok(next_byte) =
                            u8::read_options(&mut page_cursor, binrw::Endian::Big, ())
                        {
                            if next_byte == 0x00 {
                                break; // Hit padding
                            }
                        }
                        // Reset and break on any other error
                        page_cursor.set_position(pos_before);
                        return Err(EncodingError::BinRw(e));
                    }
                }
            }

            ckey_pages.push(Page {
                entries,
                original_data: page_data,
            });
        }

        Ok(ckey_pages)
    }

    /// Parse `EKey` pages from cursor
    fn parse_ekey_pages(
        cursor: &mut Cursor<&[u8]>,
        header: &EncodingHeader,
        ekey_index: &[IndexEntry],
    ) -> Result<Vec<Page<EKeyPageEntry>>, EncodingError> {
        let mut ekey_pages = Vec::with_capacity(header.ekey_page_count as usize);
        let ekey_page_size = header.ekey_page_size();

        for index in ekey_index {
            let mut page_data = vec![0u8; ekey_page_size];
            cursor.read_exact(&mut page_data)?;

            // Verify checksum
            if !index.verify(&page_data) {
                return Err(EncodingError::ChecksumMismatch);
            }

            // Parse entries from page using BinRead
            let mut page_cursor = Cursor::new(&page_data);
            let mut entries = Vec::new();

            while page_cursor.position() < page_data.len() as u64 {
                let pos_before = page_cursor.position();
                // Try to read entry using BinRead implementation
                match EKeyPageEntry::read_options(&mut page_cursor, binrw::Endian::Big, ()) {
                    Ok(entry) => {
                        entries.push(entry);
                    }
                    Err(e) => {
                        // Check if we're at the end of meaningful data
                        let remaining = page_data.len() as u64 - pos_before;
                        if remaining < 25 {
                            // Minimum EKey entry size (16 + 4 + 5)
                            break; // Not enough space for another entry
                        }
                        // If we have space but still failed, it might be padding
                        // Check if we're hitting all-zero padding
                        page_cursor.set_position(pos_before);
                        let mut check_bytes = [0u8; 16];
                        if page_cursor.read_exact(&mut check_bytes).is_ok()
                            && check_bytes.iter().all(|&b| b == 0x00)
                        {
                            break; // Hit padding
                        }
                        // Reset and break on any other error
                        page_cursor.set_position(pos_before);
                        return Err(EncodingError::BinRw(e));
                    }
                }
            }

            ekey_pages.push(Page {
                entries,
                original_data: page_data,
            });
        }

        Ok(ekey_pages)
    }
    /// Parse encoding file from BLTE-compressed data
    pub fn parse_blte(data: &[u8]) -> Result<Self, EncodingError> {
        // First decompress BLTE
        let mut cursor = Cursor::new(data);
        let blte = BlteFile::read_options(&mut cursor, binrw::Endian::Big, ())?;
        let decompressed = blte.decompress()?;

        // Parse decompressed encoding data
        Self::parse(&decompressed)
    }

    /// Parse encoding file from decompressed data
    pub fn parse(data: &[u8]) -> Result<Self, EncodingError> {
        let mut cursor = Cursor::new(data);

        // Read header
        let header =
            EncodingHeader::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| {
                if let binrw::Error::AssertFail { message, .. } = &e {
                    if message.contains("Invalid encoding magic") {
                        // Extract the actual magic bytes for better error
                        let magic = [data[0], data[1]];
                        return EncodingError::InvalidMagic(magic);
                    }
                }
                EncodingError::BinRw(e)
            })?;

        // Validate header
        if header.version != 1 {
            return Err(EncodingError::UnsupportedVersion(header.version));
        }

        // Read ESpec table (comes right after header per CASC specification)
        let espec_table = if header.espec_block_size > 0 {
            let mut espec_data = vec![0u8; header.espec_block_size as usize];
            cursor.read_exact(&mut espec_data)?;
            ESpecTable::parse(&espec_data)?
        } else {
            // No ESpec table in header
            ESpecTable::default()
        };

        // Read CKey index
        let mut ckey_index = Vec::with_capacity(header.ckey_page_count as usize);

        for _ in 0..header.ckey_page_count {
            // Read index entry manually to avoid binrw issues
            let mut first_key = [0u8; 16];
            let mut checksum = [0u8; 16];
            cursor.read_exact(&mut first_key)?;
            cursor.read_exact(&mut checksum)?;
            ckey_index.push(IndexEntry::new(first_key, checksum));
        }

        // Read CKey pages
        let ckey_pages = Self::parse_ckey_pages(&mut cursor, &header, &ckey_index)?;

        // Read EKey index
        let mut ekey_index = Vec::with_capacity(header.ekey_page_count as usize);
        for _ in 0..header.ekey_page_count {
            // Read index entry manually to avoid binrw issues
            let mut first_key = [0u8; 16];
            let mut checksum = [0u8; 16];
            cursor.read_exact(&mut first_key)?;
            cursor.read_exact(&mut checksum)?;
            ekey_index.push(IndexEntry::new(first_key, checksum));
        }

        // Read EKey pages
        let ekey_pages = Self::parse_ekey_pages(&mut cursor, &header, &ekey_index)?;

        // Read trailing self-describing ESpec if present
        let mut trailing_data = Vec::new();
        cursor.read_to_end(&mut trailing_data)?;
        let trailing_espec = if trailing_data.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&trailing_data).to_string())
        };

        Ok(Self {
            header,
            espec_table,
            ckey_index,
            ckey_pages,
            ekey_index,
            ekey_pages,
            trailing_espec,
        })
    }

    /// Build encoding file into raw bytes
    pub fn build(&self) -> Result<Vec<u8>, EncodingError> {
        let mut data = Vec::new();
        let mut cursor = Cursor::new(&mut data);

        // Write header
        self.header
            .write_options(&mut cursor, binrw::Endian::Big, ())?;

        // Write ESpec table (right after header per CASC specification)
        let espec_data = self.espec_table.build();
        if espec_data.len() != self.header.espec_block_size as usize {
            return Err(EncodingError::InvalidESpecSize);
        }
        std::io::Write::write_all(&mut cursor, &espec_data)?;

        // Use original CKey page data (exact binary preservation)
        let mut ckey_pages_data = Vec::new();
        let mut ckey_index_updated = Vec::new();

        for (page_idx, page) in self.ckey_pages.iter().enumerate() {
            // Use the original page data exactly as it was
            let page_data = page.original_data.clone();

            // Calculate checksum of original data (should match original index)
            let checksum = md5::compute(&page_data);
            let original_index = &self.ckey_index[page_idx];

            // Use the original index entry (should be identical)
            ckey_index_updated.push(IndexEntry::new(original_index.first_key, *checksum));
            ckey_pages_data.push(page_data);
        }

        // Write updated CKey index
        for entry in &ckey_index_updated {
            entry.write_options(&mut cursor, binrw::Endian::Big, ())?;
        }

        // Write CKey pages
        for page_data in &ckey_pages_data {
            std::io::Write::write_all(&mut cursor, page_data)?;
        }

        // Use original EKey page data (exact binary preservation)
        let mut ekey_pages_data = Vec::new();
        let mut ekey_index_updated = Vec::new();

        for (page_idx, page) in self.ekey_pages.iter().enumerate() {
            // Use the original page data exactly as it was
            let page_data = page.original_data.clone();

            // Calculate checksum of original data (should match original index)
            let checksum = md5::compute(&page_data);
            let original_index = &self.ekey_index[page_idx];

            // Use the original index entry (should be identical)
            ekey_index_updated.push(IndexEntry::new(original_index.first_key, *checksum));
            ekey_pages_data.push(page_data);
        }

        // Write updated EKey index
        for entry in &ekey_index_updated {
            entry.write_options(&mut cursor, binrw::Endian::Big, ())?;
        }

        // Write EKey pages
        for page_data in &ekey_pages_data {
            std::io::Write::write_all(&mut cursor, page_data)?;
        }

        // Write trailing self-describing ESpec if present
        if let Some(ref trailing) = self.trailing_espec {
            std::io::Write::write_all(&mut cursor, trailing.as_bytes())?;
        }

        Ok(data)
    }

    /// Build encoding file and compress with BLTE
    pub fn build_blte(&self) -> Result<Vec<u8>, EncodingError> {
        let uncompressed = self.build()?;

        // Create BLTE with ZLib compression
        let blte = BlteFile::single_chunk(uncompressed, crate::blte::CompressionMode::ZLib)?;

        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        blte.write_options(&mut cursor, binrw::Endian::Big, ())?;
        Ok(output)
    }

    /// Find encoding key for a content key
    pub fn find_encoding(&self, content_key: &ContentKey) -> Option<EncodingKey> {
        // Binary search through pages to find the right page
        for page in &self.ckey_pages {
            for entry in &page.entries {
                if entry.content_key == *content_key {
                    return entry.encoding_keys.first().copied();
                }
            }
        }
        None
    }

    /// Find all encoding keys for a given content key
    pub fn find_all_encodings(&self, content_key: &ContentKey) -> Vec<EncodingKey> {
        for page in &self.ckey_pages {
            for entry in &page.entries {
                if entry.content_key == *content_key {
                    return entry.encoding_keys.clone();
                }
            }
        }
        Vec::new()
    }

    /// Find `ESpec` for an encoding key
    pub fn find_espec(&self, encoding_key: &EncodingKey) -> Option<&str> {
        // Search through EKey pages
        for page in &self.ekey_pages {
            for entry in &page.entries {
                if entry.encoding_key == *encoding_key {
                    return self.espec_table.get(entry.espec_index);
                }
            }
        }
        None
    }

    /// Get total number of content keys
    pub fn ckey_count(&self) -> usize {
        self.ckey_pages.iter().map(|p| p.entries.len()).sum()
    }

    /// Get total number of encoding keys
    pub fn ekey_count(&self) -> usize {
        self.ekey_pages.iter().map(|p| p.entries.len()).sum()
    }
}

impl crate::CascFormat for EncodingFile {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}
