//! BLTE (Block Table Entry) Compression/Decompression Library
//!
//! BLTE is Blizzard's compression and encryption format used throughout
//! their content distribution system. This crate provides parsing and
//! decompression capabilities for all BLTE modes.
//!
//! ## Archive Support
//!
//! Blizzard's CDN serves content as 256MB archive files containing multiple
//! concatenated BLTE files. Use the `archive` module for handling these files.

pub mod adaptive;
pub mod archive;
pub mod builder;
pub mod chunk;
pub mod compress;
pub mod decompress;
pub mod error;
pub mod header;
pub mod memory_pool;
pub mod stream;

pub use adaptive::{
    CompressionRecommendation, DataAnalysis, FileType, analyze_data, auto_compress,
    compress_with_best_ratio, select_compression_mode,
};
pub use archive::{
    ArchiveEntry, ArchiveMetadata, ArchiveStats, BLTEArchive,
    builder::{ArchiveBuilder, MultiArchiveBuilder},
    recreation::{
        ChunkStructure, ExtractedFile, HeaderFormat, OriginalFileMetadata, PerfectArchiveBuilder,
        analyze_chunk_structure, detect_compression_mode, detect_header_format,
        recreate_perfect_blte_file,
    },
};
pub use builder::{
    BLTEBuilder, ChunkSpec, CompressionStrategy, EncryptionAlgorithm, EncryptionSpec,
};
pub use chunk::{BLTEFile, BLTEFileRef, ChunkData, ChunkDataRef};
pub use compress::{
    EncryptionMethod,
    auto_select_compression_mode,
    compress_chunk,
    compress_data_encrypted_multi,
    compress_data_encrypted_single,
    compress_data_multi,
    compress_data_single,
    // Encryption support
    compress_encrypted,
    create_single_chunk_blte,
};
pub use decompress::{
    decompress_blte, decompress_blte_pooled, decompress_chunk, decompress_chunk_pooled,
};
pub use error::{Error, Result};
pub use header::{BLTEHeader, ChunkInfo};
pub use memory_pool::{
    BLTEMemoryPool, PoolConfig, PoolStats, PooledBuffer, PooledBufferGuard, global_pool,
    init_global_pool,
};
pub use stream::{BLTEStream, create_streaming_reader};

/// BLTE magic bytes
pub const BLTE_MAGIC: [u8; 4] = [b'B', b'L', b'T', b'E'];

/// BLTE compression modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionMode {
    /// No compression (mode 'N')
    None = b'N' as isize,
    /// ZLib compression (mode 'Z')
    ZLib = b'Z' as isize,
    /// LZ4 compression (mode '4')
    LZ4 = b'4' as isize,
    /// Frame/Recursive BLTE (mode 'F')
    Frame = b'F' as isize,
    /// Encrypted (mode 'E')
    Encrypted = b'E' as isize,
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
