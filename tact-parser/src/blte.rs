//! [BLTE][0] archive parser/extractor.
//!
//! [0]: https://wowdev.wiki/BLTE

use crate::{
    Error, MD5_LENGTH, Md5, Result,
    ioutils::{AsyncReadInt, ReadInt},
};
use md5::{Digest, Md5 as Md5Hasher};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tracing::*;

const BLTE_MAGIC: &[u8; 4] = b"BLTE";

/// [BLTE][0] archive header / metadata.
///
/// [0]: https://wowdev.wiki/BLTE
#[derive(Debug, PartialEq, Eq)]
pub struct BlteHeader {
    /// Length of the BLTE headers, in bytes.
    length: u32,

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Set to 0 if unknown.
    total_decompressed_size: u64,

    /// Block info.
    ///
    /// When **not** present, the remainder of the BLTE stream contains a single
    /// block.
    block_info: Option<Vec<BlteBlockInfo>>,
}

/// [BLTE][0] archive block info/metadata.
///
/// [0]: https://wowdev.wiki/BLTE
#[derive(Debug, PartialEq, Eq)]
pub struct BlteBlockInfo {
    /// The compressed size of the block, including block header byte(s).
    compressed_size: u32,

    /// The decompressed size of the block.
    ///
    /// For non-compressed blocks, this is `compressed_size - 1` (for the block
    /// header byte).
    decompressed_size: u32,

    /// The MD5 checksum of the compressed block, including header byte(s).
    ///
    /// Can be verified with [`BlteExtractor::verify_compressed_checksum`][].
    compressed_hash: Md5,

    /// The MD5 checksum of the block when decompressed.
    ///
    /// Only present for table format `0x10`.
    decompressed_hash: Option<Md5>,

    /// Offset of this data block, relative to the start of the header.
    compressed_offset: u64,

    /// Offset of this data block when decompressed, relative to the start of
    /// the decompressed file.
    decompressed_offset: u64,
}

impl BlteHeader {
    /// Parse a BLTE header at the file's current position.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mut magic = [0; BLTE_MAGIC.len()];
        f.read_exact(&mut magic)?;
        if &magic != BLTE_MAGIC {
            return Err(Error::BadMagic);
        }

        let length = f.read_u32be()?;
        if length <= 8 + 4 + 24 {
            // We couldn't fit a BlockInfo entry
            return Ok(BlteHeader {
                block_info: None,
                length: length.max(8),
                total_decompressed_size: 0,
            });
        }

        if length > 65535 {
            // Probably invalid, number is arbitrary.
            error!("huge BLTE header: {length} bytes");
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
        let blocks_len = length - 8 - 4;
        let block_len = if has_uncompressed_hash { 40 } else { 24 };

        if blocks_len != num_blocks * block_len {
            error!(
                "Invalid length for blocks data: got {blocks_len}, expected {num_blocks} * {block_len} ({})",
                num_blocks * block_len,
            );
            return Err(Error::FailedPrecondition);
        }

        let mut block_info = Vec::with_capacity(num_blocks as usize);
        let mut compressed_offset = u64::from(length);
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
                compressed_offset,
                decompressed_offset,
            });

            decompressed_offset += u64::from(decompressed_size);
            compressed_offset += u64::from(compressed_size);
        }

        Ok(Self {
            block_info: Some(block_info),
            length,
            total_decompressed_size: decompressed_offset,
        })
    }

    /// Parse a BLTE header at the file's current position.
    pub async fn aparse<R: AsyncReadInt + AsyncReadExt + Unpin>(f: &mut R) -> Result<Self> {
        let mut magic = [0; BLTE_MAGIC.len()];
        f.read_exact(&mut magic).await?;
        if &magic != BLTE_MAGIC {
            return Err(Error::BadMagic);
        }

        let length = f.read_u32().await?;
        if length <= 8 + 4 + 24 {
            // We couldn't fit a BlockInfo entry
            return Ok(BlteHeader {
                block_info: None,
                length: length.max(8),
                total_decompressed_size: 0,
            });
        }

        if length > 65535 {
            // Probably invalid, number is arbitrary.
            error!("huge BLTE header: {length} bytes");
            return Err(Error::FailedPrecondition);
        }

        // We have a ChunkInfo struct with 1 or more BlockInfos
        let table_format = f.read_u8().await?;
        if table_format != 0xf && table_format != 0x10 {
            error!("unknown BLTE ChunkInfo format: {table_format:#x}");
            return Err(Error::NotImplemented);
        }
        let has_uncompressed_hash = table_format == 0x10;
        let num_blocks = f.read_u24().await?;

        // Is the header the correct size for the expected number of blocks?
        let blocks_len = length - 8 - 4;
        let block_len = if has_uncompressed_hash { 40 } else { 24 };

        if blocks_len != num_blocks * block_len {
            error!(
                "Invalid length for blocks data: got {blocks_len}, expected {num_blocks} * {block_len} ({})",
                num_blocks * block_len,
            );
            return Err(Error::FailedPrecondition);
        }

        let mut block_info = Vec::with_capacity(num_blocks as usize);
        let mut compressed_offset = u64::from(length);
        let mut decompressed_offset = 0;
        for _ in 0..num_blocks {
            let compressed_size = f.read_u32().await?;
            let decompressed_size = f.read_u32().await?;

            let mut compressed_hash = [0; MD5_LENGTH];
            f.read_exact(&mut compressed_hash).await?;

            let decompressed_hash = if has_uncompressed_hash {
                let mut hash = [0; MD5_LENGTH];
                f.read_exact(&mut hash).await?;
                Some(hash)
            } else {
                None
            };

            block_info.push(BlteBlockInfo {
                compressed_size,
                decompressed_size,
                compressed_hash,
                decompressed_hash,
                compressed_offset,
                decompressed_offset,
            });

            decompressed_offset += u64::from(decompressed_size);
            compressed_offset += u64::from(compressed_size);
        }

        Ok(Self {
            block_info: Some(block_info),
            length,
            total_decompressed_size: decompressed_offset,
        })
    }

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Returns 0 if unknown.
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_decompressed_size
    }

    pub const fn block_count(&self) -> usize {
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
            return Some(self.length.into());
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

    /// Parses an encrypted block header asynchronously.
    pub async fn aparse<R: AsyncReadExt + Unpin>(f: &mut R) -> Result<Self> {
        let key_name_length = f.read_u8().await?;
        let mut key_name = vec![0; key_name_length as usize];
        f.read_exact(&mut key_name).await?;

        let iv_length = f.read_u8().await?;
        let mut iv = vec![0; iv_length as usize];
        f.read_exact(&mut iv).await?;

        Ok(Self { key_name, iv })
    }

    /// Length of the [`EncryptedBlockHeader`] on disk, including length
    /// prefixes.
    pub fn len(&self) -> usize {
        self.key_name.len() + self.iv.len() + 2
    }

    /// `true` if the [`EncryptedBlockHeader`] would take up 0 bytes.
    ///
    /// This is always `false`.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
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
            other => return Err(Error::UnsupportedBlteEncoding(other)),
        })
    }

    /// Parses an block encoding header asynchronously.
    pub async fn aparse<R: AsyncReadExt + Unpin>(f: &mut R) -> Result<Self> {
        let mode = f.read_u8().await?;

        Ok(match mode {
            b'N' => Self::None,
            b'Z' => Self::Zlib,
            b'4' => Self::Lz4hc,
            b'E' => Self::Encrypted(EncryptedBlockHeader::aparse(f).await?),
            other => return Err(Error::UnsupportedBlteEncoding(other)),
        })
    }

    /// Length of the encoding header on disk.
    pub fn len(&self) -> usize {
        1 + if let BlockEncoding::Encrypted(h) = self {
            h.len()
        } else {
            0
        }
    }

    /// `true` if the [`BlockEncoding`] would take up 0 bytes.
    ///
    /// This is always `false`.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
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
pub struct BlteExtractor<T> {
    /// File handle.
    f: T,

    /// Offset of the start of the BLTE stream within `f`.
    offset: u64,

    /// Length of the BLTE stream.
    length: u64,

    /// BLTE stream header.
    header: BlteHeader,
}

const EXTRACTOR_BUFFER_SIZE: usize = 8192;
const BUF_SIZE_U32: u32 = EXTRACTOR_BUFFER_SIZE as u32;
const BUF_SIZE_U64: u64 = EXTRACTOR_BUFFER_SIZE as u64;

impl<T> BlteExtractor<T> {
    /// The header of the BLTE stream.
    pub fn header(&self) -> &BlteHeader {
        &self.header
    }

    /// Returns `true` if the BLTE stream has block-level checksums, which can
    /// be verified with [`BlteExtractor::verify_compressed_checksum`].
    pub fn has_block_level_checksums(&self) -> bool {
        self.header.block_info.is_some()
    }
}

impl<T: BufRead + Seek> BlteExtractor<T> {
    /// Parse a BLTE chunk at `offset`.
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
        let header = BlteHeader::parse(&mut f)?;

        Ok(Self {
            f,
            header,
            offset,
            length,
        })
    }

    /// Reads the block encoding header, and leaves the file's position at the
    /// first byte of the payload.
    pub fn read_block_header(&mut self, block: usize) -> Result<BlockEncodingInfo> {
        let (off, size, doff, dsize) = if let Some(block_infos) = self.header.block_info.as_ref() {
            let block_info = block_infos.get(block).ok_or(Error::BlockIndexOutOfRange(
                block,
                self.header.block_count(),
            ))?;

            (
                block_info.compressed_offset,
                u64::from(block_info.compressed_size),
                block_info.decompressed_offset,
                block_info.decompressed_size,
            )
        } else {
            // No block infos
            if block != 0 {
                return Err(Error::BlockIndexOutOfRange(block, 1));
            }

            (
                u64::from(self.header.length),
                self.length - u64::from(self.header.length),
                0,
                0,
            )
        };

        if off + size > self.length {
            error!(
                "Block {block} is out of range: {off} + {size} > {}",
                self.length
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
    /// Compressed data will be automatically decompressed.
    ///
    /// This does not verify checksums during extraction. Those can be verified
    /// with [`BlteExtractor::verify_compressed_checksum`].
    ///
    /// To extract data to RAM, pass a [`std::io::Cursor`][] to this function.
    pub fn write_to_file<W: Write>(&mut self, mut file: W) -> Result<W> {
        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];

        for block in 0..self.header.block_count() {
            let header = self.read_block_header(block)?;

            // Position in the block, skip the headers
            let mut p = header.encoding.len() as u64;

            match header.encoding {
                BlockEncoding::None => {
                    // Directly copy the contents
                    while p < header.compressed_size {
                        let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);

                        self.f.read_exact(&mut buf[0..read_size as usize])?;
                        file.write_all(&buf[0..read_size as usize])?;
                        p += read_size;
                    }
                }

                BlockEncoding::Zlib => {
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
}

impl<T: AsyncBufReadExt + AsyncSeekExt + Unpin + Send> BlteExtractor<T> {
    /// Parse a BLTE chunk at `offset`.
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
    pub async fn anew(mut f: T, offset: u64, length: u64) -> Result<Self> {
        f.seek(SeekFrom::Start(offset)).await?;
        let header = BlteHeader::aparse(&mut f).await?;

        Ok(Self {
            f,
            header,
            offset,
            length,
        })
    }

    /// Reads the block encoding header, and leaves the file's position at the
    /// first byte of the payload.
    pub async fn aread_block_header(&mut self, block: usize) -> Result<BlockEncodingInfo> {
        let (off, size, doff, dsize) = if let Some(block_infos) = self.header.block_info.as_ref() {
            let block_info = block_infos.get(block).ok_or(Error::BlockIndexOutOfRange(
                block,
                self.header.block_count(),
            ))?;

            (
                block_info.compressed_offset,
                u64::from(block_info.compressed_size),
                block_info.decompressed_offset,
                block_info.decompressed_size,
            )
        } else {
            // No block infos
            if block != 0 {
                return Err(Error::BlockIndexOutOfRange(block, 1));
            }

            (
                u64::from(self.header.length),
                self.length - u64::from(self.header.length),
                0,
                0,
            )
        };

        if off + size > self.length {
            error!(
                "Block {block} is out of range: {off} + {size} > {}",
                self.length
            );
            return Err(Error::FailedPrecondition);
        }

        self.f.seek(SeekFrom::Start(self.offset + off)).await?;
        let encoding = BlockEncoding::aparse(&mut self.f).await?;
        Ok(BlockEncodingInfo {
            encoding,
            compressed_size: size,
            decompressed_size: dsize,
            decompressed_offset: doff,
        })
    }

    /// Extracts blocks to a file (partially) asynchronously.
    ///
    /// Block data may be encrypted or compressed. An uncompressed size _may_
    /// be available in [`BlteHeader::total_decompressed_size()`].
    ///
    /// Compressed data will be automatically decompressed.
    ///
    /// This does not verify checksums during extraction. Those can be verified
    /// with [`BlteExtractor::verify_compressed_checksum`].
    ///
    /// To extract data to RAM, pass a [`std::io::Cursor`][] to this function.
    pub async fn awrite_to_file<W: AsyncWrite + Unpin>(&mut self, mut file: W) -> Result<W> {
        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];

        for block in 0..self.header.block_count() {
            let header = self.aread_block_header(block).await?;

            // Position in the block, skip the headers
            let mut p = header.encoding.len() as u64;

            match header.encoding {
                BlockEncoding::None => {
                    // Directly copy the contents
                    while p < header.compressed_size {
                        let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);

                        self.f.read_exact(&mut buf[0..read_size as usize]).await?;
                        file.write_all(&buf[0..read_size as usize]).await?;

                        p += read_size;
                    }
                }

                BlockEncoding::Zlib => {
                    let mut decompressor = async_compression::tokio::write::ZlibDecoder::new(file);
                    while p < header.compressed_size {
                        let read_size = (header.compressed_size - p).min(BUF_SIZE_U64);
                        self.f.read_exact(&mut buf[0..read_size as usize]).await?;
                        decompressor.write_all(&buf[0..read_size as usize]).await?;
                        p += read_size;
                    }

                    decompressor.shutdown().await?;
                    file = decompressor.into_inner();
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
    pub async fn averify_compressed_checksum(&mut self) -> Result<()> {
        let Some(block_infos) = self.header.block_info.as_ref() else {
            return Ok(());
        };

        let mut buf = [0; EXTRACTOR_BUFFER_SIZE];
        for header in block_infos {
            let mut hasher = Md5Hasher::new();
            self.f
                .seek(SeekFrom::Start(self.offset + header.compressed_offset))
                .await?;
            let mut p = 0u32;

            while p < header.compressed_size {
                let read_size = (header.compressed_size - p).min(BUF_SIZE_U32);
                self.f.read_exact(&mut buf[0..read_size as usize]).await?;
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
}
