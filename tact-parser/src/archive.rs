//! Parsing archives and [archive indexes][1].
//!
//! [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)

use md5::{Digest, Md5 as Md5Hasher};
use tracing::error;

use crate::{Error, Md5, Result};
use std::{
    io::{Read, Seek, SeekFrom},
    iter::repeat_n,
};

/// [Archive index][1] footer parser.
///
/// [1]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
#[derive(Debug, PartialEq, Eq)]
pub struct ArchiveIndexFooter {
    pub toc_hash: Vec<u8>,
    pub format_revision: u8,
    pub flags0: u8,
    pub flags1: u8,
    pub block_size_bytes: usize,
    pub offset_bytes: u8,
    pub size_bytes: u8,
    pub key_bytes: u8,
    pub hash_bytes: u8,
    pub num_elements: u32,
}

impl ArchiveIndexFooter {
    const MIN_HASH_BYTES: u8 = 0x8;
    const MAX_HASH_BYTES: u8 = 0x10;
    const MAX_FOOTER_SIZE: usize = Self::size(Self::MAX_HASH_BYTES) as usize;
    const MAX_FOOTER_SIZE_I64: i64 = Self::size(Self::MAX_HASH_BYTES) as i64;

    /// Size of the footer given `hash_bytes`.
    pub const fn size(hash_bytes: u8) -> u16 {
        12 + ((hash_bytes as u16) * 2)
    }

    /// Parses an archive index footer.
    pub fn parse<R: Read + Seek>(f: &mut R, hash: &Md5) -> Result<Self> {
        let mut footer_buf = [0; Self::MAX_FOOTER_SIZE];
        f.seek(SeekFrom::End(-Self::MAX_FOOTER_SIZE_I64))?;
        f.read_exact(&mut footer_buf)?;
        let (hash_bytes, footer) = Self::find_footer(&footer_buf, hash)?;

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

        if &actual_footer_hash[..expected_footer_hash.len()] != expected_footer_hash {
            error!(
                "footer_hash mismatch: {} != {}",
                hex::encode(&actual_footer_hash[..]),
                hex::encode(expected_footer_hash),
            );
            return Err(Error::FailedPrecondition);
        }

        Ok(Self {
            toc_hash,
            format_revision,
            flags0: footer[hash_bytes_usize + 1],
            flags1: footer[hash_bytes_usize + 2],
            block_size_bytes: usize::from(footer[hash_bytes_usize + 3]) << 10,
            offset_bytes: footer[hash_bytes_usize + 4],
            size_bytes: footer[hash_bytes_usize + 5],
            key_bytes: footer[hash_bytes_usize + 6],
            hash_bytes,
            num_elements: u32::from_le_bytes(
                footer[hash_bytes_usize + 8..hash_bytes_usize + 12]
                    .try_into()
                    .unwrap(),
            ),
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
}
