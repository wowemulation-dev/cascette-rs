#![allow(clippy::expect_used, clippy::panic)]
//! Root file with FileDataID example
//!
//! Demonstrates building root files with RootBuilder, adding files with
//! FileDataID mappings, parsing them back, and resolving by ID and path.
//!
//! ```text
//! cargo run -p cascette-formats --example root_file
//! ```

use cascette_crypto::md5::{ContentKey, FileDataId};
use cascette_formats::root::{
    ContentFlags, LocaleFlags, RootBuilder, RootFile, RootVersion, calculate_name_hash,
};

fn main() {
    println!("=== Name Hash Calculation ===");

    let path1 = "Interface\\Icons\\INV_Misc_QuestionMark.blp";
    let path2 = "interface/icons/inv_misc_questionmark.blp";
    let hash1 = calculate_name_hash(path1);
    let hash2 = calculate_name_hash(path2);

    println!("Path:   {path1}");
    println!("Hash:   0x{hash1:016x}");
    println!("Path:   {path2}");
    println!("Hash:   0x{hash2:016x}");
    println!(
        "Match:  {} (case-insensitive, slash-normalized)",
        hash1 == hash2
    );

    let other_hash = calculate_name_hash("World\\Maps\\Azeroth\\Azeroth.wdt");
    println!("Other:  0x{other_hash:016x}");
    println!("Differ: {}", hash1 != other_hash);

    println!();
    println!("=== Building Root File (V2) ===");

    let mut builder = RootBuilder::new(RootVersion::V2);

    // Add files with different locale and content flag combinations
    let files = [
        (
            100u32,
            "0123456789abcdef0123456789abcdef",
            Some("Interface\\Icons\\INV_Misc_QuestionMark.blp"),
            LocaleFlags::ENUS,
            ContentFlags::INSTALL,
        ),
        (
            101,
            "fedcba9876543210fedcba9876543210",
            Some("World\\Maps\\Azeroth\\Azeroth.wdt"),
            LocaleFlags::ENUS | LocaleFlags::DEDE | LocaleFlags::FRFR,
            ContentFlags::INSTALL,
        ),
        (
            102,
            "aabbccddee0011223344556677889900",
            Some("Interface\\FrameXML\\UIParent.lua"),
            LocaleFlags::ALL,
            ContentFlags::INSTALL,
        ),
        (
            200,
            "1111111111111111aaaaaaaaaaaaaaaa",
            None, // No name hash
            LocaleFlags::ENUS,
            ContentFlags::INSTALL | ContentFlags::NO_NAME_HASH,
        ),
    ];

    for (fdid, hex, path, locale, content) in &files {
        let ckey = ContentKey::from_hex(hex).expect("hex parse should succeed");
        builder.add_file(
            FileDataId::new(*fdid),
            ckey,
            *path,
            LocaleFlags::new(*locale),
            ContentFlags::new(*content),
        );
    }

    // Need at least 100 files for V2 detection to work correctly
    for i in 300..400 {
        let ckey = ContentKey::from_hex(&format!("{i:032x}")).expect("hex parse should succeed");
        builder.add_file(
            FileDataId::new(i),
            ckey,
            Some(&format!("data/file{i}.bin")),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
    }

    let root_data = builder.build().expect("root build should succeed");
    println!("Serialized size:  {} bytes", root_data.len());

    println!();
    println!("=== Parsing Root File ===");

    let root = RootFile::parse(&root_data).expect("root parse should succeed");
    println!("Version:          {:?}", root.version);
    println!("Has header:       {}", root.header.is_some());
    println!("Total files:      {}", root.total_files());
    println!("Named files:      {}", root.named_files());

    match root.validate() {
        Ok(()) => println!("Validation:       passed"),
        Err(e) => println!("Validation:       failed ({e})"),
    }

    println!();
    println!("=== Resolve by FileDataID ===");

    for (fdid, _, _, locale, content) in &files {
        let resolved = root.resolve_by_id(
            FileDataId::new(*fdid),
            LocaleFlags::new(*locale),
            ContentFlags::new(*content),
        );
        match resolved {
            Some(ckey) => println!("  FDID {fdid:>5} -> CKey={}", hex::encode(ckey.as_bytes())),
            None => println!("  FDID {fdid:>5} -> not found"),
        }
    }

    // Look up a non-existent FileDataID
    let missing = root.resolve_by_id(
        FileDataId::new(99999),
        LocaleFlags::new(LocaleFlags::ENUS),
        ContentFlags::new(ContentFlags::INSTALL),
    );
    println!("  FDID 99999 -> found: {}", missing.is_some());

    println!();
    println!("=== Resolve by Path ===");

    let paths = [
        "Interface\\Icons\\INV_Misc_QuestionMark.blp",
        "interface/icons/inv_misc_questionmark.blp", // Case-insensitive
        "World\\Maps\\Azeroth\\Azeroth.wdt",
        "Interface\\FrameXML\\UIParent.lua",
        "nonexistent/path.blp",
    ];

    for path in paths {
        let resolved = root.resolve_by_path(
            path,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        match resolved {
            Some(ckey) => println!("  {path} -> {}", hex::encode(ckey.as_bytes())),
            None => println!("  {path} -> not found"),
        }
    }

    println!();
    println!("=== Root File Versions ===");

    for version in [
        RootVersion::V1,
        RootVersion::V2,
        RootVersion::V3,
        RootVersion::V4,
    ] {
        let mut vb = RootBuilder::new(version);
        // Add enough files for detection
        for i in 0..120 {
            let ckey =
                ContentKey::from_hex(&format!("{i:032x}")).expect("hex parse should succeed");
            vb.add_file(
                FileDataId::new(i),
                ckey,
                Some(&format!("file{i}.bin")),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );
        }
        let vdata = vb.build().expect("version build should succeed");
        let vparsed = RootFile::parse(&vdata).expect("version parse should succeed");
        println!(
            "  {:?}: has_header={} total_files={} size={} bytes",
            version,
            vparsed.header.is_some(),
            vparsed.total_files(),
            vdata.len(),
        );
    }

    println!();
    println!("=== Locale and Content Flags ===");

    println!("Locale flags:");
    println!("  ENUS:   0x{:08x}", LocaleFlags::ENUS);
    println!("  DEDE:   0x{:08x}", LocaleFlags::DEDE);
    println!("  FRFR:   0x{:08x}", LocaleFlags::FRFR);
    println!("  ALL:    0x{:08x}", LocaleFlags::ALL);

    println!("Content flags:");
    println!("  INSTALL:       0x{:08x}", ContentFlags::INSTALL);
    println!("  LOW_VIOLENCE:  0x{:08x}", ContentFlags::LOW_VIOLENCE);
    println!("  NO_NAME_HASH:  0x{:08x}", ContentFlags::NO_NAME_HASH);
    println!("  BUNDLE:        0x{:08x}", ContentFlags::BUNDLE);
}
