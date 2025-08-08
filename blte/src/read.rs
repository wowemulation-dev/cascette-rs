use crate::{BLTEHeader, ChunkEncodingHeader, ChunkInfo, Error, Result};
use md5::{Digest, Md5 as Md5Hasher};
use std::io::{BufRead, Seek, SeekFrom, Write};
use tracing::{error, warn};

/// Information about a compressed block's encoding
#[derive(Debug, PartialEq)]
pub struct ChunkEncodingInfo {
    /// How the block is encoded.
    pub encoding: ChunkEncodingHeader,

    /// Where the chunk is located within the file.
    pub info: ChunkInfo,
}

/// BLTE payload parser.
pub struct BLTEFile<T> {
    /// File handle.
    f: T,

    /// Offset of the start of the BLTE stream within `f`.
    offset: u64,

    /// Length of the BLTE stream.
    length: u64,

    /// BLTE stream header.
    header: BLTEHeader,
}

const EXTRACTOR_BUFFER_SIZE: usize = 8192;
const BUF_SIZE_U32: u32 = EXTRACTOR_BUFFER_SIZE as u32;
const BUF_SIZE_U64: u64 = EXTRACTOR_BUFFER_SIZE as u64;

impl<T> BLTEFile<T> {
    /// The header of the BLTE stream.
    pub fn header(&self) -> &BLTEHeader {
        &self.header
    }

    /// Returns `true` if the BLTE stream has chunk-level checksums, which can
    /// be verified with [`BlteExtractor::verify_compressed_checksum`].
    pub fn has_chunk_level_checksums(&self) -> bool {
        self.header
            .chunks()
            .iter()
            .all(|i| i.compressed_hash.is_some())
    }

    /// Get the number of chunks in the stream.
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.header.chunks().len()
    }
}

impl<T: BufRead + Seek> BLTEFile<T> {
    /// Parse a BLTE stream at `offset`.
    ///
    /// This is designed to work directly with complete `/tpr/{product}/data/`
    /// blobs (where there are multiple BLTE streams in a single file), but can
    /// also work on a file with a single BLTE stream.
    ///
    /// # Arguments
    ///
    /// * `offset`: byte offset of the start of the BLTE stream within `f`.
    ///
    ///   If `f` contains a single BLTE stream at the start of the file, set
    ///   this to `0`.
    ///
    /// * `size`: length of the BLTE stream within `f`.
    pub fn new(mut f: T, offset: u64, length: u64) -> Result<Self> {
        f.seek(SeekFrom::Start(offset))?;
        let header = BLTEHeader::parse(&mut f, length)?;

        Ok(Self {
            f,
            header,
            offset,
            length,
        })
    }

    /// Reads the chunk encoding header, and leaves the file's position at the
    /// first byte of the payload.
    pub fn read_chunk_header(&mut self, chunk: usize) -> Result<ChunkEncodingInfo> {
        let Some(info) = self.header.get_chunk(chunk).cloned() else {
            return Err(Error::ChunkIndexOutOfRange(chunk, 1));
        };

        if info.compressed_offset + info.compressed_size > self.length {
            // This is also checked by `BLTEHeader`
            error!(
                "Block {chunk} is out of range: {} + {} > {}",
                info.compressed_offset, info.compressed_size, self.length,
            );
            return Err(Error::TruncatedData {
                expected: info.compressed_offset + info.compressed_size,
                actual: self.length,
            });
        }

        self.f
            .seek(SeekFrom::Start(self.offset + info.compressed_offset))?;
        let encoding = ChunkEncodingHeader::parse(&mut self.f)?;
        Ok(ChunkEncodingInfo { encoding, info })
    }

    /// Extracts chunks to a file.
    ///
    /// Chunk data may be encrypted or compressed. An uncompressed size _may_
    /// be available in [`BlteHeader::total_decompressed_size()`].
    ///
    /// Compressed data will be automatically decompressed.
    ///
    /// This does not verify checksums during extraction. Those can be verified
    /// with [`BlteExtractor::verify_compressed_checksum`].
    ///
    /// To extract data to RAM, pass a [`std::io::Cursor`][] to this function.
    pub fn write_to_file<W: Write>(&mut self, mut file: W) -> Result<W> {
        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];

        for block in 0..self.header.chunks().len() {
            let header = self.read_chunk_header(block)?;

            // Position in the block, skip the headers
            let mut p = header.encoding.len() as u64;

            match header.encoding {
                ChunkEncodingHeader::None => {
                    // Directly copy the contents
                    while p < header.info.compressed_size {
                        let read_size = (header.info.compressed_size - p).min(BUF_SIZE_U64);

                        self.f.read_exact(&mut buf[0..read_size as usize])?;
                        file.write_all(&buf[0..read_size as usize])?;
                        p += read_size;
                    }
                }
                ChunkEncodingHeader::ZLib => {
                    let mut decompressor = flate2::write::ZlibDecoder::new(file);
                    while p < header.info.compressed_size {
                        let read_size = (header.info.compressed_size - p).min(BUF_SIZE_U64);
                        self.f.read_exact(&mut buf[0..read_size as usize])?;
                        decompressor.write_all(&buf[0..read_size as usize])?;
                        p += read_size;
                    }

                    file = decompressor.finish()?;
                }
                ChunkEncodingHeader::Lz4hc(_) => {
                    // TODO: lz4_flex API wants to own the read file handle
                    // while decompressing, which won't work when we only have a
                    // mutable reference to it.
                    //
                    // By comparison, flate2 can own
                    // the write file handle, and we can just push in more
                    // compressed buffer.
                    //
                    // We could set an extra Default trait bound, but that
                    // doesn't work with actual file handles. The work-around
                    // will likely involve some hacks.
                    //
                    // However, I can't locate an LZ4HC compressed file to test
                    // this stuff against, so this is pretty moot.
                    todo!();
                }
                ChunkEncodingHeader::Encrypted(_) => todo!(),
                ChunkEncodingHeader::Frame => todo!(),
            }
        }

        Ok(file)
    }

    /// Verify the checksum of compressed data.
    ///
    /// Returns [`Error::ChecksumMismatch`][] on checksum failures.
    ///
    /// Returns `Ok(())` if block-level checksums are valid **OR** there are no
    /// block-level checksums. [`BlteExtractor::has_block_level_checksums`] will
    /// return `false` for streams without block-level checksums.
    pub fn verify_compressed_checksum(&mut self) -> Result<()> {
        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];
        for header in self.header.chunks() {
            let Some(expected) = header.compressed_hash.as_ref() else {
                // Single stream file with no hash
                return Ok(());
            };

            let mut hasher = Md5Hasher::new();
            self.f
                .seek(SeekFrom::Start(self.offset + header.compressed_offset))?;
            let mut p = 0u64;

            while p < header.compressed_size {
                let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);
                self.f.read_exact(&mut buf[0..read_size as usize])?;
                hasher.update(&buf[0..read_size as usize]);
                p += read_size;
            }

            let result = hasher.finalize();
            if !result.starts_with(expected) {
                warn!(
                    "MD5 mismatch: {} != {}",
                    hex::encode(result),
                    hex::encode(expected),
                );
                return Err(Error::ChecksumMismatch {
                    expected: expected.to_vec(),
                    actual: result.into(),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;

    fn create_single_chunk_blte() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        data.extend_from_slice(b"N"); // No compression
        data.extend_from_slice(b"Hello, BLTE!"); // Payload
        data
    }

    fn create_multi_chunk_blte() -> Vec<u8> {
        let chunk1_data = b"NHello";
        let chunk2_data = b"N, BLTE!";

        let mut data = Vec::new();

        // Calculate header size: 4 (magic) + 4 (header_size) + 1 (flags) + 3 (chunk_count) + 2 * 24 (chunk_info)
        let header_size = 8 + 1 + 3 + 2 * 24; // = 60

        // Header
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&(header_size as u32).to_be_bytes());

        // Chunk table
        data.push(0x0F); // Standard flags
        data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks

        // Chunk 1 info
        data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes()); // Compressed size
        data.extend_from_slice(&5u32.to_be_bytes()); // Decompressed: "Hello"
        data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk 2 info
        data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes()); // Compressed size
        data.extend_from_slice(&7u32.to_be_bytes()); // Decompressed: ", BLTE!"
        data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk data
        data.extend_from_slice(chunk1_data); // Chunk 1
        data.extend_from_slice(chunk2_data); // Chunk 2

        data
    }

    #[test]
    fn test_single_chunk_file() {
        let data = create_single_chunk_blte();
        let mut buf = Cursor::new(&data);
        let mut blte_file = BLTEFile::new(&mut buf, 0, data.len() as u64).unwrap();

        assert!(!blte_file.has_chunk_level_checksums());
        assert_eq!(blte_file.chunk_count(), 1);

        let chunk = blte_file.read_chunk_header(0).unwrap();
        assert_eq!(chunk.info.compressed_size, 13);
        assert_eq!(chunk.info.decompressed_size, 0); // Unknown for single chunk
        assert!(matches!(chunk.encoding, ChunkEncodingHeader::None));

        let mut data = [0; 12];
        blte_file.f.read_exact(&mut data).unwrap();
        assert_eq!(&data, b"Hello, BLTE!");
    }

    #[test]
    fn test_multi_chunk_file() {
        let data = create_multi_chunk_blte();
        let mut buf = Cursor::new(&data);
        let mut blte_file = BLTEFile::new(&mut buf, 0, data.len() as u64).unwrap();

        assert!(blte_file.has_chunk_level_checksums());
        assert_eq!(blte_file.chunk_count(), 2);

        // Test chunk 0
        let chunk0 = blte_file.read_chunk_header(0).unwrap();
        assert_eq!(chunk0.info.compressed_size, 6);
        assert_eq!(chunk0.info.decompressed_size, 5);
        assert!(matches!(chunk0.encoding, ChunkEncodingHeader::None));

        let mut data = [0; 5];
        blte_file.f.read_exact(&mut data).unwrap();
        assert_eq!(&data, b"Hello");

        // Test chunk 1
        let chunk1 = blte_file.read_chunk_header(1).unwrap();
        assert_eq!(chunk1.info.compressed_size, 8);
        assert_eq!(chunk1.info.decompressed_size, 7);
        assert!(matches!(chunk1.encoding, ChunkEncodingHeader::None));

        let mut data = [0; 7];
        blte_file.f.read_exact(&mut data).unwrap();
        assert_eq!(&data, b", BLTE!");
    }

    #[test]
    fn test_invalid_chunk_index() {
        let data = create_single_chunk_blte();
        let mut buf = Cursor::new(&data);
        let mut blte_file = BLTEFile::new(&mut buf, 0, data.len() as u64).unwrap();

        let err = blte_file.read_chunk_header(1).unwrap_err();
        assert!(
            matches!(err, Error::ChunkIndexOutOfRange(1, 1)),
            "actual: {err:?}",
        );
    }
}
