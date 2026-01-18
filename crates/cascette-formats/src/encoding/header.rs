use binrw::{BinRead, BinWrite};

/// Encoding file header (22 bytes)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)] // Big-endian for all fields
#[bw(big)]
pub struct EncodingHeader {
    /// Magic bytes: 'EN'
    #[br(assert(magic == *b"EN", "Invalid encoding magic"))]
    pub magic: [u8; 2],

    /// Version (typically 1)
    pub version: u8,

    /// Size of content key hashes (typically 16 for MD5)
    pub ckey_hash_size: u8,

    /// Size of encoding key hashes (typically 16 for MD5)
    pub ekey_hash_size: u8,

    /// Content key page size in KB
    pub ckey_page_size_kb: u16,

    /// Encoding key page size in KB
    pub ekey_page_size_kb: u16,

    /// Number of content key pages
    pub ckey_page_count: u32,

    /// Number of encoding key pages
    pub ekey_page_count: u32,

    /// Unknown field (usually 0)
    pub unk_11: u8,

    /// Size of `ESpec` block at end of file
    pub espec_block_size: u32,
}

impl EncodingHeader {
    /// Create a new encoding header with default values
    pub fn new() -> Self {
        Self {
            magic: *b"EN",
            version: 1,
            ckey_hash_size: 16,
            ekey_hash_size: 16,
            ckey_page_size_kb: 4,
            ekey_page_size_kb: 4,
            ckey_page_count: 0,
            ekey_page_count: 0,
            unk_11: 0,
            espec_block_size: 0,
        }
    }

    /// Get content key page size in bytes
    pub fn ckey_page_size(&self) -> usize {
        self.ckey_page_size_kb as usize * 1024
    }

    /// Get encoding key page size in bytes
    pub fn ekey_page_size(&self) -> usize {
        self.ekey_page_size_kb as usize * 1024
    }

    /// Calculate total size of header and all pages
    pub fn data_size(&self) -> usize {
        22 // Header size
            + (self.ckey_page_count as usize * (32 + self.ckey_page_size()))
            + (self.ekey_page_count as usize * (32 + self.ekey_page_size()))
            + self.espec_block_size as usize
    }
}

impl Default for EncodingHeader {
    fn default() -> Self {
        Self::new()
    }
}
