//! BLTE (Block Table Entry) Compression/Decompression Library
//!
//! BLTE is Blizzard's compression and encryption format used throughout
//! their content distribution system. This crate provides parsing and
//! decompression capabilities for all BLTE modes.

mod chunk;
// pub mod decompress;
mod error;
mod header;
mod read;
// pub mod stream;

pub use chunk::{ChunkEncodingHeader, EncryptedChunkHeader};
// pub use decompress::{decompress_blte, decompress_chunk};
pub use error::{Error, Result};
pub use header::{BLTEHeader, ChunkInfo};
pub use read::BLTEFile;
// pub use stream::{BLTEStream, create_streaming_reader};

/// BLTE magic bytes
pub const BLTE_MAGIC: [u8; 4] = *b"BLTE";

const MD5_LENGTH: usize = 16;
pub type Md5 = [u8; MD5_LENGTH];

