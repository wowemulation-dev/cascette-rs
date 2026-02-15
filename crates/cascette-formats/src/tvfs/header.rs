//! TVFS header structures and parsing

use crate::tvfs::error::{TvfsError, TvfsResult};
use binrw::{BinRead, BinWrite};

/// TVFS format flags
/// Include content keys in container file table
pub const TVFS_FLAG_INCLUDE_CKEY: u32 = 0x01;
/// Write support enabled
pub const TVFS_FLAG_WRITE_SUPPORT: u32 = 0x02;
/// Patch support enabled
pub const TVFS_FLAG_PATCH_SUPPORT: u32 = 0x04;
/// Encoding specification table present (write support flag only)
pub const TVFS_FLAG_ENCODING_SPEC: u32 = TVFS_FLAG_WRITE_SUPPORT;

/// TVFS file header (38 bytes base, 46 bytes with encoding spec table)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)] // Big-endian
#[bw(big)]
pub struct TvfsHeader {
    /// Magic bytes: "TVFS"
    #[br(assert(magic == *b"TVFS", "Invalid TVFS magic"))]
    pub magic: [u8; 4],

    /// Format version (always 1)
    pub format_version: u8,

    /// Header size (38 without EST, 46 with EST)
    pub header_size: u8,

    /// EKey size (always 9)
    pub ekey_size: u8,

    /// PKey size (always 9)
    pub pkey_size: u8,

    /// Format flags
    pub flags: u32,

    /// Path table offset
    pub path_table_offset: u32,

    /// Path table size
    pub path_table_size: u32,

    /// VFS table offset
    pub vfs_table_offset: u32,

    /// VFS table size
    pub vfs_table_size: u32,

    /// Container file table offset
    pub cft_table_offset: u32,

    /// Container file table size
    pub cft_table_size: u32,

    /// Maximum depth
    pub max_depth: u16,

    /// Encoding spec table offset (optional)
    #[br(if(flags & TVFS_FLAG_ENCODING_SPEC != 0))]
    #[bw(if(*flags & TVFS_FLAG_ENCODING_SPEC != 0))]
    pub est_table_offset: Option<u32>,

    /// Encoding spec table size (optional)
    #[br(if(flags & TVFS_FLAG_ENCODING_SPEC != 0))]
    #[bw(if(*flags & TVFS_FLAG_ENCODING_SPEC != 0))]
    pub est_table_size: Option<u32>,
}

impl TvfsHeader {
    /// Create a new TVFS header with default values
    pub fn new(flags: u32) -> Self {
        Self {
            magic: *b"TVFS",
            format_version: 1,
            header_size: if flags & TVFS_FLAG_ENCODING_SPEC != 0 {
                46
            } else {
                38
            },
            ekey_size: 9,
            pkey_size: 9,
            flags,
            path_table_offset: 0,
            path_table_size: 0,
            vfs_table_offset: 0,
            vfs_table_size: 0,
            cft_table_offset: 0,
            cft_table_size: 0,
            max_depth: 0,
            est_table_offset: if flags & TVFS_FLAG_ENCODING_SPEC != 0 {
                Some(0)
            } else {
                None
            },
            est_table_size: if flags & TVFS_FLAG_ENCODING_SPEC != 0 {
                Some(0)
            } else {
                None
            },
        }
    }

    /// Validate header values
    pub fn validate(&self) -> TvfsResult<()> {
        if self.format_version != 1 {
            return Err(TvfsError::UnsupportedVersion(self.format_version));
        }

        let expected_header_size = if self.flags & TVFS_FLAG_ENCODING_SPEC != 0 {
            46
        } else {
            38
        };
        if self.header_size != expected_header_size {
            return Err(TvfsError::InvalidHeaderSize(self.header_size));
        }

        if self.ekey_size != 9 || self.pkey_size != 9 {
            return Err(TvfsError::InvalidKeySize {
                ekey: self.ekey_size,
                pkey: self.pkey_size,
            });
        }

        Ok(())
    }

    /// Check if content keys are included
    pub fn includes_content_keys(&self) -> bool {
        (self.flags & TVFS_FLAG_INCLUDE_CKEY) != 0
    }

    /// Check if write support is enabled
    pub fn has_write_support(&self) -> bool {
        (self.flags & TVFS_FLAG_WRITE_SUPPORT) != 0
    }

    /// Check if patch support is enabled
    pub fn has_patch_support(&self) -> bool {
        (self.flags & TVFS_FLAG_PATCH_SUPPORT) != 0
    }

    /// Check if encoding spec table is present
    pub fn has_encoding_spec(&self) -> bool {
        (self.flags & TVFS_FLAG_ENCODING_SPEC) != 0
    }

    /// Update table offsets and sizes after building tables
    #[allow(clippy::too_many_arguments)]
    pub fn update_table_info(
        &mut self,
        path_table_offset: u32,
        path_table_size: u32,
        vfs_table_offset: u32,
        vfs_table_size: u32,
        cft_table_offset: u32,
        cft_table_size: u32,
        max_depth: u16,
    ) {
        self.path_table_offset = path_table_offset;
        self.path_table_size = path_table_size;
        self.vfs_table_offset = vfs_table_offset;
        self.vfs_table_size = vfs_table_size;
        self.cft_table_offset = cft_table_offset;
        self.cft_table_size = cft_table_size;
        self.max_depth = max_depth;
    }
}
