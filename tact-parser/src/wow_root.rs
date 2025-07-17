//! Implementation of the [WoW TACT Root][0] file format (`TSFM` / `MFST`).
//!
//! This is sometimes called the CASC Root - but CASC has its own file formats
//! for as they appear on disk.
//!
//! [0]: https://wowdev.wiki/TACT#Root

use crate::{Error, Result, ioutils::ReadInt, utils::jenkins3_hashpath};
use modular_bitfield::{bitfield, prelude::*};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    io::{ErrorKind, Read, Seek},
    ops::BitAnd,
};

const TACT_MAGIC: &[u8; 4] = b"TSFM";
const MD5_LENGTH: usize = 16;
pub type Md5 = [u8; MD5_LENGTH];

#[derive(Debug)]
pub struct WowRootHeader {
    pub use_old_record_format: bool,
    pub version: u32,
    pub total_file_count: u32,
    pub named_file_count: u32,
    pub allow_non_named_files: bool,
}

impl WowRootHeader {
    /// Parses a WoW Root header.
    pub fn parse<R: Read + Seek>(f: &mut R) -> Result<Self> {
        let mut magic = [0; TACT_MAGIC.len()];
        f.read_exact(&mut magic)?;
        if &magic != TACT_MAGIC {
            // Pre-8.2 WoW root file (used by Classic Era)
            f.seek_relative(-(TACT_MAGIC.len() as i64))?;
            return Ok(Self {
                use_old_record_format: true,
                version: 0,
                total_file_count: 0,
                named_file_count: 0,
                allow_non_named_files: true,
            });
        }

        // See if there's a header size here
        let mut header_size = f.read_u32le()?;
        let mut version = 0;
        let total_file_count;

        if header_size == 0x18 {
            // Format >= 10.1.7.50893
            version = f.read_u32le()?;
            total_file_count = f.read_u32le()?;
        } else {
            total_file_count = header_size;
            header_size = 0;
        }
        let named_file_count = f.read_u32le()?;

        if header_size == 0x18 {
            // skip padding
            f.seek_relative(4)?;
        }

        Ok(Self {
            use_old_record_format: false,
            allow_non_named_files: total_file_count != named_file_count,
            version,
            total_file_count,
            named_file_count,
        })
    }
}

pub struct CasBlock {
    pub flags: LocaleContentFlags,
    pub fid_md5: Option<Vec<(u32, Md5)>>,
    pub name_hash_fid: Option<Vec<(u64, u32)>>,
}

impl std::fmt::Debug for CasBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CasBlock")
            .field("context", &self.flags)
            .field("fid_md5.len", &self.fid_md5.as_ref().map(|v| v.len()))
            .field(
                "name_hash_fid.len",
                &self.name_hash_fid.as_ref().map(|v| v.len()),
            )
            .finish()
    }
}

impl CasBlock {
    pub fn parse<R: Read + Seek>(
        f: &mut R,
        header: &WowRootHeader,
        only_locale: LocaleFlags,
    ) -> Result<Self> {
        let num_records = f.read_u32le()? as usize;

        let flags = if header.version == 2 {
            let locale = LocaleFlags::from(f.read_u32le()?);
            let v1 = f.read_u32le()?;
            let v2 = f.read_u32le()?;
            let v3 = f.read_u8()?;

            LocaleContentFlags {
                locale,
                content: ContentFlags::from(v1 | v2 | (u32::from(v3) << 17)),
            }
        } else {
            LocaleContentFlags {
                content: ContentFlags::from(f.read_u32le()?),
                locale: LocaleFlags::from(f.read_u32le()?),
            }
        };

        if num_records == 0 {
            // Ignore empty blocks without seeking
            return Ok(Self {
                flags,
                fid_md5: None,
                name_hash_fid: None,
            });
        }

        let has_name_hashes = header.use_old_record_format
            || !(header.allow_non_named_files && flags.content.no_name_hash());
        if !flags.locale.all() && !(flags.locale & only_locale).any() {
            // Skip the section, not for us.
            // The size of the section is the same in both old and new record
            // format, just arranged differently.
            let record_length =
                size_of::<u32>() + MD5_LENGTH + if has_name_hashes { size_of::<u64>() } else { 0 };
            f.seek_relative((num_records * record_length) as i64)?;

            return Ok(Self {
                flags,
                fid_md5: None,
                name_hash_fid: None,
            });
        }

        // Convert file_id_deltas -> absolute file_id
        let mut file_ids: Vec<u32> = Vec::with_capacity(num_records);
        let mut file_id = 0u32;
        for i in 0..num_records {
            let delta = f.read_i32le()?;

            file_id = if i == 0 {
                u32::try_from(delta).map_err(|_| Error::FileIdDeltaOverflow)?
            } else {
                (file_id)
                    .checked_add_signed(1 + delta)
                    .ok_or(Error::FileIdDeltaOverflow)?
            };

            file_ids.push(file_id);
        }

        // Collect content MD5s
        let mut fid_md5: Vec<(u32, Md5)> = Vec::with_capacity(num_records);
        let mut name_hash_fid: Option<Vec<(u64, u32)>> = None;

        if header.use_old_record_format {
            let mut o = Vec::with_capacity(num_records);

            for file_id in file_ids {
                let mut md5 = [0; MD5_LENGTH];
                f.read_exact(&mut md5)?;
                fid_md5.push((file_id, md5));
                o.push((f.read_u64le()?, file_id));
            }

            name_hash_fid = Some(o);
        } else {
            for &file_id in file_ids.iter() {
                let mut md5 = [0; MD5_LENGTH];
                f.read_exact(&mut md5)?;
                fid_md5.push((file_id, md5));
            }

            if has_name_hashes {
                let mut o = Vec::with_capacity(num_records);

                for &file_id in file_ids.iter() {
                    let hash = f.read_u64le()?;
                    o.push((hash, file_id));
                }

                name_hash_fid = Some(o);
            }
        }

        Ok(Self {
            flags,
            fid_md5: Some(fid_md5),
            name_hash_fid,
        })
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct LocaleContentFlags {
    pub locale: LocaleFlags,
    pub content: ContentFlags,
}

/// Bitmask of locales the content should be used for.
#[bitfield(bytes = 4)]
#[derive(PartialEq, Eq, Debug, Copy, Clone, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub struct LocaleFlags {
    #[skip]
    __: B1,
    pub en_us: bool, // 0x2
    #[skip]
    __: B1,
    pub ko_kr: bool, // 0x4

    pub fr_fr: bool, // 0x10
    pub de_de: bool, // 0x20
    pub zh_cn: bool, // 0x40
    pub es_es: bool, // 0x80

    pub zh_tw: bool, // 0x100
    pub en_gb: bool, // 0x200
    pub en_cn: bool, // 0x400
    pub en_tw: bool, // 0x800

    pub es_mx: bool, // 0x1000
    pub ru_ru: bool, // 0x2000
    pub pt_br: bool, // 0x4000
    pub it_it: bool, // 0x8000

    pub pt_pt: bool, // 0x10000
    #[skip]
    __: B15,
}

impl LocaleFlags {
    /// `LocaleFlags` which sets all locales to `true`.
    pub fn any_locale() -> Self {
        LocaleFlags::from(0xffffffff)
    }

    /// `true` if the flags indicate all locales.
    pub fn all(&self) -> bool {
        self == &Self::any_locale()
    }

    /// `true` if there is at least one locale flag set.
    pub fn any(&self) -> bool {
        u32::from(*self) != 0
    }
}

impl BitAnd for LocaleFlags {
    type Output = LocaleFlags;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from(u32::from(self) & u32::from(rhs))
    }
}

/// TACT content flags on the WoW root index.
///
/// Reference: [WoWDev Wiki](https://wowdev.wiki/TACT#Root)
#[bitfield(bytes = 4)]
#[derive(PartialEq, Eq, Debug, Copy, Clone, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub struct ContentFlags {
    /// Is high-res texture (Cataclysm 4.4.0 beta).
    pub high_res_texture: bool, // 0x1
    #[skip]
    __: B1,
    /// File is in install manifest.
    pub install: bool, // 0x4
    /// Non-Windows clients should ignore this file.
    pub windows: bool, // 0x8

    /// Non-macOS clients should ignore this file.
    pub macos: bool, // 0x10
    /// `x86_32` binary.
    pub x86_32: bool, // 0x20
    /// `x86_64` binary.
    pub x86_64: bool, // 0x40
    /// Low violence variant.
    pub low_violence: bool, // 0x80

    /// Non-mystery-platform clients should ignore this file.
    pub mystery_platform: bool, // 0x100
    #[skip]
    __: B2,
    /// Only set for `UpdatePlugin.{dll,dylib}`
    pub update_plugin: bool, // 0x800

    #[skip]
    __: B3,
    /// `aarch64` / ARM64 binary.
    pub aarch64: bool, // 0x8000

    #[skip]
    __: B11,
    pub encrypted: bool, // 0x8000000

    pub no_name_hash: bool, // 0x10000000
    /// Non-1280px wide cinematics.
    pub uncommon_resolution: bool, // 0x20000000
    pub bundle: bool,       // 0x40000000
    pub no_compression: bool, // 0x80000000
}

/// [WoW TACT Root][0] parser.
///
/// [0]: https://wowdev.wiki/TACT#Root
pub struct WowRoot {
    /// Mapping of File ID -> Flags + MD5
    pub fid_md5: BTreeMap<u32, BTreeMap<LocaleContentFlags, Md5>>,

    /// Mapping of `jenkins3_hashpath` -> file ID.
    pub name_hash_fid: HashMap<u64, u32>,
}

impl WowRoot {
    /// Parse a WoW TACT root file.
    pub fn parse<R: Read + Seek>(f: &mut R, only_locale: LocaleFlags) -> Result<Self> {
        let header = WowRootHeader::parse(f)?;
        let mut o = Self {
            fid_md5: BTreeMap::new(),
            name_hash_fid: HashMap::new(),
        };

        // Keep reading to EOF
        loop {
            match CasBlock::parse(f, &header, only_locale) {
                Ok(block) => {
                    // Add fids and name hashes to our collection
                    // TODO: make CasBlock push this directly, rather than
                    // allocating many temporary Vecs.
                    if let Some(fid_md5) = block.fid_md5 {
                        for (k, v) in fid_md5 {
                            if let Some(e) = o.fid_md5.get_mut(&k) {
                                assert!(e.insert(block.flags, v).is_none());
                            } else {
                                o.fid_md5.insert(k, BTreeMap::from([(block.flags, v)]));
                            }
                        }
                    }

                    if let Some(name_hash_fid) = block.name_hash_fid {
                        for (k, v) in name_hash_fid {
                            o.name_hash_fid.entry(k).or_insert(v);
                        }
                    }
                }

                Err(Error::IOError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    break;
                }

                Err(e) => return Err(e),
            }
        }
        Ok(o)
    }

    /// Gets a file ID for the given `path`.
    ///
    /// Returns `None` if the file cannot be found in the root index.
    pub fn get_fid(&self, path: &str) -> Option<u32> {
        let hash = jenkins3_hashpath(path);
        self.name_hash_fid.get(&hash).copied()
    }
}

impl Debug for WowRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WowRoot")
            .field("fid_md5.len", &self.fid_md5.len())
            .field("name_hash_fid.len", &self.name_hash_fid.len())
            .finish()
    }
}
