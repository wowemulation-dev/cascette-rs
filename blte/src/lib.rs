//! BLTE (Block Table Entry) Compression/Decompression Library
//!
//! BLTE is Blizzard's compression and encryption format used throughout
//! their content distribution system. This crate provides parsing and
//! decompression capabilities for all BLTE modes.

pub mod chunk;
pub mod decompress;
pub mod error;
pub mod header;
pub mod stream;

pub use chunk::{BLTEFile, ChunkData};
pub use decompress::{decompress_blte, decompress_chunk};
pub use error::{Error, Result};
pub use header::{BLTEHeader, ChunkInfo};
pub use stream::{BLTEStream, create_streaming_reader};

/// BLTE magic bytes
pub const BLTE_MAGIC: [u8; 4] = *b"BLTE";

const MD5_LENGTH: usize = 16;
pub type Md5 = [u8; MD5_LENGTH];

/// BLTE compression modes
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum CompressionMode {
    /// No compression (mode 'N')
    None = b'N',
    /// ZLib compression (mode 'Z')
    ZLib = b'Z',
    /// LZ4 compression (mode '4')
    LZ4 = b'4',
    /// Frame/Recursive BLTE (mode 'F')
    Frame = b'F',
    /// Encrypted (mode 'E')
    Encrypted = b'E',
}

impl CompressionMode {
    /// Parse compression mode from byte
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'N' => Some(Self::None),
            b'Z' => Some(Self::ZLib),
            b'4' => Some(Self::LZ4),
            b'F' => Some(Self::Frame),
            b'E' => Some(Self::Encrypted),
            _ => None,
        }
    }

    /// Get the byte representation
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}
