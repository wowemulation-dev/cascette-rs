#![allow(clippy::expect_used, clippy::panic)]
//! Config file parsing example
//!
//! Demonstrates parsing BuildConfig, CdnConfig, and KeyringConfig files
//! from realistic config strings and extracting their fields.
//!
//! ```text
//! cargo run -p cascette-formats --example parse_build_config
//! ```

use cascette_formats::config::{ArchiveInfo, BuildConfig, CdnConfig, KeyringConfig};

fn main() {
    println!("=== Build Config ===");

    let config_text = "\
# Build Configuration

root = a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6
install = 1111111111111111aaaaaaaaaaaaaaaa 2222222222222222bbbbbbbbbbbbbbbb
install-size = 4096 8192
download = 3333333333333333cccccccccccccccc 4444444444444444dddddddddddddddd
download-size = 16384 32768
encoding = 5555555555555555eeeeeeeeeeeeeeee 6666666666666666ffffffffffffffff
encoding-size = 65536 131072
build-name = WoW-52393patch11.0.7_Retail
build-uid = wow
build-product = wow
client-version = 11.0.7.58238
build-partial-priority = aabbccddee0011223344556677889900:262144 0099887766554433221100eeddccbbaa:1048576
";

    let config =
        BuildConfig::parse(config_text.as_bytes()).expect("BuildConfig should parse from text");

    println!("Root CKey:        {}", config.root().unwrap_or("(not set)"));
    println!(
        "Build name:       {}",
        config.build_name().unwrap_or("(not set)")
    );
    println!(
        "Build product:    {}",
        config.build_product().unwrap_or("(not set)")
    );
    println!(
        "Client version:   {}",
        config.client_version().unwrap_or("(not set)")
    );

    if let Some(enc) = config.encoding() {
        println!("Encoding CKey:    {}", enc.content_key);
        println!(
            "Encoding EKey:    {}",
            enc.encoding_key.as_deref().unwrap_or("(none)")
        );
        println!(
            "Encoding size:    {}",
            enc.size
                .map_or_else(|| "(none)".to_string(), |s| format!("{s} bytes"))
        );
    }

    let installs = config.install();
    println!("Install entries:  {}", installs.len());
    for (i, info) in installs.iter().enumerate() {
        println!("  [{i}] CKey={}", info.content_key);
    }

    let downloads = config.download();
    println!("Download entries: {}", downloads.len());

    let priorities = config.build_partial_priority();
    println!("Partial priority entries: {}", priorities.len());
    for p in &priorities {
        println!("  key={} priority={}", p.key, p.priority);
    }

    // Validate the config
    match config.validate() {
        Ok(()) => println!("Validation:       passed"),
        Err(e) => println!("Validation:       failed ({e})"),
    }

    // Round-trip: build back to bytes, re-parse
    let rebuilt = config.build();
    let reparsed = BuildConfig::parse(&rebuilt[..]).expect("Round-trip parse should succeed");
    println!(
        "Round-trip root:  {}",
        reparsed.root().unwrap_or("(not set)")
    );

    println!();
    println!("=== CDN Config ===");

    let mut cdn = CdnConfig::new();
    cdn.set_archives(vec![
        ArchiveInfo {
            content_key: "0036fbcc88e4c2e817b1bbaa89397c75".to_string(),
            index_size: Some(12_345),
        },
        ArchiveInfo {
            content_key: "00f40d4a63bcc2e87cf0fb62a3c47da4".to_string(),
            index_size: Some(67_890),
        },
        ArchiveInfo {
            content_key: "aabbccddee0011223344556677889900".to_string(),
            index_size: Some(54_321),
        },
    ]);
    cdn.set_archive_group("9e13aa0f34968b1f9b4fc7e09ae88d26");

    let archives = cdn.archives();
    println!("Archive count:    {}", archives.len());
    for (i, a) in archives.iter().enumerate() {
        println!(
            "  [{i}] key={} index_size={}",
            a.content_key,
            a.index_size
                .map_or_else(|| "(none)".to_string(), |s| s.to_string())
        );
    }
    println!(
        "Archive group:    {}",
        cdn.archive_group().unwrap_or("(not set)")
    );
    println!("Has patch archives: {}", cdn.has_patch_archives());

    // Round-trip
    let cdn_bytes = cdn.build();
    let cdn_reparsed =
        CdnConfig::parse(&cdn_bytes[..]).expect("CDN config round-trip should succeed");
    println!("Round-trip archives: {}", cdn_reparsed.archives().len());

    println!();
    println!("=== Keyring Config ===");

    let mut keyring = KeyringConfig::new();
    keyring.add_entry("4eb4869f95f23b53", "c9316739348dcc033aa8112f9a3acf5d");
    keyring.add_entry("1b3e4e1ecfb25877", "3de60d37c664723595f27c5cdbf08bfa");
    keyring.add_entry("205901f51aabb942", "c68778823c964c6f247acc0f4a2584f8");

    println!("Keyring entries:  {}", keyring.len());
    for entry in keyring.entries() {
        println!(
            "  key_id={} value={}...",
            entry.key_id,
            &entry.key_value[..16]
        );
    }

    // Lookup by hex ID
    let lookup_result = keyring.get_key("4eb4869f95f23b53");
    println!(
        "Lookup 4eb4869f95f23b53: {}",
        lookup_result.unwrap_or("(not found)")
    );

    // Lookup by numeric ID
    let numeric_result = keyring.get_key_by_id(0x1b3e4e1ecfb25877);
    println!(
        "Lookup 0x1b3e4e1ecfb25877: {}",
        numeric_result.unwrap_or("(not found)")
    );

    // Validate
    match keyring.validate() {
        Ok(()) => println!("Validation:       passed"),
        Err(e) => println!("Validation:       failed ({e})"),
    }

    // Round-trip
    let kr_bytes = keyring.build();
    let kr_reparsed =
        KeyringConfig::parse(&kr_bytes[..]).expect("Keyring round-trip should succeed");
    println!("Round-trip entries: {}", kr_reparsed.len());
}
