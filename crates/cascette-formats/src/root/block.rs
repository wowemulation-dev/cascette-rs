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

/// Block header for Version 2 format (17 bytes) - used when extended header version >= 2
/// Per wowdev.wiki: locale_flags moved before content flags, plus extra fields
#[derive(BinRead, BinWrite, Debug, Clone, PartialEq, Eq)]
#[brw(little)]
pub struct RootBlockHeaderV2 {
    /// Number of file records in this block
    pub num_records: u32,
    /// Locale flags (MOVED to second position in V2)
    pub locale_flags: LocaleFlags,
    /// Content flags (was second in V1, now third in V2)
    pub content_flags: u32,
    /// Unknown field 2
    pub unk2: u32,
    /// Unknown field 3 (upper bits define flags via bit-shift)
    pub unk3: u8,
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
            // V1 (no header): 12-byte block header, interleaved format
            RootVersion::V1 => {
                let header = RootBlockHeader::read_le(reader)?;

                // Sanity check - detect garbage data
                if header.num_records == 0 || header.num_records > 1_000_000 {
                    return Ok(Self {
                        header,
                        records: Vec::new(),
                    });
                }

                let count = header.num_records as usize;
                parse_v1_block(reader, header, count)
            }
            // V2/V3 (with MFST/TSFM header): 17-byte block header, separated format
            RootVersion::V2 | RootVersion::V3 => {
                let header_v2 = RootBlockHeaderV2::read_le(reader)?;

                // Sanity check - detect garbage data
                if header_v2.num_records == 0 || header_v2.num_records > 1_000_000 {
                    let header = RootBlockHeader {
                        num_records: header_v2.num_records,
                        content_flags: u64::from(header_v2.content_flags),
                        locale_flags: header_v2.locale_flags,
                    };
                    return Ok(Self {
                        header,
                        records: Vec::new(),
                    });
                }

                let count = header_v2.num_records as usize;
                let content_flags = ContentFlags::new(u64::from(header_v2.content_flags));

                let header = RootBlockHeader {
                    num_records: header_v2.num_records,
                    content_flags: u64::from(header_v2.content_flags),
                    locale_flags: header_v2.locale_flags,
                };

                parse_v2_block(reader, header, count, content_flags)
            }
            // V4: 18-byte block header with 40-bit (5-byte) content flags
            RootVersion::V4 => {
                let num_records = u32::read_le(reader)?;
                let locale_flags = LocaleFlags::read_le(reader)?;
                let content_flags = ContentFlags::read_v4(reader)?;
                let _unk2 = u32::read_le(reader)?;
                let _unk3 = u8::read_le(reader)?;

                // Sanity check - detect garbage data
                if num_records == 0 || num_records > 1_000_000 {
                    let header = RootBlockHeader {
                        num_records,
                        content_flags: content_flags.value,
                        locale_flags,
                    };
                    return Ok(Self {
                        header,
                        records: Vec::new(),
                    });
                }

                let count = num_records as usize;

                let header = RootBlockHeader {
                    num_records,
                    content_flags: content_flags.value,
                    locale_flags,
                };

                parse_v2_block(reader, header, count, content_flags)
            }
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
            RootVersion::V2 | RootVersion::V3 => {
                #[allow(clippy::cast_possible_truncation)]
                let header_v2 = RootBlockHeaderV2 {
                    num_records: self.header.num_records,
                    locale_flags: self.header.locale_flags,
                    content_flags: (self.header.content_flags & 0xFFFF_FFFF) as u32,
                    unk2: 0,
                    unk3: 0,
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn create_test_records() -> Vec<RootRecord> {
        vec![
            RootRecord::new(
                FileDataId::new(100),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                Some(0x1234_567890abcdef),
            ),
            RootRecord::new(
                FileDataId::new(102),
                ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                    .expect("Operation should succeed"),
                Some(0xfedc_ba0987654321),
            ),
        ]
    }

    #[test]
    fn test_block_header_round_trip() {
        let header = RootBlockHeader {
            num_records: 42,
            content_flags: 0x1234_5678,
            locale_flags: LocaleFlags::new(LocaleFlags::ENUS),
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write_le(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = RootBlockHeader::read_le(&mut cursor).expect("Operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(buffer.len(), 12); // 4 + 4 + 4 bytes
    }

    #[test]
    fn test_v1_block_round_trip() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        for record in create_test_records() {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V1, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V1, true).expect("Operation should succeed");

        assert_eq!(block, restored);
    }

    #[test]
    fn test_v2_block_round_trip_with_names() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        for record in create_test_records() {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V2, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V2, true).expect("Operation should succeed");

        assert_eq!(block, restored);
    }

    #[test]
    fn test_v2_block_round_trip_without_names() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::NO_NAME_HASH),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        // Create records without name hashes
        let records = vec![
            RootRecord::new(
                FileDataId::new(100),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                None,
            ),
            RootRecord::new(
                FileDataId::new(102),
                ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                    .expect("Operation should succeed"),
                None,
            ),
        ];

        for record in records {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V2, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V2, true).expect("Operation should succeed");

        assert_eq!(block, restored);
    }

    #[test]
    fn test_v3_block_round_trip() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE),
        );

        for record in create_test_records() {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V3, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V3, true).expect("Operation should succeed");

        assert_eq!(block, restored);
    }

    #[test]
    fn test_v4_block_round_trip() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::BUNDLE),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        for record in create_test_records() {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V4, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V4, true).expect("Operation should succeed");

        assert_eq!(block, restored);
    }

    #[test]
    fn test_v4_block_round_trip_extended_content_flags() {
        // V4 supports 40-bit content flags -- verify bits above 31 survive round-trip
        let flags_with_high_bits = ContentFlags::new(0xAB_0000_8004); // bit 39, 33, plus INSTALL
        let mut block = RootBlock::new(flags_with_high_bits, LocaleFlags::new(LocaleFlags::ENUS));

        for record in create_test_records() {
            block.add_record(record);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V4, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V4, true).expect("Operation should succeed");

        // The 40-bit content flags should survive the round-trip
        assert_eq!(
            restored.content_flags().value,
            0xAB_0000_8004,
            "V4 40-bit content flags should round-trip without truncation"
        );
        assert_eq!(block, restored);
    }

    #[test]
    fn test_empty_block_v1() {
        let block = RootBlock::new(
            ContentFlags::new(ContentFlags::NONE),
            LocaleFlags::new(LocaleFlags::ALL),
        );

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V1, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V1, true).expect("Operation should succeed");

        assert_eq!(block, restored);
        assert_eq!(restored.records.len(), 0);
        assert_eq!(buffer.len(), 12); // V1 header is 12 bytes
    }

    #[test]
    fn test_empty_block_v2() {
        let block = RootBlock::new(
            ContentFlags::new(ContentFlags::NONE),
            LocaleFlags::new(LocaleFlags::ALL),
        );

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        block
            .write(&mut cursor, RootVersion::V2, true)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootBlock::parse(&mut cursor, RootVersion::V2, true).expect("Operation should succeed");

        assert_eq!(block, restored);
        assert_eq!(restored.records.len(), 0);
        assert_eq!(buffer.len(), 17); // V2 header is 17 bytes
    }

    #[test]
    fn test_block_sort_records() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        // Add records in reverse order
        let records = vec![
            RootRecord::new(
                FileDataId::new(300),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                Some(0x1111_111111111111),
            ),
            RootRecord::new(
                FileDataId::new(100),
                ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                    .expect("Operation should succeed"),
                Some(0x2222_222222222222),
            ),
            RootRecord::new(
                FileDataId::new(200),
                ContentKey::from_hex("abcdefabcdefabcdefabcdefabcdefab")
                    .expect("Operation should succeed"),
                Some(0x3333_333333333333),
            ),
        ];

        for record in records {
            block.add_record(record);
        }

        // Should be unsorted
        assert_eq!(block.records[0].file_data_id, FileDataId::new(300));
        assert_eq!(block.records[1].file_data_id, FileDataId::new(100));
        assert_eq!(block.records[2].file_data_id, FileDataId::new(200));

        block.sort_records();

        // Should now be sorted
        assert_eq!(block.records[0].file_data_id, FileDataId::new(100));
        assert_eq!(block.records[1].file_data_id, FileDataId::new(200));
        assert_eq!(block.records[2].file_data_id, FileDataId::new(300));
    }

    #[test]
    fn test_block_size_calculation_v1() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        // Empty V1 block: header(12)
        assert_eq!(block.calculate_size(RootVersion::V1, true), 12);

        // Add records
        for record in create_test_records() {
            block.add_record(record);
        }

        // V1 with 2 records: header(12) + fdids(8) + ckeys(32) + names(16) = 68
        assert_eq!(block.calculate_size(RootVersion::V1, true), 68);
    }

    #[test]
    fn test_block_size_calculation_v2() {
        let mut block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL),
            LocaleFlags::new(LocaleFlags::ENUS),
        );

        // Empty V2 block: header(17)
        assert_eq!(block.calculate_size(RootVersion::V2, true), 17);

        // Add records
        for record in create_test_records() {
            block.add_record(record);
        }

        // V2 with 2 records and names: header(17) + fdids(8) + ckeys(32) + names(16) = 73
        assert_eq!(block.calculate_size(RootVersion::V2, true), 73);

        // V2 without names: header(17) + fdids(8) + ckeys(32) = 57
        let mut no_names_block = RootBlock::new(
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::NO_NAME_HASH),
            LocaleFlags::new(LocaleFlags::ENUS),
        );
        for record in create_test_records() {
            no_names_block.add_record(RootRecord::new(
                record.file_data_id,
                record.content_key,
                None,
            ));
        }
        assert_eq!(no_names_block.calculate_size(RootVersion::V2, true), 57);
    }
}
