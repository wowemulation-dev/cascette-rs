use crate::{Error, MD5_LENGTH, Md5, Result, ioutils::ReadInt};
use md5::{Digest, Md5 as Md5Hasher};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{Cursor, Read, Seek},
};
use tracing::*;

const ENCODING_MAGIC: &[u8; 2] = b"EN";

#[derive(Debug)]
pub struct EncodingTableHeader {
    pub version: u8,
    pub hash_size_ckey: u8,
    pub hash_size_ekey: u8,

    pub ce_key_page_table_page_size: u32,
    pub e_key_spec_page_table_page_size: u32,

    pub ce_key_page_table_page_count: u32,
    pub e_key_spec_page_table_page_count: u32,

    pub flags: u8,
    pub e_spec_block_size: u32,
}

impl EncodingTableHeader {
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mut magic = [0; 2];
        f.read_exact(&mut magic)?;
        if &magic != ENCODING_MAGIC {
            error!("Incorrect magic");
            return Err(Error::BadMagic);
        }

        let version = f.read_u8()?;
        if version != 1 {
            error!("Unsupported encoding table version: {version}");
            return Err(Error::NotImplemented);
        }

        Ok(Self {
            version,
            hash_size_ckey: f.read_u8()?,
            hash_size_ekey: f.read_u8()?,
            ce_key_page_table_page_size: u32::from(f.read_u16be()?) * 1024,
            e_key_spec_page_table_page_size: u32::from(f.read_u16be()?) * 1024,
            ce_key_page_table_page_count: f.read_u32be()?,
            e_key_spec_page_table_page_count: f.read_u32be()?,
            flags: f.read_u8()?,
            e_spec_block_size: f.read_u32be()?,
        })
    }
}

/// Entry in the [`CEKeyPageTable`][0].
///
/// [0]: https://wowdev.wiki/TACT#CEKeyPageTable
#[derive(Debug)]
struct CEKeyEntry<const CKEY_SIZE: usize, const EKEY_SIZE: usize> {
    /// Size of the non-encoded version of the file.
    pub file_size: u64,

    /// CKey (MD5) of the decoded file.
    pub ckey: [u8; CKEY_SIZE],

    /// EKey(s) (MD5) of the encoded file.
    pub ekeys: Vec<[u8; EKEY_SIZE]>,
}

impl<const CKEY_SIZE: usize, const EKEY_SIZE: usize> CEKeyEntry<CKEY_SIZE, EKEY_SIZE> {
    const BASE_ENTRY_SIZE: usize = 6 + CKEY_SIZE;

    /// Parse a single `CEKeyPageTable` entry.
    pub fn parse<R: Read>(f: &mut R, remain: &mut u32) -> Result<Option<Self>> {
        if *remain < Self::BASE_ENTRY_SIZE as u32 + EKEY_SIZE as u32 {
            // There's no way we could read this
            return Ok(None);
        }

        let count = f.read_u8()?;
        if count == 0 {
            // End of the block
            *remain -= 1;
            return Ok(None);
        }

        // count + file_size + ckey + (count * ekey)
        if let Some(nr) =
            remain.checked_sub(Self::BASE_ENTRY_SIZE as u32 + (count as u32 * EKEY_SIZE as u32))
        {
            *remain = nr;
        } else {
            // This would overrun the block
            return Ok(None);
        }

        let file_size = f.read_u40be()?;
        let mut ckey = [0; CKEY_SIZE];
        f.read_exact(&mut ckey)?;

        let mut ekeys = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            let mut ekey = [0; EKEY_SIZE];
            f.read_exact(&mut ekey)?;
            ekeys.push(ekey);
        }

        Ok(Some(Self {
            file_size,
            ckey,
            ekeys,
        }))
    }
}

/// [Encoding table][0].
///
/// The encoding file contains:
///
/// * Encoding spec table (not parsed)
/// * `ckey` (MD5) -> `(file_size, ekeys)`
/// * `ekey` (MD5) -> encoding spec (not parsed)
/// * Encoding spec for the encoding file itself (not parsed)
///
/// [0]: https://wowdev.wiki/TACT#Encoding_table
pub struct EncodingTable {
    /// Mapping of `ckey` (MD5) -> `(file_size, ekeys)`.
    pub md5_map: HashMap<Md5, (u64, Vec<Md5>)>,
}

impl EncodingTable {
    /// Parse an encoding table.
    pub fn parse<R: Read + Seek>(f: &mut R) -> Result<Self> {
        let header = EncodingTableHeader::parse(f)?;

        // These appear to be "always" 0x10 bytes, and we rely on this so that
        // structures have a known size at compile time.
        if header.hash_size_ckey != MD5_LENGTH as u8 {
            error!("hash_size_ckey {} != {MD5_LENGTH}", header.hash_size_ckey);
            return Err(Error::FailedPrecondition);
        }

        if header.hash_size_ekey != MD5_LENGTH as u8 {
            error!("hash_size_ekey {} != {MD5_LENGTH}", header.hash_size_ekey);
            return Err(Error::FailedPrecondition);
        }

        // Skip the espec BLTE strings
        f.seek_relative(header.e_spec_block_size.into())?;

        // CEKeyPageTable index
        // https://wowdev.wiki/TACT#Page_Tables
        //
        // We only use the MD5 of the pages here.
        let mut ce_key_page_table_md5: Vec<Md5> =
            Vec::with_capacity(header.ce_key_page_table_page_count as usize);
        for _ in 0..header.ce_key_page_table_page_count {
            // Skip first_ce_key (we don't use this)
            f.seek_relative(MD5_LENGTH as i64)?;

            let mut page_md5 = [0; MD5_LENGTH];
            f.read_exact(&mut page_md5)?;
            ce_key_page_table_md5.push(page_md5);
        }

        // CEKeyPageTable entries
        // https://wowdev.wiki/TACT#CEKeyPageTable
        //
        // We pre-allocate the HashMap assuming the maximum number of entries.
        // On Retail encoding tables, HashMap ends up with the same capacity
        // regardless of whether we pre-allocate or not, or if we only
        // pre-allocate at each page.
        //
        // However, pre-allocating the maximum number of entries _once_ is the
        // fastest.
        const MIN_ENTRY_SIZE: usize = 6 + (MD5_LENGTH * 2);
        let max_entry_count = ((header.ce_key_page_table_page_size as usize) / MIN_ENTRY_SIZE)
            * header.ce_key_page_table_page_count as usize;
        let mut md5_map = HashMap::with_capacity(max_entry_count);

        let mut hasher = Md5Hasher::new();
        for (i, page_md5) in ce_key_page_table_md5.iter().enumerate() {
            // Read in the entire CEKeyPageTable, so that we don't overrun
            // buffers.
            let mut page = vec![0; header.ce_key_page_table_page_size as usize];
            f.read_exact(&mut page)?;

            // Check the MD5 of the page
            hasher.update(&page);
            let result = hasher.finalize_reset();
            if !result.starts_with(page_md5) {
                error!(
                    "Page {i} MD5 {} != {}",
                    hex::encode(result),
                    hex::encode(page_md5)
                );
                return Err(Error::ChecksumMismatch);
            }

            // Parse the entries in the page
            let mut page = Cursor::new(&page);
            let mut remain = header.ce_key_page_table_page_size;
            while let Some(entry) = CEKeyEntry::parse(&mut page, &mut remain)? {
                if let Some(old) = md5_map.insert(entry.ckey, (entry.file_size, entry.ekeys)) {
                    warn!(
                        "Encoding key conflict for {}: {old:?}",
                        hex::encode(entry.ckey),
                    );
                }
            }
        }

        // EKeySpecPageTable and ESpec for encoding table not parsed.
        Ok(Self { md5_map })
    }
}
