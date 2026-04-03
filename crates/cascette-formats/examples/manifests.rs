#![allow(clippy::expect_used, clippy::panic, clippy::cast_lossless)]
//! Install and Download manifest example
//!
//! Demonstrates building and parsing Install manifests with tags and files,
//! and Download manifests with priority-based streaming.
//!
//! ```text
//! cargo run -p cascette-formats --example manifests
//! ```

use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::download::{DownloadManifest, DownloadManifestBuilder, FileSize40};
use cascette_formats::install::{InstallManifest, InstallManifestBuilder, TagType};

fn main() {
    println!("=== Install Manifest ===");

    let ckey1 =
        ContentKey::from_hex("0123456789abcdef0123456789abcdef").expect("hex parse should succeed");
    let ckey2 =
        ContentKey::from_hex("fedcba9876543210fedcba9876543210").expect("hex parse should succeed");
    let ckey3 =
        ContentKey::from_hex("aabbccddee0011223344556677889900").expect("hex parse should succeed");

    let manifest = InstallManifestBuilder::new()
        .add_tag("Windows".to_string(), TagType::Platform)
        .add_tag("OSX".to_string(), TagType::Platform)
        .add_tag("x86_64".to_string(), TagType::Architecture)
        .add_tag("enUS".to_string(), TagType::Locale)
        .add_file("data/game.exe".to_string(), ckey1, 1_048_576)
        .add_file("data/game.app".to_string(), ckey2, 2_097_152)
        .add_file("data/shared.pak".to_string(), ckey3, 524_288)
        // Associate game.exe with Windows + x86_64 + enUS
        .associate_file_with_tag(0, "Windows")
        .expect("association should succeed")
        .associate_file_with_tag(0, "x86_64")
        .expect("association should succeed")
        .associate_file_with_tag(0, "enUS")
        .expect("association should succeed")
        // Associate game.app with OSX + x86_64 + enUS
        .associate_file_with_tag(1, "OSX")
        .expect("association should succeed")
        .associate_file_with_tag(1, "x86_64")
        .expect("association should succeed")
        .associate_file_with_tag(1, "enUS")
        .expect("association should succeed")
        // Associate shared.pak with all platforms
        .associate_file_with_tag(2, "Windows")
        .expect("association should succeed")
        .associate_file_with_tag(2, "OSX")
        .expect("association should succeed")
        .associate_file_with_tag(2, "x86_64")
        .expect("association should succeed")
        .associate_file_with_tag(2, "enUS")
        .expect("association should succeed")
        .build()
        .expect("build should succeed");

    println!("Version:          {}", manifest.header.version);
    println!("File count:       {}", manifest.entries.len());
    println!("Tag count:        {}", manifest.tags.len());

    for entry in &manifest.entries {
        println!("  {} ({} bytes)", entry.path, entry.file_size);
    }

    for tag in &manifest.tags {
        println!(
            "  Tag '{}' (type={:?}, files={})",
            tag.name,
            tag.tag_type,
            tag.file_count()
        );
    }

    // Check tag associations
    let windows_tag = manifest
        .tags
        .iter()
        .find(|t| t.name == "Windows")
        .expect("Windows tag should exist");
    println!("Windows has game.exe: {}", windows_tag.has_file(0));
    println!("Windows has game.app: {}", windows_tag.has_file(1));
    println!("Windows has shared.pak: {}", windows_tag.has_file(2));

    // Serialize and parse back
    let data = manifest.build().expect("manifest build should succeed");
    println!("Serialized size:  {} bytes", data.len());

    let parsed = InstallManifest::parse(&data).expect("parse should succeed");
    println!("Parsed files:     {}", parsed.entries.len());
    println!("Parsed tags:      {}", parsed.tags.len());

    // Validate
    match parsed.validate() {
        Ok(()) => println!("Validation:       passed"),
        Err(e) => println!("Validation:       failed ({e})"),
    }

    // Calculate install size for Windows x86_64
    let mut windows_size = 0u64;
    let win_tag = parsed.tags.iter().find(|t| t.name == "Windows");
    for (idx, entry) in parsed.entries.iter().enumerate() {
        if win_tag.is_none_or(|t| t.has_file(idx)) {
            windows_size += u64::from(entry.file_size);
        }
    }
    println!("Windows install:  {windows_size} bytes");

    println!();
    println!("=== Download Manifest (V1) ===");

    let ekey1 = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
        .expect("hex parse should succeed");
    let ekey2 = EncodingKey::from_hex("fedcba9876543210fedcba9876543210")
        .expect("hex parse should succeed");
    let ekey3 = EncodingKey::from_hex("aabbccddee0011223344556677889900")
        .expect("hex parse should succeed");

    let dl_v1 = DownloadManifestBuilder::new(1)
        .expect("v1 builder should succeed")
        .add_file(ekey1, 1024, -1)
        .expect("add_file should succeed") // Critical priority
        .add_file(ekey2, 4096, 0)
        .expect("add_file should succeed") // Essential priority
        .add_file(ekey3, 65536, 50)
        .expect("add_file should succeed") // Normal priority
        .add_tag("Windows".to_string(), TagType::Platform)
        .associate_file_with_tag(0, "Windows")
        .expect("association should succeed")
        .associate_file_with_tag(1, "Windows")
        .expect("association should succeed")
        .associate_file_with_tag(2, "Windows")
        .expect("association should succeed")
        .build()
        .expect("build should succeed");

    println!("Version:          {}", dl_v1.header.version());
    println!("File count:       {}", dl_v1.entries.len());
    println!("Tag count:        {}", dl_v1.tags.len());

    for (i, entry) in dl_v1.entries.iter().enumerate() {
        let category = entry.priority_category(&dl_v1.header);
        let effective = entry.effective_priority(&dl_v1.header);
        println!(
            "  [{i}] size={} priority={} effective={} category={:?} essential={}",
            entry.file_size.as_u64(),
            entry.priority,
            effective,
            category,
            entry.is_essential(&dl_v1.header),
        );
    }

    // Serialize and parse back
    let dl_data = dl_v1.build().expect("build should succeed");
    let dl_parsed = DownloadManifest::parse(&dl_data).expect("parse should succeed");
    println!("Round-trip files: {}", dl_parsed.entries.len());

    println!();
    println!("=== Download Manifest (V3 with base priority) ===");

    let dl_v3 = DownloadManifestBuilder::new(3)
        .expect("v3 builder should succeed")
        .with_checksums(true)
        .with_flags(1)
        .expect("with_flags should succeed")
        .with_base_priority(-2)
        .expect("with_base_priority should succeed")
        .add_file(ekey1, 2048, -1)
        .expect("add_file should succeed")
        .add_file(ekey2, 8192, 5)
        .expect("add_file should succeed")
        .set_file_checksum(0, 0xDEAD_BEEF)
        .expect("set_file_checksum should succeed")
        .set_file_checksum(1, 0xCAFE_BABE)
        .expect("set_file_checksum should succeed")
        .set_file_flags(0, vec![0x01])
        .expect("set_file_flags should succeed")
        .set_file_flags(1, vec![0x02])
        .expect("set_file_flags should succeed")
        .build()
        .expect("build should succeed");

    println!("Version:          {}", dl_v3.header.version());
    println!("Base priority:    {}", dl_v3.header.base_priority());
    println!("Flag size:        {}", dl_v3.header.flag_size());

    for (i, entry) in dl_v3.entries.iter().enumerate() {
        let effective = entry.effective_priority(&dl_v3.header);
        let category = entry.priority_category(&dl_v3.header);
        println!(
            "  [{i}] size={} raw_prio={} effective={} category={:?} checksum={:?} flags={:?}",
            entry.file_size.as_u64(),
            entry.priority,
            effective,
            category,
            entry.checksum,
            entry.flags,
        );
    }

    println!();
    println!("=== FileSize40 (40-bit file sizes) ===");

    let sizes = [0u64, 1024, 1_073_741_824, 549_755_813_888];
    for &sz in &sizes {
        let fs40 = FileSize40::new(sz).expect("FileSize40::new should succeed");
        let bytes = fs40.to_bytes();
        println!(
            "  {sz:>15} -> [{:02x} {:02x} {:02x} {:02x} {:02x}] -> {}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            fs40.as_u64()
        );
    }

    // Maximum 40-bit value
    let max_40 = FileSize40::new(0xFF_FFFF_FFFF).expect("max 40-bit should succeed");
    println!("  Max 40-bit: {}", max_40.as_u64());

    // Oversized value should fail
    let oversized = FileSize40::new(0x1_0000_0000_0000);
    println!("  Oversized rejected: {}", oversized.is_err());

    println!();
    println!("=== Priority Categories ===");

    let categories = [
        (-5i8, "Critical"),
        (0, "Essential"),
        (5, "High"),
        (50, "Normal"),
        (120, "Low"),
    ];

    for (prio, label) in categories {
        let m = DownloadManifestBuilder::new(1)
            .expect("builder should succeed")
            .add_file(ekey1, 1024, prio)
            .expect("add_file should succeed")
            .build()
            .expect("build should succeed");
        let cat = m.entries[0].priority_category(&m.header);
        println!("  priority={prio:>4} -> {cat:?} (expected: {label})");
    }
}
