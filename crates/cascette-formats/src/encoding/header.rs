use crate::encoding::error::EncodingError;
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

    /// Flags field at offset 0x11 (must be 0)
    ///
    /// Agent.exe (`tact::EncodingTable::ParseHeader` at 0x6a23e6) validates
    /// this field equals 0 and rejects the encoding table otherwise.
    pub flags: u8,

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
            flags: 0,
            espec_block_size: 0,
        }
    }

    /// Validate header fields against Agent.exe constraints
    pub fn validate(&self) -> Result<(), EncodingError> {
        if self.version != 1 {
            return Err(EncodingError::UnsupportedVersion(self.version));
        }

        if self.flags != 0 {
            return Err(EncodingError::InvalidFlags(self.flags));
        }

        if self.ckey_hash_size == 0 || self.ckey_hash_size > 16 {
            return Err(EncodingError::InvalidHashSize {
                field: "ckey_hash_size",
                value: self.ckey_hash_size,
            });
        }

        if self.ekey_hash_size == 0 || self.ekey_hash_size > 16 {
            return Err(EncodingError::InvalidHashSize {
                field: "ekey_hash_size",
                value: self.ekey_hash_size,
            });
        }

        if self.ckey_page_size_kb == 0 {
            return Err(EncodingError::InvalidPageSize(0));
        }

        if self.ekey_page_size_kb == 0 {
            return Err(EncodingError::InvalidPageSize(0));
        }

        if self.ckey_page_count == 0 {
            return Err(EncodingError::InvalidPageCount {
                field: "ckey_page_count",
                value: self.ckey_page_count,
            });
        }

        if self.ekey_page_count == 0 {
            return Err(EncodingError::InvalidPageCount {
                field: "ekey_page_count",
                value: self.ekey_page_count,
            });
        }

        if self.espec_block_size == 0 {
            return Err(EncodingError::InvalidESpecBlockSize(self.espec_block_size));
        }

        Ok(())
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn valid_header() -> EncodingHeader {
        EncodingHeader {
            magic: *b"EN",
            version: 1,
            ckey_hash_size: 16,
            ekey_hash_size: 16,
            ckey_page_size_kb: 4,
            ekey_page_size_kb: 4,
            ckey_page_count: 1,
            ekey_page_count: 1,
            flags: 0,
            espec_block_size: 10,
        }
    }

    #[test]
    fn test_valid_header_passes_validation() {
        assert!(valid_header().validate().is_ok());
    }

    #[test]
    fn test_invalid_version_rejected() {
        let mut h = valid_header();
        h.version = 2;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn test_nonzero_flags_rejected() {
        // Agent.exe (tact::EncodingTable::ParseHeader at 0x6a23e6) requires
        // the flags byte at offset 0x11 to be exactly 0.
        let mut h = valid_header();
        h.flags = 1;
        assert!(matches!(h.validate(), Err(EncodingError::InvalidFlags(1))));
    }

    #[test]
    fn test_zero_ckey_hash_size_rejected() {
        let mut h = valid_header();
        h.ckey_hash_size = 0;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::InvalidHashSize {
                field: "ckey_hash_size",
                value: 0
            })
        ));
    }

    #[test]
    fn test_large_ekey_hash_size_rejected() {
        let mut h = valid_header();
        h.ekey_hash_size = 17;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::InvalidHashSize {
                field: "ekey_hash_size",
                value: 17
            })
        ));
    }

    #[test]
    fn test_boundary_hash_sizes_accepted() {
        let mut h = valid_header();
        h.ckey_hash_size = 1;
        h.ekey_hash_size = 1;
        assert!(h.validate().is_ok());
    }

    #[test]
    fn test_zero_ckey_page_count_rejected() {
        let mut h = valid_header();
        h.ckey_page_count = 0;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::InvalidPageCount {
                field: "ckey_page_count",
                ..
            })
        ));
    }

    #[test]
    fn test_zero_ekey_page_count_rejected() {
        let mut h = valid_header();
        h.ekey_page_count = 0;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::InvalidPageCount {
                field: "ekey_page_count",
                ..
            })
        ));
    }

    #[test]
    fn test_zero_espec_block_size_rejected() {
        let mut h = valid_header();
        h.espec_block_size = 0;
        assert!(matches!(
            h.validate(),
            Err(EncodingError::InvalidESpecBlockSize(0))
        ));
    }
}
