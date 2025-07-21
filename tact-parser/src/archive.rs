//! Parsing archives and [archive indexes][1].
//!
//! [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)

use crate::{Error, Md5, Result};
use md5::{Digest, Md5 as Md5Hasher};
use std::{
    io::{BufRead, Cursor, Read, Seek, SeekFrom},
    iter::repeat_n,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};
use tracing::*;

/// [Archive index][1] footer parser.
///
/// [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
#[derive(Debug, PartialEq, Eq)]
pub struct ArchiveIndexFooter {
    toc_hash: Vec<u8>,
    format_revision: u8,
    flags0: u8,
    flags1: u8,
    block_size_bytes: u32,
    offset_bytes: u8,
    size_bytes: u8,
    key_bytes: u8,
    hash_bytes: u8,
    /// Number of index entries in the file.
    num_elements: u32,
    // /// Number of blocks in the file, excluding the TOC.
    // num_blocks: u64,
    footer_offset: u64,
}

impl ArchiveIndexFooter {
    const MIN_HASH_BYTES: u8 = 0x8;
    const MAX_HASH_BYTES: u8 = 0x10;

    const MAX_FOOTER_SIZE_U16: u16 = Self::size(Self::MAX_HASH_BYTES);
    const MAX_FOOTER_SIZE: usize = Self::MAX_FOOTER_SIZE_U16 as usize;

    /// Size of the footer given `hash_bytes`.
    pub const fn size(hash_bytes: u8) -> u16 {
        12 + ((hash_bytes as u16) * 2)
    }

    /// Parses an archive index footer.
    pub fn parse<R: Read + Seek>(f: &mut R, hash: &Md5) -> Result<Self> {
        let mut footer_buf = [0; Self::MAX_FOOTER_SIZE];
        let earliest_footer_point = f.seek(SeekFrom::End(-(Self::MAX_FOOTER_SIZE_U16 as i64)))?;
        f.read_exact(&mut footer_buf)?;
        let (hash_bytes, footer) = Self::find_footer(&footer_buf, hash)?;
        Self::parse_inner(earliest_footer_point, hash_bytes, footer)
    }

    /// Parses an archive index footer asynchronously.
    pub async fn aparse<R: AsyncRead + AsyncSeek + Unpin>(f: &mut R, hash: &Md5) -> Result<Self> {
        let mut footer_buf = [0; Self::MAX_FOOTER_SIZE];
        let earliest_footer_point = f
            .seek(SeekFrom::End(-(Self::MAX_FOOTER_SIZE_U16 as i64)))
            .await?;
        f.read_exact(&mut footer_buf).await?;
        let (hash_bytes, footer) = Self::find_footer(&footer_buf, hash)?;
        Self::parse_inner(earliest_footer_point, hash_bytes, footer)
    }

    fn parse_inner(earliest_footer_point: u64, hash_bytes: u8, footer: &[u8]) -> Result<Self> {
        // Find where the footer actually finishes
        let footer_offset =
            earliest_footer_point + (Self::MAX_FOOTER_SIZE_U16 as u64) - (footer.len() as u64);

        // Check that the hash_bytes is the same as what's in the data structure
        let hash_bytes_usize = usize::from(hash_bytes);
        let expected_hash_bytes = footer[hash_bytes_usize + 7];
        if expected_hash_bytes != hash_bytes {
            error!("hash_bytes field mismatch: {expected_hash_bytes} != {hash_bytes}");
            return Err(Error::FailedPrecondition);
        }

        let toc_hash = footer[..hash_bytes_usize].to_vec();
        let format_revision = footer[hash_bytes_usize];
        if format_revision != 1 {
            error!("unknown format_revision: {format_revision}");
            return Err(Error::NotImplemented);
        }

        // Check that the footer_hash would match
        let expected_footer_hash = &footer[footer.len() - hash_bytes_usize..];
        let mut hasher = Md5Hasher::new();
        hasher.update(&footer[hash_bytes_usize..footer.len() - hash_bytes_usize]);
        let nul = Vec::from_iter(repeat_n(0, hash_bytes_usize));
        hasher.update(&nul);
        let actual_footer_hash = hasher.finalize();

        if !actual_footer_hash.starts_with(expected_footer_hash) {
            error!(
                "footer_hash mismatch: {} != {}",
                hex::encode(&actual_footer_hash[..]),
                hex::encode(expected_footer_hash),
            );
            return Err(Error::ChecksumMismatch);
        }

        let offset_bytes = footer[hash_bytes_usize + 4];
        if usize::from(offset_bytes) > size_of::<u64>() {
            error!("offset_bytes > {}", size_of::<u64>());
            return Err(Error::FailedPrecondition);
        }

        let size_bytes = footer[hash_bytes_usize + 5];
        if usize::from(size_bytes) > size_of::<u64>() {
            error!("size_bytes > {}", size_of::<u64>());
            return Err(Error::FailedPrecondition);
        }

        let block_size_bytes = u32::from(footer[hash_bytes_usize + 3]) << 10;
        if block_size_bytes == 0 {
            error!("Archive index block size cannot be 0");
            return Err(Error::FailedPrecondition);
        }
        // Huge TOCs pick the wrong spot for this
        // let num_blocks = footer_offset / block_size_bytes;

        Ok(Self {
            toc_hash,
            format_revision,
            flags0: footer[hash_bytes_usize + 1],
            flags1: footer[hash_bytes_usize + 2],
            block_size_bytes,
            offset_bytes,
            size_bytes,
            key_bytes: footer[hash_bytes_usize + 6],
            hash_bytes,
            num_elements: u32::from_le_bytes(
                footer[hash_bytes_usize + 8..hash_bytes_usize + 12]
                    .try_into()
                    .unwrap(),
            ),
            footer_offset,
            // num_blocks,
        })
    }

    /// Find the `hash_bytes` and footer from a buffer with unknown length.
    ///
    /// `hash_bytes` is unknown, and the client normally guesses it by trying
    /// different values from `0x10` down to `0x8`
    ///
    /// This tries to find a structure size where the MD5 of the footer is
    /// `hash`.
    ///
    /// On success, returns (`hash_bytes`, `footer`).
    fn find_footer<'a>(
        footer_buf: &'a [u8; Self::MAX_FOOTER_SIZE],
        hash: &Md5,
    ) -> Result<(u8, &'a [u8])> {
        for hash_bytes in (Self::MIN_HASH_BYTES..=Self::MAX_HASH_BYTES).rev() {
            let footer_len = usize::from(Self::size(hash_bytes));

            let mut hasher = Md5Hasher::new();
            let footer = &footer_buf[footer_buf.len() - footer_len..];
            hasher.update(footer);
            let result = hasher.finalize();
            if &result[..] == hash {
                return Ok((hash_bytes, footer));
            }
        }

        error!("no matching hash for footer");
        Err(Error::FailedPrecondition)
    }

    /// Number of index entries in the file.
    pub fn num_elements(&self) -> u32 {
        self.num_elements
    }

    // /// Number of blocks in the file, excluding the TOC.
    // pub fn num_blocks(&self) -> u64 {
    //     self.num_blocks
    // }
}

/// [Archive index][1] TOC parser.
///
/// [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
#[derive(Default, PartialEq, Eq)]
pub struct ArchiveIndexToc {
    /// The last EKey of each block.
    pub last_ekey: Vec<Vec<u8>>,

    /// Partial MD5 checksum of the block.
    block_partial_md5: Vec<Vec<u8>>,

    /// Number of blocks in the TOC
    pub num_blocks: usize,
}

impl ArchiveIndexToc {
    /// Parses an archive index TOC.
    pub fn parse<R: Read + Seek>(f: &mut R, footer: &ArchiveIndexFooter) -> Result<Self> {
        // The TOC might be larger than a page, so we need to find it by looking
        // at the MD5.
        // Example: e353ca95b78f9ead4290b49c65a19d63

        let mut hasher = Md5Hasher::new();
        let mut buf = Vec::new();
        let max_blocks = usize::try_from(footer.footer_offset / u64::from(footer.block_size_bytes))
            .map_err(|_| Error::ArchiveIndexTocTooLarge)?;
        let mut num_blocks = 0;
        for estimated_num_blocks in (max_blocks / 2..=max_blocks).rev() {
            // read the last bit of the file in, minus headers
            let estimated_toc_start =
                (estimated_num_blocks as u64) * u64::from(footer.block_size_bytes);
            let estimated_toc_length = (footer.footer_offset - estimated_toc_start) as usize;
            debug!("trying TOC at: {estimated_toc_start:#x} with length {estimated_toc_length}");

            // Read in the newest block
            f.seek(SeekFrom::Start(estimated_toc_start))?;

            if estimated_num_blocks == max_blocks {
                // If the TOC is 1 block long, then it might be a short read
                buf.resize(estimated_toc_length, 0);
                f.read_exact(&mut buf)?;
            } else {
                // Reading in another full block to prepend
                let mut prev_buf = vec![0; footer.block_size_bytes as usize];
                f.read_exact(&mut prev_buf)?;

                // Append existing buffer to new buffer
                prev_buf.append(&mut buf);
                buf = prev_buf;
            }

            // Check MD5
            hasher.update(&buf);
            let hash = hasher.finalize_reset();
            if hash.starts_with(&footer.toc_hash) {
                // we have our match!
                debug!("TOC is {estimated_num_blocks} long");
                num_blocks = estimated_num_blocks;
                break;
            }
        }

        if num_blocks == 0 {
            error!("Cannot find archive index TOC with matching MD5");
            return Err(Error::FailedPrecondition);
        }

        let mut o = Self {
            last_ekey: Vec::with_capacity(num_blocks),
            block_partial_md5: Vec::with_capacity(num_blocks),
            num_blocks,
        };

        let mut toc = Cursor::new(&buf);
        for _ in 0..num_blocks {
            let mut e = vec![0; footer.key_bytes.into()];
            toc.read_exact(&mut e)?;
            o.last_ekey.push(e);
        }

        for _ in 0..num_blocks {
            let mut e = vec![0; footer.hash_bytes.into()];
            toc.read_exact(&mut e)?;
            o.block_partial_md5.push(e);
        }

        Ok(o)
    }
}

/// Entry in an [archive index][1] block.
///
/// [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
#[derive(Default, PartialEq, Eq)]
pub struct ArchiveIndexEntry {
    /// The EKey of the item in the archive.
    pub ekey: Vec<u8>,

    /// The BLTE-encoded size of the item in the archive.
    pub blte_encoded_size: u64,

    /// The offset of the item within the archive.
    pub archive_offset: u64,
}

/// Iterator-based archive index block parser.
///
/// This is an internal implementation detail of
/// [`ArchiveIndexParser::read_block`].
struct ArchiveIndexBlockParser<'a> {
    /// The archive index block.
    block: Vec<u8>,

    /// Current position within `block`.
    p: usize,

    /// The total length of each entry in the block.
    entry_length: usize,

    /// The archive index footer.
    footer: &'a ArchiveIndexFooter,

    /// The last expected EKey within the block.
    last_ekey: &'a [u8],
}

impl<'a> ArchiveIndexBlockParser<'a> {
    fn new(block: Vec<u8>, footer: &'a ArchiveIndexFooter, last_ekey: &'a [u8]) -> Self {
        let entry_length = usize::from(footer.key_bytes)
            + usize::from(footer.size_bytes)
            + usize::from(footer.offset_bytes);

        Self {
            block,
            p: 0,
            footer,
            entry_length,
            last_ekey,
        }
    }
}

impl<'a> Iterator for ArchiveIndexBlockParser<'a> {
    type Item = ArchiveIndexEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.block.len() < self.entry_length + self.p {
            return None;
        }

        let mut buf = &self.block[self.p..self.p + self.entry_length];
        self.p += self.entry_length;

        let ekey;
        (ekey, buf) = buf.split_at(self.footer.key_bytes.into());
        if ekey.iter().all(|b| *b == 0) {
            // All-zeroes.
            self.p = self.block.len();
            return None;
        }

        // These are variable-length integers, that aren't always powers of 2.
        // Lets pretend they're all big-endian u64s.
        let blte_encoded_size = if self.footer.size_bytes == 0 {
            0
        } else {
            let src;
            (src, buf) = buf.split_at(self.footer.size_bytes.into());

            let mut v = [0; size_of::<u64>()];
            let off = v.len() - usize::from(self.footer.size_bytes);
            v[off..].copy_from_slice(src);
            u64::from_be_bytes(v)
        };

        let archive_offset = if self.footer.offset_bytes == 0 {
            0
        } else {
            let src;
            (src, buf) = buf.split_at(self.footer.offset_bytes.into());

            let mut v = [0; size_of::<u64>()];
            let off = v.len() - usize::from(self.footer.size_bytes);
            v[off..].copy_from_slice(src);
            u64::from_be_bytes(v)
        };

        // Our previous reads should all add up
        assert!(buf.is_empty());

        if ekey == self.last_ekey {
            // This is the last EKey in the block, brick the iterator
            self.p = self.block.len();
        }

        Some(ArchiveIndexEntry {
            ekey: ekey.to_vec(),
            blte_encoded_size,
            archive_offset,
        })
    }
}

/// [Archive index][1] parser.
///
/// [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
#[derive(PartialEq, Eq)]
pub struct ArchiveIndexParser<T: BufRead + Seek> {
    /// File handle
    f: T,

    /// The archive index footer.
    footer: ArchiveIndexFooter,

    /// The archive index TOC.
    toc: ArchiveIndexToc,
}

impl<T: BufRead + Seek> ArchiveIndexParser<T> {
    /// Parse an archive index.
    pub fn new(mut f: T, hash: &Md5) -> Result<Self> {
        // Try to read the footer and TOC first
        let footer = ArchiveIndexFooter::parse(&mut f, hash)?;
        let toc = ArchiveIndexToc::parse(&mut f, &footer)?;
        Ok(Self { f, footer, toc })
    }

    /// The archive index footer.
    pub fn footer(&self) -> &ArchiveIndexFooter {
        &self.footer
    }

    /// The archive index TOC.
    pub fn toc(&self) -> &ArchiveIndexToc {
        &self.toc
    }

    /// Read archive index entries from a block.
    pub fn read_block(&mut self, index: usize) -> Result<impl Iterator<Item = ArchiveIndexEntry>> {
        let last_ekey = self
            .toc
            .last_ekey
            .get(index)
            .ok_or(Error::BlockIndexOutOfRange(index, self.toc.num_blocks))?;

        let expected_hash = self
            .toc
            .block_partial_md5
            .get(index)
            .ok_or(Error::BlockIndexOutOfRange(index, self.toc.num_blocks))?;

        let index = index as u64;

        self.f.seek(SeekFrom::Start(
            index * u64::from(self.footer.block_size_bytes),
        ))?;

        // Load the entire block into memory (it's small)
        let mut buf = vec![0; self.footer.block_size_bytes as usize];
        self.f.read_exact(&mut buf)?;

        // Verify block checksum
        let mut hasher = Md5Hasher::new();
        hasher.update(&buf);
        let actual_hash = hasher.finalize();

        if !actual_hash.starts_with(expected_hash) {
            error!(
                "block {index} hash mismatch: {} != {}",
                hex::encode(&actual_hash[..]),
                hex::encode(expected_hash),
            );
            return Err(Error::ChecksumMismatch);
        }

        Ok(ArchiveIndexBlockParser::new(buf, &self.footer, last_ekey))
    }

    /// Release the file handle from `ArchiveIndexParser`.
    pub fn to_inner(self) -> T {
        self.f
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn archive_index_test() {
        let _ = tracing_subscriber::fmt::try_init();
        let hash = b"\x00\x17\xa4\x02\xf5V\xfb\xec\xe4l8\xdcC\x1a,\x9b";
        let expected = ArchiveIndexFooter {
            toc_hash: vec![122, 251, 115, 207, 0, 207, 164, 22],
            format_revision: 1,
            flags0: 0,
            flags1: 0,
            block_size_bytes: 4096,
            offset_bytes: 4,
            size_bytes: 4,
            key_bytes: 16,
            hash_bytes: 8,
            num_elements: 7060,
            footer_offset: (4096 * 3) + 1024,
        };

        // Stripped down footer from 0017a402f556fbece46c38dc431a2c9b.index.
        //
        // This puts some dummy data at the start of the index to simulate other
        // entries, and a dummy TOC.
        let mut b = vec![0; (4096 * 3) + 1024];
        b.append(
            &mut hex::decode("7afb73cf00cfa4160100000404041008941b0000c2e814eb60ab8cf8").unwrap(),
        );

        let actual = ArchiveIndexFooter::parse(&mut Cursor::new(b), &hash).unwrap();
        assert_eq!(expected, actual);
    }
}
