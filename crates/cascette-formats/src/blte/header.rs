//! BLTE header structures and parsing
//!
//! Uses expect in binrw map functions where Result types cannot be used.
#![allow(clippy::expect_used)]

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};

use super::chunk::ChunkData;
use super::error::{BlteError, BlteResult};

/// BLTE magic bytes
pub const BLTE_MAGIC: [u8; 4] = *b"BLTE";

/// Header flags for chunk table format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeaderFlags {
    /// Standard chunk info (24 bytes per chunk)
    Standard = 0x0F,
    /// Extended chunk info (40 bytes per chunk)
    Extended = 0x10,
}

impl HeaderFlags {
    /// Parse from byte value
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x0F => Some(Self::Standard),
            0x10 => Some(Self::Extended),
            _ => None,
        }
    }

    /// Get the size of each chunk info entry
    pub fn chunk_info_size(&self) -> usize {
        match self {
            Self::Standard => 24,
            Self::Extended => 40,
        }
    }
}

/// BLTE file header
#[derive(Debug, Clone)]
pub struct BlteHeader {
    /// Magic bytes (always "BLTE")
    pub magic: [u8; 4],
    /// Header size (0 = single chunk, >0 = multi-chunk with extended header)
    pub header_size: u32,
    /// Extended header (present when `header_size` > 0)
    pub extended: Option<ExtendedHeader>,
}

impl BinRead for BlteHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read magic
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if magic != BLTE_MAGIC {
            return Err(binrw::Error::Custom {
                pos: 0,
                err: Box::new(BlteError::InvalidMagic(magic)),
            });
        }

        // Read header size (big-endian)
        let header_size = u32::read_options(reader, binrw::Endian::Big, ())?;

        // Read extended header if present
        let extended = if header_size > 0 {
            Some(ExtendedHeader::read_options(reader, endian, ())?)
        } else {
            None
        };

        Ok(Self {
            magic,
            header_size,
            extended,
        })
    }
}

impl BinWrite for BlteHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write magic
        writer.write_all(&self.magic)?;

        // Write header size (big-endian)
        self.header_size
            .write_options(writer, binrw::Endian::Big, ())?;

        // Write extended header if present
        if let Some(ref extended) = self.extended {
            extended.write_options(writer, endian, ())?;
        }

        Ok(())
    }
}

impl BlteHeader {
    /// Create a header for a single-chunk file
    pub fn single_chunk() -> Self {
        Self {
            magic: BLTE_MAGIC,
            header_size: 0,
            extended: None,
        }
    }

    /// Create a header for a multi-chunk file (standard format)
    #[allow(clippy::cast_possible_truncation)]
    pub fn multi_chunk(chunks: &[ChunkData]) -> BlteResult<Self> {
        Self::multi_chunk_with_flags(chunks, HeaderFlags::Standard)
    }

    /// Create a header for a multi-chunk file with extended format
    #[allow(clippy::cast_possible_truncation)]
    pub fn multi_chunk_extended(chunks: &[ChunkData]) -> BlteResult<Self> {
        Self::multi_chunk_with_flags(chunks, HeaderFlags::Extended)
    }

    /// Create a header for a multi-chunk file with specified flags
    #[allow(clippy::cast_possible_truncation)]
    fn multi_chunk_with_flags(chunks: &[ChunkData], flags: HeaderFlags) -> BlteResult<Self> {
        if chunks.is_empty() {
            return Err(BlteError::InvalidChunkCount(0));
        }

        if chunks.len() > 0xFF_FFFF {
            // Safe cast: we just checked it's > 0xFFFFFF
            return Err(BlteError::InvalidChunkCount(chunks.len() as u32));
        }

        let chunk_infos: Vec<ChunkInfo> = if flags == HeaderFlags::Extended {
            chunks
                .iter()
                .map(ChunkInfo::from_chunk_data_extended)
                .collect()
        } else {
            chunks.iter().map(ChunkInfo::from_chunk_data).collect()
        };

        let extended = ExtendedHeader {
            flags,
            // Safe cast: we checked chunks.len() <= 0xFFFFFF above
            chunk_count: chunks.len() as u32,
            chunk_infos,
        };

        // Calculate header size: 8 (magic + header_size field) + 4 (flags + chunk_count) + entries
        // The on-disk header_size includes the 8-byte BLTE preamble
        let header_size = 12 + (chunks.len() * extended.flags.chunk_info_size());

        Ok(Self {
            magic: BLTE_MAGIC,
            // Safe cast: header size for 0xFFFFFF chunks won't exceed u32
            header_size: header_size as u32,
            extended: Some(extended),
        })
    }

    /// Get the number of chunks
    pub fn chunk_count(&self) -> usize {
        match &self.extended {
            Some(extended) => extended.chunk_count as usize,
            None => 1, // Single chunk
        }
    }

    /// Check if this is a single-chunk file
    pub fn is_single_chunk(&self) -> bool {
        self.header_size == 0
    }

    /// Get the data offset (where chunk data starts)
    pub fn data_offset(&self) -> usize {
        if self.is_single_chunk() {
            8 // Just magic + header_size
        } else {
            // header_size already includes the 8-byte preamble
            self.header_size as usize
        }
    }

    /// Get the total header size including magic and header_size fields
    pub fn total_header_size(&self) -> usize {
        if self.is_single_chunk() {
            8 // magic (4) + header_size (4)
        } else {
            // header_size already includes the 8-byte preamble
            self.header_size as usize
        }
    }
}

/// Extended header for multi-chunk files
#[derive(Debug, Clone, BinRead)]
#[br(big)]
#[allow(clippy::cast_possible_truncation)]
pub struct ExtendedHeader {
    /// Flags indicating chunk info format
    #[br(map = |x: u8| HeaderFlags::from_byte(x).expect("valid header flags byte"))]
    pub flags: HeaderFlags,

    /// 24-bit chunk count (big-endian)
    #[br(map = |x: [u8; 3]| u32::from_be_bytes([0, x[0], x[1], x[2]]))]
    pub chunk_count: u32,

    /// Chunk information table
    #[br(count = chunk_count, args { inner: (flags,) })]
    pub chunk_infos: Vec<ChunkInfo>,
}

impl BinWrite for ExtendedHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write flags
        writer.write_all(&[self.flags as u8])?;

        // Write 24-bit chunk count (big-endian)
        #[allow(clippy::cast_possible_truncation)]
        let chunk_count_bytes = [
            (self.chunk_count >> 16) as u8,
            (self.chunk_count >> 8) as u8,
            self.chunk_count as u8,
        ];
        writer.write_all(&chunk_count_bytes)?;

        // Write chunk infos with proper flags
        for chunk_info in &self.chunk_infos {
            chunk_info.write_options(writer, binrw::Endian::Big, (self.flags,))?;
        }

        Ok(())
    }
}

/// Chunk information (24 bytes standard, 40 bytes extended)
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    /// Compressed size
    pub compressed_size: u32,
    /// Decompressed size
    pub decompressed_size: u32,
    /// MD5 checksum of compressed data
    pub checksum: [u8; 16],
    /// MD5 checksum of decompressed data (extended format only)
    pub decompressed_checksum: Option<[u8; 16]>,
}

impl BinRead for ChunkInfo {
    type Args<'a> = (HeaderFlags,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let (flags,) = args;

        // Read sizes (big-endian)
        let compressed_size = u32::read_options(reader, binrw::Endian::Big, ())?;
        let decompressed_size = u32::read_options(reader, binrw::Endian::Big, ())?;

        // Read checksum of compressed data
        let mut checksum = [0u8; 16];
        reader.read_exact(&mut checksum)?;

        // Read decompressed checksum if extended format
        let decompressed_checksum = if flags == HeaderFlags::Extended {
            let mut decompressed_checksum = [0u8; 16];
            reader.read_exact(&mut decompressed_checksum)?;
            Some(decompressed_checksum)
        } else {
            None
        };

        Ok(Self {
            compressed_size,
            decompressed_size,
            checksum,
            decompressed_checksum,
        })
    }
}

impl BinWrite for ChunkInfo {
    type Args<'a> = (HeaderFlags,);

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let (flags,) = args;

        // Write sizes (big-endian)
        self.compressed_size
            .write_options(writer, binrw::Endian::Big, ())?;
        self.decompressed_size
            .write_options(writer, binrw::Endian::Big, ())?;

        // Write checksum of compressed data
        writer.write_all(&self.checksum)?;

        // Write decompressed checksum if extended format
        if flags == HeaderFlags::Extended {
            if let Some(decompressed_checksum) = self.decompressed_checksum {
                writer.write_all(&decompressed_checksum)?;
            } else {
                // If no decompressed checksum provided, write zeros
                writer.write_all(&[0u8; 16])?;
            }
        }

        Ok(())
    }
}

impl ChunkInfo {
    /// Create from chunk data (standard format)
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_chunk_data(chunk: &ChunkData) -> Self {
        use cascette_crypto::md5::ContentKey;

        let compressed = chunk.compressed_data();
        let key = ContentKey::from_data(&compressed);
        let checksum = *key.as_bytes();

        Self {
            // Safe cast: BLTE chunks are limited to reasonable sizes
            compressed_size: compressed.len() as u32,
            decompressed_size: chunk.decompressed_size() as u32,
            checksum,
            decompressed_checksum: None,
        }
    }

    /// Create from chunk data with extended format (includes decompressed checksum)
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_chunk_data_extended(chunk: &ChunkData) -> Self {
        use cascette_crypto::md5::ContentKey;

        let compressed = chunk.compressed_data();
        let compressed_key = ContentKey::from_data(&compressed);
        let checksum = *compressed_key.as_bytes();

        // Calculate decompressed data checksum
        let decompressed = chunk.decompress(0).unwrap_or_else(|_| compressed.clone());
        let decompressed_key = ContentKey::from_data(&decompressed);
        let decompressed_checksum = Some(*decompressed_key.as_bytes());

        Self {
            // Safe cast: BLTE chunks are limited to reasonable sizes
            compressed_size: compressed.len() as u32,
            decompressed_size: chunk.decompressed_size() as u32,
            checksum,
            decompressed_checksum,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_header_flags() {
        assert_eq!(HeaderFlags::from_byte(0x0F), Some(HeaderFlags::Standard));
        assert_eq!(HeaderFlags::from_byte(0x10), Some(HeaderFlags::Extended));
        assert_eq!(HeaderFlags::from_byte(0xFF), None);

        assert_eq!(HeaderFlags::Standard.chunk_info_size(), 24);
        assert_eq!(HeaderFlags::Extended.chunk_info_size(), 40);
    }

    #[test]
    fn test_single_chunk_header() {
        let header = BlteHeader::single_chunk();
        assert_eq!(header.magic, BLTE_MAGIC);
        assert_eq!(header.header_size, 0);
        assert!(header.extended.is_none());
        assert!(header.is_single_chunk());
        assert_eq!(header.chunk_count(), 1);
        assert_eq!(header.data_offset(), 8);
    }

    #[test]
    fn test_extended_chunk_format() {
        use super::super::chunk::{ChunkData, CompressionMode};

        // Create test chunks
        let chunk_a = ChunkData::new(vec![1, 2, 3, 4], CompressionMode::None)
            .expect("Test operation should succeed");
        let chunk_b = ChunkData::new(vec![5, 6, 7, 8], CompressionMode::None)
            .expect("Test operation should succeed");
        let chunks = vec![chunk_a, chunk_b];

        // Create header with extended format
        let header =
            BlteHeader::multi_chunk_extended(&chunks).expect("Test operation should succeed");
        assert_eq!(header.magic, BLTE_MAGIC);
        assert!(header.extended.is_some());

        let extended = header.extended.expect("Test operation should succeed");
        assert_eq!(extended.flags, HeaderFlags::Extended);
        assert_eq!(extended.chunk_count, 2);

        // Verify all chunk infos have decompressed checksums
        for chunk_info in &extended.chunk_infos {
            assert!(chunk_info.decompressed_checksum.is_some());
        }

        // Calculate expected header size for extended format
        // 8 (magic + header_size) + 4 (flags + chunk_count) + 2 chunks * 40 bytes per chunk
        let expected_header_size: u32 = 12 + (2 * 40);
        assert_eq!(header.header_size, expected_header_size);
    }

    #[test]
    fn test_extended_format_round_trip() {
        use binrw::io::Cursor;
        use binrw::{BinRead, BinWrite};

        // Create an extended header with test data
        let chunk_info = ChunkInfo {
            compressed_size: 100,
            decompressed_size: 200,
            checksum: [1; 16],
            decompressed_checksum: Some([2; 16]),
        };

        let extended_header = ExtendedHeader {
            flags: HeaderFlags::Extended,
            chunk_count: 1,
            chunk_infos: vec![chunk_info],
        };

        // Write to bytes
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        extended_header
            .write_be(&mut cursor)
            .expect("Test operation should succeed");

        // Read back
        let mut read_cursor = Cursor::new(&buffer);
        let read_header =
            ExtendedHeader::read(&mut read_cursor).expect("Test operation should succeed");

        assert_eq!(read_header.flags, HeaderFlags::Extended);
        assert_eq!(read_header.chunk_count, 1);
        assert_eq!(read_header.chunk_infos.len(), 1);

        let read_chunk = &read_header.chunk_infos[0];
        assert_eq!(read_chunk.compressed_size, 100);
        assert_eq!(read_chunk.decompressed_size, 200);
        assert_eq!(read_chunk.checksum, [1; 16]);
        assert_eq!(read_chunk.decompressed_checksum, Some([2; 16]));
    }
}
