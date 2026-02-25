//! Root file block structures and parsing logic

use crate::root::{
    entry::RootRecord,
    error::Result,
    flags::{ContentFlags, LocaleFlags},
    version::RootVersion,
};
use binrw::{BinRead, BinWrite};
use cascette_crypto::md5::{ContentKey, FileDataId};
use std::io::{Read, Seek, Write};

/// Block header containing metadata for file entries
///
/// Stores parsed header data for all root versions. The `content_flags` field
/// is `u64` to hold V4's 40-bit content flags without truncation.
/// V1 on-disk format is 12 bytes: `num_records(4) + content_flags(4) + locale_flags(4)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBlockHeader {
    /// Number of file records in this block
    pub num_records: u32,
    /// Content flags for all files in this block (up to 40 bits for V4)
    pub content_flags: u64,
    /// Locale flags for all files in this block
    pub locale_flags: LocaleFlags,
}

/// Block header for Version 2 format (17 bytes).
///
/// Introduced in build 11.1.0.58221. V2 moves locale_flags before content flags
/// and splits content flags into 3 fields.
/// Reconstructed content_flags = content_flags_1 | content_flags_2 | ((content_flags_3 as u32) << 17)
#[derive(BinRead, BinWrite, Debug, Clone, PartialEq, Eq)]
#[brw(little)]
pub struct RootBlockHeaderV2 {
    /// Number of file records in this block
    pub num_records: u32,
    /// Locale flags (locale IS first in V2, unlike V1)
    pub locale_flags: LocaleFlags,
    /// Content flags split field 1 (unk1)
    pub content_flags_1: u32,
    /// Content flags split field 2 (unk2)
    pub content_flags_2: u32,
    /// Content flags split field 3 (unk3)
    pub content_flags_3: u8,
}

impl BinRead for RootBlockHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let num_records = u32::read_le(reader)?;
        let content_flags = u64::from(u32::read_le(reader)?);
        let locale_flags = LocaleFlags::read_le(reader)?;
        Ok(Self {
            num_records,
            content_flags,
            locale_flags,
        })
    }
}

impl BinWrite for RootBlockHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        #[allow(clippy::cast_possible_truncation)]
        let content_flags_u32 = (self.content_flags & 0xFFFF_FFFF) as u32;
        self.num_records.write_le(writer)?;
        content_flags_u32.write_le(writer)?;
        self.locale_flags.write_le(writer)?;
        Ok(())
    }
}

/// Complete root block with header and file records
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBlock {
    /// Block header
    pub header: RootBlockHeader,
    /// File records in this block
    pub records: Vec<RootRecord>,
}

impl RootBlock {
    /// Create new empty block
    pub fn new(content_flags: ContentFlags, locale_flags: LocaleFlags) -> Self {
        Self {
            header: RootBlockHeader {
                num_records: 0,
                content_flags: content_flags.value,
                locale_flags,
            },
            records: Vec::new(),
        }
    }

    /// Add record to block
    pub fn add_record(&mut self, record: RootRecord) {
        self.records.push(record);
        // Since we're building the file, we can assume the number of records fits in u32
        // CASC root files are not expected to have more than 4 billion records
        #[allow(clippy::cast_possible_truncation)]
        {
            self.header.num_records = self.records.len() as u32;
        }
    }

    /// Get content flags as `ContentFlags`
    pub fn content_flags(&self) -> ContentFlags {
        ContentFlags::new(self.header.content_flags)
    }

    /// Get locale flags
    pub const fn locale_flags(&self) -> LocaleFlags {
        self.header.locale_flags
    }

    /// Get number of records
    pub const fn num_records(&self) -> u32 {
        self.header.num_records
    }

    /// Check if block has name hashes
    pub fn has_name_hashes(&self, version: RootVersion, has_named_files: bool) -> bool {
        match version {
            RootVersion::V1 => true, // V1 always has name hashes
            RootVersion::V2 | RootVersion::V3 | RootVersion::V4 => {
                has_named_files && self.content_flags().has_name_hashes()
            }
        }
    }

    /// Parse block from reader based on version
    pub fn parse<R: Read + Seek>(
        reader: &mut R,
        version: RootVersion,
        _has_named_files: bool,
    ) -> Result<Self> {
        match version {
            RootVersion::V1 => parse_v1_header(reader),
            RootVersion::V2 | RootVersion::V3 => parse_v2v3_header(reader),
            RootVersion::V4 => parse_v4_header(reader),
        }
    }

    /// Write block to writer based on version
    pub fn write<W: Write + Seek>(
        &self,
        writer: &mut W,
        version: RootVersion,
        _has_named_files: bool,
    ) -> Result<()> {
        match version {
            // V1 (no MFST header): 12-byte header + interleaved format
            RootVersion::V1 => {
                self.header.write_le(writer)?;
                if !self.records.is_empty() {
                    write_v1_block(writer, &self.records)?;
                }
            }
            // V2/V3 (MFST header): 17-byte header + separated format
            // Write content_flags into content_flags_1; content_flags_2 and _3 are zero.
            RootVersion::V2 | RootVersion::V3 => {
                #[allow(clippy::cast_possible_truncation)]
                let header_v2 = RootBlockHeaderV2 {
                    num_records: self.header.num_records,
                    locale_flags: self.header.locale_flags,
                    content_flags_1: (self.header.content_flags & 0xFFFF_FFFF) as u32,
                    content_flags_2: 0,
                    content_flags_3: 0,
                };
                header_v2.write_le(writer)?;
                if !self.records.is_empty() {
                    write_v2_v3_block(writer, &self.records, self.content_flags())?;
                }
            }
            // V4: 18-byte header with 40-bit content flags + separated format
            RootVersion::V4 => {
                self.header.num_records.write_le(writer)?;
                self.header.locale_flags.write_le(writer)?;
                ContentFlags::new(self.header.content_flags).write_v4(writer)?;
                0u32.write_le(writer)?; // unk2
                0u8.write_le(writer)?; // unk3
                if !self.records.is_empty() {
                    write_v2_v3_block(writer, &self.records, self.content_flags())?;
                }
            }
        }
        Ok(())
    }

    /// Sort records by `FileDataID` for optimal delta encoding
    pub fn sort_records(&mut self) {
        self.records.sort_by_key(|r| r.file_data_id);
    }

    /// Calculate block size in bytes for given version
    pub fn calculate_size(&self, version: RootVersion, has_named_files: bool) -> usize {
        // V1: 12-byte header (num_records + content_flags + locale_flags)
        // V2/V3: 17-byte header (num_records + locale_flags + content_flags(4) + unk2 + unk3)
        // V4: 18-byte header (num_records + locale_flags + content_flags(5) + unk2 + unk3)
        let header_size = match version {
            RootVersion::V1 => 12,
            RootVersion::V2 | RootVersion::V3 => 17,
            RootVersion::V4 => 18,
        };

        let count = self.records.len();

        if count == 0 {
            return header_size;
        }

        let fdid_size = count * 4; // i32 deltas
        let ckey_size = count * 16; // MD5 hashes

        let name_hash_size = if self.has_name_hashes(version, has_named_files) {
            count * 8 // u64 hashes
        } else {
            0
        };

        header_size + fdid_size + ckey_size + name_hash_size
    }
}

/// Parse a V1 block header and dispatch to record parsing.
fn parse_v1_header<R: Read + Seek>(reader: &mut R) -> Result<RootBlock> {
    let header = RootBlockHeader::read_le(reader)?;
    if header.num_records == 0 || header.num_records > 1_000_000 {
        return Ok(RootBlock {
            header,
            records: Vec::new(),
        });
    }
    let count = header.num_records as usize;
    parse_v1_block(reader, header, count)
}

/// Parse a V2/V3 block header (17 bytes) and dispatch to record parsing.
/// Reconstructs content_flags from 3 split fields per the wowdev wiki spec.
fn parse_v2v3_header<R: Read + Seek>(reader: &mut R) -> Result<RootBlock> {
    let header_v2 = RootBlockHeaderV2::read_le(reader)?;
    let reconstructed_flags = reconstruct_v2_content_flags(&header_v2);
    let header = RootBlockHeader {
        num_records: header_v2.num_records,
        content_flags: reconstructed_flags,
        locale_flags: header_v2.locale_flags,
    };
    if header_v2.num_records == 0 || header_v2.num_records > 1_000_000 {
        return Ok(RootBlock {
            header,
            records: Vec::new(),
        });
    }
    let count = header_v2.num_records as usize;
    let content_flags = ContentFlags::new(reconstructed_flags);
    parse_v2_block(reader, header, count, content_flags)
}

/// Reconstruct a single content_flags u64 from the 3 split fields in a V2 header.
/// Formula from wowdev wiki: flags_1 | flags_2 | (flags_3 << 17)
fn reconstruct_v2_content_flags(h: &RootBlockHeaderV2) -> u64 {
    u64::from(h.content_flags_1)
        | u64::from(h.content_flags_2)
        | (u64::from(h.content_flags_3) << 17)
}

/// Parse a V4 block header (18 bytes) and dispatch to record parsing.
fn parse_v4_header<R: Read + Seek>(reader: &mut R) -> Result<RootBlock> {
    let num_records = u32::read_le(reader)?;
    let locale_flags = LocaleFlags::read_le(reader)?;
    let content_flags = ContentFlags::read_v4(reader)?;
    let _unk2 = u32::read_le(reader)?;
    let _unk3 = u8::read_le(reader)?;
    let header = RootBlockHeader {
        num_records,
        content_flags: content_flags.value,
        locale_flags,
    };
    if num_records == 0 || num_records > 1_000_000 {
        return Ok(RootBlock {
            header,
            records: Vec::new(),
        });
    }
    let count = num_records as usize;
    parse_v2_block(reader, header, count, content_flags)
}

/// Parse V1 block (all deltas first, then interleaved ckey+hash per record)
///
/// Format (12-byte header, interleaved data):
/// 1. Block header: count (4) + content_flags (4) + locale_flags (4)
/// 2. ALL FileDataID deltas: count * 4 bytes
/// 3. For each record: ckey (16 bytes) + namehash (8 bytes) interleaved
fn parse_v1_block<R: Read + Seek>(
    reader: &mut R,
    header: RootBlockHeader,
    count: usize,
) -> Result<RootBlock> {
    // Read ALL deltas first
    let mut deltas = Vec::with_capacity(count);
    for _ in 0..count {
        deltas.push(u32::read_le(reader)?);
    }

    // Decode FileDataIDs from deltas using TACT.Net accumulation
    let fdids = decode_file_data_ids_tact(&deltas);

    // Now read interleaved (ckey, namehash) for each record
    let mut records = Vec::with_capacity(count);
    for fdid in fdids {
        let content_key = ContentKey::read_le(reader)?;
        let name_hash = Some(u64::read_le(reader)?); // V1 always has name hashes

        records.push(RootRecord::new(fdid, content_key, name_hash));
    }

    Ok(RootBlock { header, records })
}

/// Decode FileDataIDs from deltas using TACT.Net format
/// currentId += delta; fileId = currentId++;
fn decode_file_data_ids_tact(deltas: &[u32]) -> Vec<FileDataId> {
    let mut fdids = Vec::with_capacity(deltas.len());
    let mut current_id: u32 = 0;
    for &delta in deltas {
        current_id = current_id.wrapping_add(delta);
        let file_id = current_id;
        current_id = current_id.wrapping_add(1);
        fdids.push(FileDataId::new(file_id));
    }
    fdids
}

/// Parse V2/V3/V4 block (separated arrays format)
///
/// Format (17-byte header already read, separated data):
/// 1. Block header: count (4) + locale (4) + content_flags (4) + unk2 (4) + unk3 (1) = 17 bytes
/// 2. ALL FileDataID deltas: count * 4 bytes
/// 3. ALL Content keys: count * 16 bytes
/// 4. ALL Name hashes: count * 8 bytes - only if NO_NAME_HASH flag NOT set
fn parse_v2_block<R: Read + Seek>(
    reader: &mut R,
    header: RootBlockHeader,
    count: usize,
    content_flags: ContentFlags,
) -> Result<RootBlock> {
    // Read ALL deltas FIRST (this is the key difference!)
    let mut deltas = Vec::with_capacity(count);
    for _ in 0..count {
        deltas.push(u32::read_le(reader)?);
    }

    // Decode FileDataIDs from deltas using TACT.Net accumulation
    let fdids = decode_file_data_ids_tact(&deltas);

    // Read ALL content keys (separated array)
    let mut content_keys = Vec::with_capacity(count);
    for _ in 0..count {
        content_keys.push(ContentKey::read_le(reader)?);
    }

    // Read ALL name hashes together if NO_NAME_HASH flag is NOT set
    let has_name_hashes = content_flags.has_name_hashes();
    let mut name_hashes = Vec::with_capacity(count);
    if has_name_hashes {
        for _ in 0..count {
            name_hashes.push(Some(u64::read_le(reader)?));
        }
    } else {
        name_hashes.resize(count, None);
    }

    // Combine into records using iterators
    let records: Vec<RootRecord> = fdids
        .into_iter()
        .zip(content_keys)
        .zip(name_hashes)
        .map(|((fdid, ckey), hash)| RootRecord::new(fdid, ckey, hash))
        .collect();

    Ok(RootBlock { header, records })
}

/// Write V1 block (all deltas first, then interleaved ckey+hash per record)
///
/// Format per TACT.Net RootBlock:
/// 1. ALL deltas first
/// 2. For each record: ckey + namehash (interleaved)
fn write_v1_block<W: Write + Seek>(writer: &mut W, records: &[RootRecord]) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }

    // Encode FileDataIDs to deltas
    let fdids: Vec<FileDataId> = records.iter().map(|r| r.file_data_id).collect();
    let deltas = encode_file_data_ids_tact(&fdids);

    // Write ALL deltas first
    for delta in &deltas {
        delta.write_le(writer)?;
    }

    // Write interleaved (ckey, hash) for each record
    for record in records {
        record.content_key.write_le(writer)?;
        let name_hash = record.name_hash.unwrap_or(0);
        name_hash.write_le(writer)?;
    }

    Ok(())
}

/// Encode FileDataIDs to deltas using TACT.Net format
/// For first record: delta = fileId
/// For subsequent: delta = fileId - prevFileId - 1
fn encode_file_data_ids_tact(fdids: &[FileDataId]) -> Vec<u32> {
    if fdids.is_empty() {
        return Vec::new();
    }

    let mut deltas = Vec::with_capacity(fdids.len());

    // First delta is the FileId itself
    deltas.push(fdids[0].get());

    // Subsequent deltas: current - prev - 1
    for i in 1..fdids.len() {
        let current = fdids[i].get();
        let prev = fdids[i - 1].get();
        // TACT.Net: currentId += delta; fileId = currentId++;
        // Reverse: delta = current - prev - 1 (since prev+1 was stored after prev)
        let delta = current.wrapping_sub(prev).wrapping_sub(1);
        deltas.push(delta);
    }

    deltas
}

/// Write V2/V3 block (separated arrays format per TACT.Net RootBlockV2)
///
/// Format per TACT.Net RootBlockV2:
/// 1. ALL FileDataID deltas: count * 4 bytes (FIRST!)
/// 2. ALL Content keys: count * 16 bytes
/// 3. ALL Name hashes: count * 8 bytes - only if NO_NAME_HASH flag NOT set
fn write_v2_v3_block<W: Write + Seek>(
    writer: &mut W,
    records: &[RootRecord],
    content_flags: ContentFlags,
) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }

    // Encode FileDataIDs to deltas
    let fdids: Vec<FileDataId> = records.iter().map(|r| r.file_data_id).collect();
    let deltas = encode_file_data_ids_tact(&fdids);

    // Write ALL deltas FIRST
    for delta in &deltas {
        delta.write_le(writer)?;
    }

    // Write ALL content keys
    for record in records {
        record.content_key.write_le(writer)?;
    }

    // Write ALL name hashes if NO_NAME_HASH flag is NOT set
    let has_name_hashes = content_flags.has_name_hashes();
    if has_name_hashes {
        for record in records {
            let name_hash = record.name_hash.unwrap_or(0);
            name_hash.write_le(writer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "block_tests.rs"]
mod tests;
