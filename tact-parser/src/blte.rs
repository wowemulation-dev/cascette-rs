//! [BLTE][0] parser
//!
//! CDN archives contain multiple BLTE blobs, one for each file.
//!
//! [0]: https://wowdev.wiki/BLTE

use crate::{Error, MD5_LENGTH, Md5, Result, ioutils::ReadInt};
use md5::{Digest, Md5 as Md5Hasher};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use tracing::*;

const BLTE_MAGIC: &'static [u8; 4] = b"BLTE";

#[derive(Debug, PartialEq, Eq)]
pub struct BlteHeader {
    /// Offset of the first data block, relative to the start of the header.
    data_offset: u64,

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Set to 0 if unknown.
    total_decompressed_size: u64,

    block_info: Option<Vec<BlteBlockInfo>>,
}

/// Block info
#[derive(Debug, PartialEq, Eq)]
pub struct BlteBlockInfo {
    compressed_size: u32,
    decompressed_size: u32,
    compressed_hash: Md5,

    /// Only present for table format `0x10`.
    decompressed_hash: Option<Md5>,

    /// Offset of this data block, relative to the start of the header.
    compressed_offset: u64,

    /// Offset of this data block when decompressed, relative to the start of
    /// the decompressed file.
    decompressed_offset: u64,
}

impl BlteHeader {
    /// Parses a BLTE header.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mut magic = [0; BLTE_MAGIC.len()];
        f.read_exact(&mut magic)?;
        if &magic != BLTE_MAGIC {
            return Err(Error::BadMagic);
        }

        let header_len = f.read_u32be()?;
        if header_len <= 8 + 4 + 24 {
            // We couldn't fit a header entry
            return Ok(BlteHeader {
                block_info: None,
                data_offset: u64::from(header_len.max(8)),
                total_decompressed_size: 0,
            });
        }

        if header_len > 65535 {
            // Probably invalid, number is arbitrary.
            error!("huge BLTE header: {header_len} bytes");
            return Err(Error::FailedPrecondition);
        }

        // We have a ChunkInfo struct with 1 or more BlockInfos
        let table_format = f.read_u8()?;
        if table_format != 0xf && table_format != 0x10 {
            error!("unknown BLTE ChunkInfo format: {table_format:#x}");
            return Err(Error::NotImplemented);
        }
        let has_uncompressed_hash = table_format == 0x10;
        let num_blocks = f.read_u24be()?;

        // Is the header the correct size for the expected number of blocks?
        let blocks_len = header_len - 8 - 4;
        let block_len = if has_uncompressed_hash { 40 } else { 24 };

        if blocks_len != num_blocks * block_len {
            error!(
                "Invalid length for blocks data: got {blocks_len}, expected {num_blocks} * {block_len} ({})",
                num_blocks * block_len,
            );
            return Err(Error::FailedPrecondition);
        }

        let mut block_info = Vec::with_capacity(num_blocks as usize);
        let mut o = u64::from(header_len);
        let mut decompressed_offset = 0;
        for _ in 0..num_blocks {
            let compressed_size = f.read_u32be()?;
            let decompressed_size = f.read_u32be()?;

            let mut compressed_hash = [0; MD5_LENGTH];
            f.read_exact(&mut compressed_hash)?;

            let decompressed_hash = if has_uncompressed_hash {
                let mut hash = [0; MD5_LENGTH];
                f.read_exact(&mut hash)?;
                Some(hash)
            } else {
                None
            };

            block_info.push(BlteBlockInfo {
                compressed_size,
                decompressed_size,
                compressed_hash,
                decompressed_hash,
                compressed_offset: o,
                decompressed_offset,
            });

            decompressed_offset += u64::from(decompressed_size);
            o += u64::from(compressed_size);
        }

        Ok(Self {
            block_info: Some(block_info),
            data_offset: u64::from(header_len),
            total_decompressed_size: decompressed_offset,
        })
    }

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Set to 0 if unknown.
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_decompressed_size
    }

    pub fn block_count(&self) -> usize {
        if let Some(block_info) = self.block_info.as_ref() {
            block_info.len()
        } else {
            1
        }
    }

    /// Find the offset of `block`, relative to the start of the header block.
    ///
    /// Returns `None` if `block` is out of range.
    pub fn block_data_offset(&self, block: usize) -> Option<u64> {
        if block == 0 {
            return Some(self.data_offset);
        }

        Some(self.block_info.as_ref()?.get(block)?.compressed_offset)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EncryptedBlockHeader {
    key_name: Vec<u8>,
    iv: Vec<u8>,
}

impl EncryptedBlockHeader {
    /// Parses an encrypted block header.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let key_name_length = f.read_u8()?;
        let mut key_name = vec![0; key_name_length as usize];
        f.read_exact(&mut key_name)?;

        let iv_length = f.read_u8()?;
        let mut iv = vec![0; iv_length as usize];
        f.read_exact(&mut iv)?;

        Ok(Self { key_name, iv })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlockEncoding {
    /// encoding: `N`
    None,
    /// encoding: `Z`
    Zlib,
    /// encoding: `4`
    Lz4hc,
    /// encoding: `E`
    Encrypted(EncryptedBlockHeader),
}

impl BlockEncoding {
    /// Parses an block encoding header.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mode = f.read_u8()?;

        Ok(match mode {
            b'N' => Self::None,
            b'Z' => Self::Zlib,
            b'4' => Self::Lz4hc,
            b'E' => Self::Encrypted(EncryptedBlockHeader::parse(f)?),
            other => return Err(Error::UnsupportedBtleEncoding(other)),
        })
    }
}

/// Information about a compressed block's encoding
#[derive(Debug, PartialEq, Eq)]
pub struct BlockEncodingInfo {
    /// How the block is encoded.
    pub encoding: BlockEncoding,

    /// The position of this block in the decompressed file.
    pub decompressed_offset: u64,

    /// The compressed size of this block.
    pub compressed_size: u64,

    /// The decompressed size of this block, if known.
    pub decompressed_size: u32,
}

/// BLTE payload parser.
pub struct BlteExtractor<T: BufRead + Seek> {
    /// File handle
    f: T,
    offset: u64,
    size: u64,
    header: BlteHeader,
}

const EXTRACTOR_BUFFER_SIZE: usize = 4096;
const BUF_SIZE_U32: u32 = EXTRACTOR_BUFFER_SIZE as u32;
const BUF_SIZE_U64: u64 = EXTRACTOR_BUFFER_SIZE as u64;

impl<T: BufRead + Seek> BlteExtractor<T> {
    /// Parse a BLTE chunk at `offset`.
    ///
    /// This is designed to work directly with complete `/tpr/{product}/data/`
    /// blobs (where there are multiple BLTE streams in a single file), but can
    /// also work on a file with a single BLTE stream.
    pub fn new(mut f: T, offset: u64, size: u64) -> Result<Self> {
        f.seek(SeekFrom::Start(offset))?;
        let header = BlteHeader::parse(&mut f)?;

        Ok(Self {
            f,
            header,
            offset,
            size,
        })
    }

    pub fn header(&self) -> &BlteHeader {
        &self.header
    }

    /// Reads the block encoding header, and leaves the file's position at the
    /// first byte of the payload.
    pub fn read_block_header(&mut self, block: usize) -> Result<BlockEncodingInfo> {
        let (off, size, doff, dsize) = if let Some(block_infos) = self.header.block_info.as_ref() {
            let block_info = block_infos.get(block).ok_or(Error::BlockIndexOutOfRange(
                block as u64,
                self.header.block_count() as u64,
            ))?;

            (
                block_info.compressed_offset,
                u64::from(block_info.compressed_size),
                block_info.decompressed_offset,
                block_info.decompressed_size,
            )
        } else {
            (
                self.header.data_offset,
                self.size - self.header.data_offset,
                0,
                0,
            )
        };

        if off + size > self.size {
            error!(
                "Block {block} is out of range: {off} + {size} > {}",
                self.size
            );
            return Err(Error::FailedPrecondition);
        }

        self.f.seek(SeekFrom::Start(self.offset + off))?;
        let encoding = BlockEncoding::parse(&mut self.f)?;
        Ok(BlockEncodingInfo {
            encoding,
            compressed_size: size,
            decompressed_size: dsize,
            decompressed_offset: doff,
        })
    }

    /// Extracts blocks to a file.
    ///
    /// Block data may be encrypted or compressed. An uncompressed size _may_
    /// be available in [`BlteHeader::total_decompressed_size()`].
    ///
    /// When data is compressed,
    ///
    /// To extract data to RAM, pass a [`std::io::Cursor`][] to this function.
    pub fn write_to_file<W: Write>(&mut self, mut file: W) -> Result<W> {
        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];

        for block in 0..self.header.block_count() {
            let header = self.read_block_header(block)?;
            // TODO: checksums

            match header.encoding {
                BlockEncoding::None => {
                    // Directly copy the contents
                    let mut p = 0u64;

                    while p < header.compressed_size {
                        let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);

                        self.f.read_exact(&mut buf[0..read_size as usize])?;
                        file.write_all(&buf[0..read_size as usize])?;
                        p += read_size;
                    }
                }

                BlockEncoding::Zlib => {
                    let mut p = 0u64;
                    let mut decompressor = flate2::write::ZlibDecoder::new(file);
                    while p < header.compressed_size {
                        let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);
                        self.f.read_exact(&mut buf[0..read_size as usize])?;
                        decompressor.write_all(&buf[0..read_size as usize])?;
                        p += read_size;
                    }

                    file = decompressor.finish()?;
                }

                // TODO: implement lz4hc
                BlockEncoding::Lz4hc => return Err(Error::NotImplemented),

                // TODO: implement encrypted blobs
                BlockEncoding::Encrypted(_) => return Err(Error::NotImplemented),
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
        let Some(block_infos) = self.header.block_info.as_ref() else {
            return Ok(());
        };

        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];
        for header in block_infos {
            let mut hasher = Md5Hasher::new();
            self.f
                .seek(SeekFrom::Start(self.offset + header.compressed_offset))?;
            let mut p = 0u32;

            while p < header.compressed_size {
                let read_size = (header.compressed_size - p).min(BUF_SIZE_U32);
                self.f.read_exact(&mut buf[0..read_size as usize])?;
                hasher.update(&buf[0..read_size as usize]);
                p += read_size;
            }

            let result = hasher.finalize();
            if !result.starts_with(&header.compressed_hash) {
                warn!(
                    "MD5 mismatch: {} != {}",
                    hex::encode(result),
                    hex::encode(header.compressed_hash),
                );
                return Err(Error::ChecksumMismatch);
            }
        }

        Ok(())
    }

    /// Returns `true` if the BLTE stream has block-level checksums, which can
    /// be verified with [`BlteExtractor::verify_compressed_checksum`].
    pub fn has_block_level_checksums(&self) -> bool {
        self.header.block_info.is_some()
    }
}
