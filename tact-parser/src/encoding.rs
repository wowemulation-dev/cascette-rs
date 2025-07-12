use crate::{Error, Result, ioutils::ReadInt};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{Read, Seek},
};
use tracing::*;

const ENCODING_MAGIC: &'static [u8; 2] = b"EN";

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

#[derive(Debug)]
pub struct CEKeyEntry<const CKEY_SIZE: usize, const EKEY_SIZE: usize> {
    /// Size of the non-encoded version of the file.
    pub file_size: u64,
    pub ckey: [u8; CKEY_SIZE],
    pub ekeys: Vec<[u8; EKEY_SIZE]>,
}

impl<const CKEY_SIZE: usize, const EKEY_SIZE: usize> CEKeyEntry<CKEY_SIZE, EKEY_SIZE> {
    pub fn parse<R: Read + Seek>(f: &mut R, remain: &mut u32) -> Result<Option<Self>> {
        if *remain < 0x6 + CKEY_SIZE as u32 + EKEY_SIZE as u32 {
            // There's no way we could read this
            *remain = 0;
            return Ok(None);
        }

        let count = f.read_u8()?;
        if count == 0 {
            *remain = 0;
            return Ok(None);
        }

        if let Some(nr) =
            remain.checked_sub(0x6 + CKEY_SIZE as u32 + (count as u32 * EKEY_SIZE as u32))
        {
            *remain = nr;
        } else {
            // This would overrun the block
            *remain = 0;
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

pub struct EncodingTable {
    /// Mapping of `ckey` (MD5) -> (file_size, ekeys)
    pub md5_map: HashMap<[u8; 16], (u64, Vec<[u8; 16]>)>,
}

impl EncodingTable {
    pub fn parse<R: Read + Seek>(f: &mut R) -> Result<Self> {
        let header = EncodingTableHeader::parse(f)?;

        // These are "always" 0x10 bytes, and we actually need it to be this so
        // we know it at compile time.
        if header.hash_size_ckey != 0x10 {
            error!("hash_size_ckey ({:#x}) != 0x10", header.hash_size_ckey);
            return Err(Error::FailedPrecondition);
        }

        if header.hash_size_ekey != 0x10 {
            error!("hash_size_ekey ({:#x}) != 0x10", header.hash_size_ekey);
            return Err(Error::FailedPrecondition);
        }

        // Skip the espec BLTE strings
        f.seek_relative(header.e_spec_block_size.into())?;

        // CEKeyPageTable index
        let mut ce_key_page_table_index: Vec<([u8; 0x10], [u8; 0x10])> =
            Vec::with_capacity(header.ce_key_page_table_page_count as usize);
        for _ in 0..header.ce_key_page_table_page_count {
            let mut first_ce_key = [0; 0x10];
            let mut page_md5 = [0; 0x10];

            f.read_exact(&mut first_ce_key)?;
            f.read_exact(&mut page_md5)?;
            ce_key_page_table_index.push((first_ce_key, page_md5));
        }

        // Reading pages
        let mut md5_map = HashMap::<[u8; 0x10], (u64, Vec<[u8; 0x10]>)>::new();
        for page_id in 0..header.ce_key_page_table_page_count {
            // Find where we can read to
            // let end_pos =
            // f.seek(SeekFrom::Current(0))? + u64::from(header.ce_key_page_table_page_size) - 0x26;

            // TODO: check MD5
            let mut remain = header.ce_key_page_table_page_size;
            while let Some(entry) = CEKeyEntry::<0x10, 0x10>::parse(f, &mut remain)? {
                if let Some(old) = md5_map.insert(entry.ckey, (entry.file_size, entry.ekeys)) {
                    warn!("key conflict: {old:?}");
                }
            }
        }

        Ok(Self { md5_map })
    }
}
