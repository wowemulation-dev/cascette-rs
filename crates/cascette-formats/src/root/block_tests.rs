//! Tests for root block parsing and serialization
#![allow(clippy::expect_used, clippy::unwrap_used)]

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
