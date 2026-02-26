//! Integration tests for root file parsing using real CDN data
//!
//! Tests parse real root files downloaded from Blizzard CDN via cascette-py.
//! Fixtures are truncated to a few blocks to keep the repo small.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use cascette_formats::root::{
    self,
    flags::{ContentFlags, LocaleFlags},
    version::RootVersion,
};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/root")
        .leak()
}

fn read_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

// --- V1 Classic Era root file tests ---

#[test]
fn root_cdn_v1_parse() {
    let data = read_fixture("classic_era_v1_2blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V1 root parse should succeed");

    assert_eq!(root.version, RootVersion::V1);
    assert_eq!(root.blocks.len(), 2);
    assert_eq!(root.blocks[0].num_records(), 1487);
    assert_eq!(root.blocks[1].num_records(), 6);
}

#[test]
fn root_cdn_v1_content_flags() {
    let data = read_fixture("classic_era_v1_2blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V1 root parse should succeed");

    // Block 0 has content_flags=0x00000080 = LOW_VIOLENCE
    let block0_flags = root.blocks[0].content_flags();
    assert_eq!(block0_flags.value, 0x0000_0080);
    assert!(
        block0_flags.has(ContentFlags::LOW_VIOLENCE),
        "Block 0 should have LOW_VIOLENCE flag (0x80)"
    );
    assert!(
        !block0_flags.has(ContentFlags::INSTALL),
        "Block 0 should not have INSTALL flag"
    );

    // Block 1 has content_flags=0x00000000 = NONE
    let block1_flags = root.blocks[1].content_flags();
    assert_eq!(block1_flags.value, 0);
}

#[test]
fn root_cdn_v1_locale_flags() {
    let data = read_fixture("classic_era_v1_2blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V1 root parse should succeed");

    // Block 0 has locale_flags=0x000173f6 which is a combination of locales
    let block0_locale = root.blocks[0].locale_flags();
    // Should include enUS (0x02)
    assert!(
        block0_locale.has(LocaleFlags::ENUS),
        "Block 0 should include enUS locale"
    );

    // Block 1 has locale_flags=0x00000002 = enUS only
    let block1_locale = root.blocks[1].locale_flags();
    assert_eq!(block1_locale.value(), LocaleFlags::ENUS);
}

#[test]
fn root_cdn_v1_records_have_data() {
    let data = read_fixture("classic_era_v1_2blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V1 root parse should succeed");

    // All records should have valid file data IDs
    for block in &root.blocks {
        for record in &block.records {
            // FileDataID should be > 0 for real data
            assert!(record.file_data_id.get() > 0);
            // V1 records always have name hashes
            assert!(record.has_name_hash());
        }
    }
}

#[test]
fn root_cdn_v1_lookup_tables() {
    let data = read_fixture("classic_era_v1_2blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V1 root parse should succeed");

    // parse() builds lookups automatically
    let (fdid_count, name_count) = root.lookup_stats();
    assert!(fdid_count > 0);
    assert!(name_count > 0);

    // Total entries across both blocks
    let expected_total = root
        .blocks
        .iter()
        .map(|b| b.num_records() as usize)
        .sum::<usize>();
    // fdid_count may be less than expected_total if same FDID appears in multiple blocks
    assert!(fdid_count <= expected_total);
}

// --- V2 Retail root file tests ---

#[test]
fn root_cdn_v2_extended_parse() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    assert_eq!(root.version, RootVersion::V2);
    assert!(root.header.is_some());
    assert_eq!(root.blocks.len(), 3);
    assert_eq!(root.blocks[0].num_records(), 6750);
    assert_eq!(root.blocks[1].num_records(), 1190);
    assert_eq!(root.blocks[2].num_records(), 2362);
}

#[test]
fn root_cdn_v2_header_properties() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    let header = root.header.as_ref().expect("V2 should have header");
    assert_eq!(header.magic(), root::header::RootMagic::Tsfm);
    // The truncated file has fewer files than the header claims (header is from full file)
    assert!(header.total_files() > 0);
}

#[test]
fn root_cdn_v2_content_flags_reconstruction() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    // Block 0: cf1=0x00000080, cf2=0x12000000, cf3=0x04
    // reconstructed = 0x00000080 | 0x12000000 | (0x04 << 17) = 0x12080080
    let block0_flags = root.blocks[0].content_flags();
    assert!(
        block0_flags.has(ContentFlags::LOW_VIOLENCE),
        "Block 0 reconstructed flags should include LOW_VIOLENCE (0x80)"
    );
}

#[test]
fn root_cdn_v2_locale_flags_coverage() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    // All blocks have locale_flags=0x0001f3f6 which is a broad locale mask
    let locale = root.blocks[0].locale_flags();
    assert_eq!(locale.value(), 0x0001_f3f6);

    // Verify our corrected locale constants match the real data
    assert!(locale.has(LocaleFlags::ENUS), "Should include enUS");
    assert!(locale.has(LocaleFlags::KOKR), "Should include koKR");
    assert!(locale.has(LocaleFlags::FRFR), "Should include frFR");
    assert!(locale.has(LocaleFlags::DEDE), "Should include deDE");
    assert!(locale.has(LocaleFlags::ZHCN), "Should include zhCN");
    assert!(locale.has(LocaleFlags::ESES), "Should include esES");
    assert!(locale.has(LocaleFlags::ZHTW), "Should include zhTW");
    assert!(locale.has(LocaleFlags::ENGB), "Should include enGB");
    assert!(locale.has(LocaleFlags::RURU), "Should include ruRU");
    assert!(locale.has(LocaleFlags::PTBR), "Should include ptBR");
    assert!(locale.has(LocaleFlags::ITIT), "Should include itIT");
    assert!(locale.has(LocaleFlags::PTPT), "Should include ptPT");
    // enCN, enTW, esMX are NOT in this particular mask (0x0001f3f6)
    // 0x0001f3f6 = 0b1_1111_0011_1111_0110
    // bits: 1(enus), 2(kokr), 4(frfr), 5(dede), 6(zhcn), 7(eses),
    //       8(zhtw), 9(engb), 13(ruru), 14(ptbr), 15(itit), 16(ptpt)
    // Missing: bit 10(encn), bit 11(entw), bit 12(esmx)
}

#[test]
fn root_cdn_v2_records_have_data() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    for block in &root.blocks {
        for record in &block.records {
            assert!(record.file_data_id.get() > 0);
        }
    }
}

#[test]
fn root_cdn_v2_lookup_tables() {
    let data = read_fixture("retail_11.2.7_v2_3blocks.root");
    let root = root::file::RootFile::parse(&data).expect("V2 extended root parse should succeed");

    let (fdid_count, _name_count) = root.lookup_stats();
    assert!(fdid_count > 0);
}
